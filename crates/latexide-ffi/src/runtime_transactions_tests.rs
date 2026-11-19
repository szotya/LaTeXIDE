//! Tests use the real Runtime and Workspace, with only private synthetic files.
use super::{RuntimeBoundTransaction, RuntimeTransactionGate};
use crate::runtime::Runtime;
use latexide_core::{CoreError, LineEnding, Position, TextEdit, Workspace};
use latexide_core::contracts::RevisionedEdit;
use latexide_protocol::Request;
use std::{fs, path::{Path, PathBuf}, sync::{Arc, Mutex, mpsc}, thread, time::{Duration, Instant}};

// Frozen test bodies only. Their imports exercise the actual external core crate.
#[path = "../../latexide-core/tests/transaction_contract.rs"]
mod canonical_regression;
#[path = "../../latexide-core/tests/transaction_workspace_contract.rs"]
mod workspace_regression;

fn project(label: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let base = PathBuf::from(std::env::var_os("P02_RUNTIME_FIXTURES").expect("own fixture root"));
    let image = std::env::current_exe().expect("own test image");
    let image = image.file_stem().and_then(|name| name.to_str()).expect("own test image name");
    let pid = std::process::id();
    let root = base.join(image).join(pid.to_string()).join(label);
    assert!(!root.exists(), "fresh fixture; no case replay");
    fs::create_dir_all(&root).unwrap();
    for (name, bytes) in files { fs::write(root.join(name), bytes).unwrap(); }
    println!("P02_RUNTIME_FIXTURE nonce={} pid={pid} image={image} label={label} root={}",
        std::env::var("P02_RUN_NONCE").unwrap(), root.display());
    root
}

fn runtime(root: &Path) -> Arc<Mutex<Runtime>> {
    // The actual production type; Session transport/auth and compiler jobs stay unstarted.
    Arc::new(Mutex::new(Runtime::new(Workspace::open(root.to_path_buf()).unwrap())))
}

fn request(state: &Arc<Mutex<Runtime>>, request: Request) {
    let reply = state.lock().unwrap().dispatch(request, "user");
    assert!(reply.error.is_none(), "{:?}", reply.error);
    assert!(reply.result.is_some());
}

fn open(state: &Arc<Mutex<Runtime>>, path: &str) {
    request(state, Request::OpenDocument {path: path.into()});
}

fn edit(revision: u64, text: &str) -> RevisionedEdit {
    RevisionedEdit {document_id: "main.tex".into(), expected_revision: revision,
        start_utf16: 0, length_utf16: 1, replacement: text.into()}
}

fn current_plan(gate: &RuntimeTransactionGate, state: &Arc<Mutex<Runtime>>, text: &str) -> RuntimeBoundTransaction {
    let revision = state.lock().unwrap().workspace.snapshot("main.tex").unwrap().revision;
    gate.prepare(vec![edit(revision, text)]).unwrap()
}

fn code<T>(result: Result<T, CoreError>) -> String {
    match result {Err(error) => error.code, Ok(_) => panic!("runtime refusal required")}
}

#[test]
fn r01_dirty_revision_bom_crlf_plan() {
    // A disk-analysis capture or a second clean document store loses this dirty input.
    let mut disk = vec![0xef, 0xbb, 0xbf];
    disk.extend_from_slice("\u{feff}A😀\r\nB\r\n".as_bytes());
    let root = project("r01", &[("main.tex", &disk), ("closed.tex", b"not retained")]);
    let state = runtime(&root); open(&state, "main.tex");
    request(&state, Request::ApplyEdits {document_id: "main.tex".into(), expected_revision: 1,
        edits: vec![TextEdit {start: Position {line: 0, character: 1},
            end: Position {line: 0, character: 2}, text: "a".into()}]});
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![RevisionedEdit {document_id: "main.tex".into(), expected_revision: 2,
        start_utf16: 2, length_utf16: 2, replacement: "🌍".into()}]).unwrap();
    assert_eq!(bound.snapshot().files.len(), 1, "only retained editor buffers");
    let file = &bound.snapshot().files[0];
    assert_eq!(file.utf8, "\u{feff}a😀\nB\n".as_bytes());
    assert_eq!(file.revision, Some(2)); assert!(file.dirty);
    assert_eq!(bound.plan().edits[0].start_utf16, 2);
    assert_eq!(bound.plan().edits[0].length_utf16, 2);
    gate.revalidate(&bound).unwrap();
    let live = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(live.text, "\u{feff}a😀\nB\n"); assert!(live.bom);
    assert_eq!(live.line_ending, LineEnding::Crlf); assert_eq!(live.revision, 2);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
}

#[test]
fn r02_guards_release_after_prepare_and_revalidation() {
    // Retaining a guard or a strong owner would prevent these genuine consumer operations.
    let root = project("r02", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = current_plan(&gate, &state, "A");
    assert!(state.try_lock().is_ok(), "prepare must release its real guard");
    gate.revalidate(&bound).unwrap();
    assert!(state.try_lock().is_ok(), "revalidate must release its real guard");
    assert_eq!(Arc::strong_count(&state), 1);
    let second_caller = Arc::clone(&state);
    RuntimeTransactionGate::new(&second_caller).revalidate(&bound).unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "abc");
}

#[test]
fn r03_busy_and_poison_refuse_without_waiting() {
    // A blocking lock waits until the holder releases; poison recovery accepts invalid authority.
    let root = project("r03", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = current_plan(&gate, &state, "A");
    let guard = state.lock().unwrap(); let worker_gate = gate.clone();
    let (send, recv) = mpsc::channel();
    let worker = thread::spawn(move || {
        let start = Instant::now();
        let prepare = code(worker_gate.prepare(vec![]));
        let revalidate = code(worker_gate.revalidate(&bound));
        send.send((prepare, revalidate, start.elapsed())).unwrap();
    });
    let outcome = recv.recv_timeout(Duration::from_millis(250));
    drop(guard); worker.join().unwrap();
    let (prepare, revalidate, elapsed) = outcome.expect("try_lock must refuse while the holder is locked");
    assert_eq!(prepare, "runtime_busy"); assert_eq!(revalidate, "runtime_busy");
    assert!(elapsed < Duration::from_millis(250));
    println!("P02_RUNTIME_CONTENTION_NS {}", elapsed.as_nanos());
    let bound = current_plan(&gate, &state, "A");
    println!("P02_RUNTIME_EXPECTED_POISON");
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = state.lock().unwrap(); panic!("P02 expected owned Mutex poison");
    }));
    assert!(poisoned.is_err() && state.is_poisoned());
    assert_eq!(code(gate.prepare(vec![])), "runtime_unavailable");
    assert_eq!(code(gate.revalidate(&bound)), "runtime_unavailable");
}

#[test]
fn r04_foreign_runtime_owner_refuses() {
    // Equal files/revisions do not make another real Arc the same owner.
    let root = project("r04", &[("main.tex", b"abc")]);
    let state = runtime(&root); let foreign = runtime(&root);
    open(&state, "main.tex"); open(&foreign, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = current_plan(&gate, &state, "A");
    assert_eq!(bound.snapshot().files, foreign.lock().unwrap().workspace.transaction_snapshot().unwrap().files);
    assert_eq!(code(RuntimeTransactionGate::new(&foreign).revalidate(&bound)), "runtime_transaction_owner_mismatch");
    gate.revalidate(&bound).unwrap();
    assert_eq!(foreign.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "abc");
}

#[test]
fn r05_weak_drop_and_new_runtime_aba_refuse() {
    // The capsule must not keep the Runtime alive or bind a new host at the same project.
    let root = project("r05", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state); let weak = Arc::downgrade(&state);
    let bound = current_plan(&gate, &state, "A");
    assert_eq!(Arc::strong_count(&state), 1); drop(state); assert!(weak.upgrade().is_none());
    assert_eq!(code(gate.prepare(vec![])), "runtime_unavailable");
    assert_eq!(code(gate.revalidate(&bound)), "runtime_unavailable");
    let replacement = runtime(&root); open(&replacement, "main.tex");
    let new_gate = RuntimeTransactionGate::new(&replacement);
    assert_eq!(bound.snapshot().files, replacement.lock().unwrap().workspace.transaction_snapshot().unwrap().files);
    assert_eq!(code(new_gate.revalidate(&bound)), "runtime_transaction_owner_mismatch");
    let fresh = current_plan(&new_gate, &replacement, "A"); new_gate.revalidate(&fresh).unwrap();
}

#[test]
fn r06_workspace_edit_revision_membership_and_reopen_stale() {
    // Captured-only validation misses actual live edits, undo revisions and membership.
    let root = project("r06", &[("main.tex", b"abc"), ("part.tex", b"part")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state); let old = current_plan(&gate, &state, "A");
    request(&state, Request::ApplyEdits {document_id: "main.tex".into(), expected_revision: 1,
        edits: vec![TextEdit {start: Position {line: 0, character: 0},
            end: Position {line: 0, character: 1}, text: "x".into()}]});
    let stale = gate.revalidate(&old);
    assert!(stale.is_err(), "P02_EXPECTED_RUNTIME_STALE_RED: actual Runtime edit must invalidate the old plan");
    assert_eq!(code(stale), "stale_snapshot");
    let before_undo = current_plan(&gate, &state, "X");
    request(&state, Request::Undo);
    let restored = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(restored.text, "abc"); assert_eq!(restored.revision, 3);
    assert_eq!(code(gate.revalidate(&old)), "stale_snapshot");
    assert_eq!(code(gate.revalidate(&before_undo)), "stale_snapshot");
    let before_membership = current_plan(&gate, &state, "A");
    open(&state, "part.tex");
    assert_eq!(code(gate.revalidate(&before_membership)), "stale_snapshot");
    let fresh = current_plan(&gate, &state, "A"); gate.revalidate(&fresh).unwrap();

    let root = project("r06-workspace-reopen", &[("main.tex", b"abc")]);
    let same_runtime = runtime(&root); open(&same_runtime, "main.tex");
    let same_gate = RuntimeTransactionGate::new(&same_runtime);
    let before_reopen = current_plan(&same_gate, &same_runtime, "A");
    // No document-close API exists here. Drop/reopen the actual Workspace within the same Runtime Arc.
    same_runtime.lock().unwrap().workspace = Workspace::open(root.clone()).unwrap();
    open(&same_runtime, "main.tex");
    let reopened = same_runtime.lock().unwrap().workspace.transaction_snapshot().unwrap();
    assert_eq!(reopened.files, before_reopen.snapshot().files);
    assert_eq!(reopened.epoch, before_reopen.snapshot().epoch);
    assert_ne!(reopened.id, before_reopen.snapshot().id);
    assert_eq!(code(same_gate.revalidate(&before_reopen)), "stale_snapshot");
    let fresh = current_plan(&same_gate, &same_runtime, "A"); same_gate.revalidate(&fresh).unwrap();
}

#[test]
fn r07_active_composition_refuses_prepare_and_revalidation() {
    // A capture while native composition is active observes uncommitted editor state.
    let root = project("r07", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state); let bound = current_plan(&gate, &state, "A");
    request(&state, Request::BeginComposition {document_id: "main.tex".into(), expected_revision: 1});
    assert_eq!(code(gate.prepare(vec![edit(1, "A")])), "composition_active");
    assert_eq!(code(gate.revalidate(&bound)), "composition_active");
    assert!(state.try_lock().is_ok());
    request(&state, Request::EndComposition {document_id: "main.tex".into(), expected_revision: 1, edits: vec![]});
    gate.revalidate(&bound).unwrap();
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn r08_corrupted_or_different_plan_refuses() {
    // Private test access models damaged pairing; the production capsule has no mutator/decoder.
    let root = project("r08", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let mut corrupt = current_plan(&gate, &state, "A"); corrupt.plan_id.push_str("corrupt");
    assert_eq!(code(gate.revalidate(&corrupt)), "runtime_transaction_plan_mismatch");
    let mut swapped = current_plan(&gate, &state, "A");
    let different = current_plan(&gate, &state, "B");
    assert_ne!(swapped.plan().transaction_id, different.plan().transaction_id);
    swapped.transaction = different.transaction;
    assert_eq!(code(gate.revalidate(&swapped)), "runtime_transaction_plan_mismatch");
    let fresh = current_plan(&gate, &state, "A"); gate.revalidate(&fresh).unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "abc");
}

// Single-document commit controls. The preceding qualified 48 bodies are retained
// unchanged and are not selected by this task's eight-method execution.
fn commit_edit(id: &str, revision: u64, start: u32, length: u32, text: &str) -> RevisionedEdit {
    RevisionedEdit {document_id: id.into(), expected_revision: revision,
        start_utf16: start, length_utf16: length, replacement: text.into()}
}

fn commit_seen(state: &Arc<Mutex<Runtime>>) -> serde_json::Value {
    let live = state.lock().unwrap();
    serde_json::json!({"documents": live.workspace.snapshots(),
        "undo": live.workspace.can_undo(), "redo": live.workspace.can_redo(),
        "composition": live.composition.as_deref()})
}

#[test]
fn c01_two_edits_one_document_one_undo_redo() {
    // Per-edit application would shift the second range or create two history entries.
    let root = project("c01", &[("main.tex", b"abc def\n")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 4, 3, "tail"),
        commit_edit("main.tex", 1, 0, 3, "HEAD")]).unwrap();
    let info = gate.commit(bound, "agent").unwrap();
    assert_eq!(info.revision, 2); assert!(info.dirty);
    assert_eq!(info.last_origin, "agent"); assert!(info.transaction_id > 0);
    assert!(state.try_lock().is_ok(), "commit releases the real guard");
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "HEAD tail\n");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc def\n");
    request(&state, Request::Undo);
    let undone = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(undone.text, "abc def\n"); assert_eq!(undone.revision, 3);
    assert!(!state.lock().unwrap().workspace.can_undo());
    assert!(state.lock().unwrap().workspace.can_redo());
    request(&state, Request::Redo);
    let redone = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(redone.text, "HEAD tail\n"); assert_eq!(redone.revision, 4);
    assert!(!state.lock().unwrap().workspace.can_redo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc def\n");
}

#[test]
fn c02_stale_owned_plan_refuses() {
    // Revalidation before an independent lock, or no live check, accepts this stale plan.
    let root = project("c02", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let old = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    request(&state, Request::ApplyEdits {document_id: "main.tex".into(), expected_revision: 1,
        edits: vec![TextEdit {start: Position {line: 0, character: 0},
            end: Position {line: 0, character: 1}, text: "x".into()}]});
    let before = commit_seen(&state);
    assert_eq!(code(gate.commit(old, "agent")), "stale_snapshot");
    assert_eq!(commit_seen(&state), before);
    let mut damaged = gate.prepare(vec![commit_edit("main.tex", 2, 0, 1, "X")]).unwrap();
    damaged.plan_id.push_str("damaged");
    assert_eq!(code(gate.commit(damaged, "agent")), "runtime_transaction_plan_mismatch");
    assert_eq!(commit_seen(&state), before);
    assert!(state.try_lock().is_ok());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn c03_foreign_owner_refuses() {
    // Matching file bytes do not authorize a different actual Runtime allocation.
    let root = project("c03", &[("main.tex", b"abc")]);
    let state = runtime(&root); let foreign = runtime(&root);
    open(&state, "main.tex"); open(&foreign, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let other_gate = RuntimeTransactionGate::new(&foreign);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    let before = commit_seen(&state); let other_before = commit_seen(&foreign);
    assert_eq!(code(other_gate.commit(bound, "agent")), "runtime_transaction_owner_mismatch");
    assert_eq!(commit_seen(&state), before); assert_eq!(commit_seen(&foreign), other_before);
    let fresh = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    gate.commit(fresh, "agent").unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "Abc");
    assert_eq!(commit_seen(&foreign), other_before);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn c04_active_composition_refuses() {
    // Bypassing Runtime's composition gate would commit over an uncommitted native edit.
    let root = project("c04", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    request(&state, Request::BeginComposition {document_id: "main.tex".into(), expected_revision: 1});
    let before = commit_seen(&state);
    assert_eq!(code(gate.commit(bound, "agent")), "composition_active");
    assert_eq!(commit_seen(&state), before); assert!(state.try_lock().is_ok());
    request(&state, Request::EndComposition {document_id: "main.tex".into(), expected_revision: 1, edits: vec![]});
    let fresh = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    gate.commit(fresh, "agent").unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "Abc");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn c05_busy_drop_poison_refuse() {
    // A blocking lock, strong lifetime extension or poison recovery breaks this refusal boundary.
    let root = project("c05", &[("main.tex", b"abc")]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    let before = commit_seen(&state); let guard = state.lock().unwrap();
    let worker_gate = gate.clone(); let (send, recv) = mpsc::channel();
    let worker = thread::spawn(move || {
        let started = Instant::now();
        let result = code(worker_gate.commit(bound, "agent"));
        send.send((result, started.elapsed())).unwrap();
    });
    let outcome = recv.recv_timeout(Duration::from_millis(250));
    drop(guard); worker.join().unwrap();
    let (error, elapsed) = outcome.expect("commit try_lock must not wait for owner unlock");
    assert_eq!(error, "runtime_busy"); assert!(elapsed < Duration::from_millis(250));
    assert_eq!(commit_seen(&state), before);
    println!("P02_COMMIT_CONTENTION_NS {}", elapsed.as_nanos());

    let poisoned_bound = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    println!("P02_COMMIT_EXPECTED_POISON");
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = state.lock().unwrap(); panic!("P02 commit expected owned Mutex poison");
    })).is_err());
    assert_eq!(code(gate.commit(poisoned_bound, "agent")), "runtime_unavailable");
    assert!(state.is_poisoned());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");

    let root = project("c05-drop", &[("main.tex", b"abc")]);
    let dropped = runtime(&root); open(&dropped, "main.tex");
    let weak = Arc::downgrade(&dropped); let dropped_gate = RuntimeTransactionGate::new(&dropped);
    let bound = dropped_gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    assert_eq!(Arc::strong_count(&dropped), 1); drop(dropped); assert!(weak.upgrade().is_none());
    assert_eq!(code(dropped_gate.commit(bound, "agent")), "runtime_unavailable");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn c06_unicode_bom_crlf_atomic_batch() {
    // Byte-based columns, stripped logical FEFF or LF handling corrupt these two ranges.
    let mut disk = vec![0xef, 0xbb, 0xbf];
    disk.extend_from_slice("\u{feff}A😀\r\nBé\r\n".as_bytes());
    let root = project("c06", &[("main.tex", &disk)]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state); let before = commit_seen(&state);
    assert_eq!(code(gate.prepare(vec![commit_edit("main.tex", 1, 3, 0, "x")])), "transaction_refused");
    assert_eq!(commit_seen(&state), before);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 4, 2, "\nŐ"),
        commit_edit("main.tex", 1, 2, 2, "🌍")]).unwrap();
    gate.commit(bound, "agent").unwrap();
    let live = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(live.text, "\u{feff}A🌍\nŐé\n"); assert_eq!(live.revision, 2);
    assert!(live.bom && live.dirty); assert_eq!(live.line_ending, LineEnding::Crlf);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
    request(&state, Request::Undo);
    let undone = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(undone.text, "\u{feff}A😀\nBé\n"); assert!(!undone.dirty);
    assert!(!state.lock().unwrap().workspace.can_undo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
}

#[test]
fn c07_explicit_save_then_undo() {
    // Implicit Save or a new undo stack would lose the accepted Save-after-edit history contract.
    let mut disk = vec![0xef, 0xbb, 0xbf];disk.extend_from_slice(b"one\r\ntwo\r\n");
    let root = project("c07", &[("main.tex", &disk)]);
    let state = runtime(&root); open(&state, "main.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![commit_edit("main.tex", 1, 4, 3, "TWO"),
        commit_edit("main.tex", 1, 0, 3, "ONE")]).unwrap();
    let info = gate.commit(bound, "agent").unwrap(); assert_eq!(info.revision, 2);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
    request(&state, Request::SaveDocument {document_id: "main.tex".into(), expected_revision: 2});
    let mut saved_disk = vec![0xef, 0xbb, 0xbf];saved_disk.extend_from_slice(b"ONE\r\nTWO\r\n");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), saved_disk);
    let saved = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(saved.revision, 2); assert!(!saved.dirty);
    assert!(state.lock().unwrap().workspace.can_undo());
    request(&state, Request::Undo);
    let undone = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(undone.text, "one\ntwo\n"); assert_eq!(undone.revision, 3); assert!(undone.dirty);
    assert!(!state.lock().unwrap().workspace.can_undo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), saved_disk);
    request(&state, Request::Redo);
    let redone = state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(redone.text, "ONE\nTWO\n"); assert_eq!(redone.revision, 4); assert!(!redone.dirty);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), saved_disk);
}

#[test]
fn c08_multidocument_whole_refusal() {
    // Checking IDs while applying would partially mutate the first document and its history.
    let root = project("c08", &[("main.tex", b"abc"), ("part.tex", b"def")]);
    let state = runtime(&root); open(&state, "main.tex"); open(&state, "part.tex");
    let gate = RuntimeTransactionGate::new(&state); let before = commit_seen(&state);
    let multi = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A"),
        commit_edit("part.tex", 1, 0, 1, "D")]).unwrap();
    assert_eq!(code(gate.commit(multi, "agent")), "transaction_single_document_required");
    assert_eq!(commit_seen(&state), before);
    let empty = gate.prepare(vec![]).unwrap();
    assert_eq!(code(gate.commit(empty, "agent")), "transaction_single_document_required");
    assert_eq!(commit_seen(&state), before);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(), b"def");
    let one = gate.prepare(vec![commit_edit("main.tex", 1, 0, 1, "A")]).unwrap();
    gate.commit(one, "agent").unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "Abc");
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text, "def");
    request(&state, Request::Undo);
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "abc");
    assert!(!state.lock().unwrap().workspace.can_undo());
}

// Multi-document cohort: ten methods selected once; all earlier methods stay unrun.
fn multi_setup(label: &str) -> (PathBuf, Arc<Mutex<Runtime>>, RuntimeTransactionGate) {
    let root = project(label, &[("main.tex", b"abc\n"), ("part.tex", b"def\n")]);
    let state = runtime(&root); open(&state, "main.tex"); open(&state, "part.tex");
    let gate = RuntimeTransactionGate::new(&state); (root, state, gate)
}
fn multi_plan(gate: &RuntimeTransactionGate, state: &Arc<Mutex<Runtime>>) -> RuntimeBoundTransaction {
    let live = state.lock().unwrap();
    let a = live.workspace.snapshot("main.tex").unwrap().revision;
    let b = live.workspace.snapshot("part.tex").unwrap().revision; drop(live);
    gate.prepare(vec![commit_edit("main.tex", a, 0, 1, "A"), commit_edit("part.tex", b, 0, 1, "D")]).unwrap()
}
fn multi_seen(state: &Arc<Mutex<Runtime>>) -> serde_json::Value {
    let live = state.lock().unwrap();
    serde_json::json!({"documents":live.workspace.snapshots(),"undo":live.workspace.can_undo(),
        "redo":live.workspace.can_redo(),"capture":live.workspace.transaction_snapshot().unwrap()})
}

#[test]
fn m01_two_document_success() {
    let mut bytes = vec![0xef,0xbb,0xbf]; bytes.extend_from_slice("A😀\r\n".as_bytes());
    let other: &[u8] = &[0xef,0xbb,0xbf,0xef,0xbb,0xbf,b'b',0xc3,0xa9,b'\n'];
    let root = project("m01", &[("main.tex",&bytes),("part.tex",other)]);
    let state = runtime(&root); open(&state,"main.tex"); open(&state,"part.tex");
    let gate = RuntimeTransactionGate::new(&state);
    let bound = gate.prepare(vec![commit_edit("part.tex",1,1,1,"B"),commit_edit("main.tex",1,1,2,"🌍")]).unwrap();
    let result = gate.commit_multi(bound,"agent").unwrap();
    assert_eq!(result.documents.len(),2);
    assert_eq!(result.documents[0].transaction_id,result.documents[1].transaction_id);
    assert!(result.documents[0].transaction_id>0);
    assert!(result.documents.iter().all(|d|d.revision==2 && d.last_origin=="agent" && d.dirty));
    let a=state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    assert_eq!(a.text,"A🌍\n"); assert!(a.bom); assert_eq!(a.line_ending,LineEnding::Crlf);
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text,"\u{feff}Bé\n");
    assert!(state.try_lock().is_ok());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),bytes);
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),other);
}

#[test]
fn m02_common_undo_group() {
    let (root,state,gate)=multi_setup("m02");
    gate.commit_multi(multi_plan(&gate,&state),"agent").unwrap();
    request(&state,Request::Undo);
    for (id,text) in [("main.tex","abc\n"),("part.tex","def\n")] {
        let d=state.lock().unwrap().workspace.snapshot(id).unwrap();
        assert_eq!(d.text,text); assert_eq!(d.revision,3); assert_eq!(d.last_origin,"undo:agent"); assert!(!d.dirty);
    }
    assert!(!state.lock().unwrap().workspace.can_undo()); assert!(state.lock().unwrap().workspace.can_redo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"abc\n");
}

#[cfg(target_os = "macos")]
#[test]
fn m03_common_redo_group() {
    let root=project("m03",&[("main.tex",b"abc\n"),("part.tex",b"def\n")]);
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.parent().unwrap(),fs::Permissions::from_mode(0o700)).unwrap();}
    let storage=root.parent().unwrap().join("m03-recovery");
    let state=Arc::new(Mutex::new(Runtime::new(Workspace::open_recoverable(root.clone(),storage.clone()).unwrap())));
    open(&state,"main.tex"); open(&state,"part.tex"); let gate=RuntimeTransactionGate::new(&state);
    gate.commit_multi(multi_plan(&gate,&state),"agent").unwrap();
    request(&state,Request::Undo); request(&state,Request::Redo);
    for (id,text) in [("main.tex","Abc\n"),("part.tex","Def\n")] {
        let d=state.lock().unwrap().workspace.snapshot(id).unwrap();
        assert_eq!(d.text,text); assert_eq!(d.revision,4); assert_eq!(d.last_origin,"redo:agent");
    }
    assert!(!state.lock().unwrap().workspace.can_redo()); drop(gate); drop(state);
    // Normal checkpoint/delta row roundtrip only; no crash/fault or OS-atomic claim.
    let recovered=Arc::new(Mutex::new(Runtime::new(Workspace::open_recoverable(root.clone(),storage).unwrap())));
    assert!(recovered.lock().unwrap().workspace.recovery_status().restored);
    request(&recovered,Request::Undo);
    assert_eq!(recovered.lock().unwrap().workspace.snapshot("main.tex").unwrap().text,"abc\n");
    assert_eq!(recovered.lock().unwrap().workspace.snapshot("part.tex").unwrap().text,"def\n");
    assert!(!recovered.lock().unwrap().workspace.can_undo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"abc\n");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),b"def\n");
}

#[test]
fn m04_second_document_stale_whole_refusal() {
    let (_,state,gate)=multi_setup("m04"); let bound=multi_plan(&gate,&state);
    request(&state,Request::ApplyEdits{document_id:"part.tex".into(),expected_revision:1,
        edits:vec![TextEdit{start:Position{line:0,character:0},end:Position{line:0,character:1},text:"x".into()}]});
    let before=multi_seen(&state);
    assert_eq!(code(gate.commit_multi(bound,"agent")),"stale_snapshot");
    assert_eq!(multi_seen(&state),before);
    request(&state,Request::Undo);
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().revision,1);
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text,"def\n");
    assert!(!state.lock().unwrap().workspace.can_undo());
}

#[test]
fn m05_second_document_bad_scalar_whole_refusal() {
    let root=project("m05",&[("main.tex",b"abc"),("part.tex","B😀".as_bytes())]);
    let state=runtime(&root); open(&state,"main.tex"); open(&state,"part.tex"); let before=multi_seen(&state);
    let result=state.lock().unwrap().workspace.apply_edit_group(vec![
        ("main.tex".into(),1,vec![TextEdit{start:Position{line:0,character:0},end:Position{line:0,character:1},text:"A".into()}]),
        ("part.tex".into(),1,vec![TextEdit{start:Position{line:0,character:2},end:Position{line:0,character:3},text:"x".into()}])],"agent");
    assert_eq!(code(result),"invalid_position"); assert_eq!(multi_seen(&state),before);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"abc");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),"B😀".as_bytes());
}

#[test]
fn m06_foreign_owner_refuses() {
    let (root,state,gate)=multi_setup("m06"); let foreign=runtime(&root);
    open(&foreign,"main.tex"); open(&foreign,"part.tex"); let other=RuntimeTransactionGate::new(&foreign);
    let before=multi_seen(&state); let other_before=multi_seen(&foreign);
    assert_eq!(code(other.commit_multi(multi_plan(&gate,&state),"agent")),"runtime_transaction_owner_mismatch");
    assert_eq!(multi_seen(&state),before); assert_eq!(multi_seen(&foreign),other_before);
}

#[test]
fn m07_lock_contention_refuses() {
    let (_,state,gate)=multi_setup("m07"); let bound=multi_plan(&gate,&state); let before=multi_seen(&state);
    let guard=state.lock().unwrap(); let worker_gate=gate.clone(); let(send,recv)=mpsc::channel();
    let worker=thread::spawn(move||{let started=Instant::now();let code=code(worker_gate.commit_multi(bound,"agent"));send.send((code,started.elapsed())).unwrap();});
    let outcome=recv.recv_timeout(Duration::from_millis(250)); drop(guard); worker.join().unwrap();
    let(code,elapsed)=outcome.expect("try_lock must not wait for holder unlock");
    assert_eq!(code,"runtime_busy"); assert!(elapsed<Duration::from_millis(250));
    assert_eq!(multi_seen(&state),before); println!("P02_MULTI_CONTENTION_NS {}",elapsed.as_nanos());
}

#[test]
fn m08_dirty_disk_baseline_mismatch() {
    let (root,state,gate)=multi_setup("m08");
    request(&state,Request::ApplyEdits{document_id:"main.tex".into(),expected_revision:1,
        edits:vec![TextEdit{start:Position{line:0,character:0},end:Position{line:0,character:1},text:"dirty".into()}]});
    fs::write(root.join("main.tex"),b"foreign disk").unwrap();
    gate.commit_multi(multi_plan(&gate,&state),"agent").unwrap();
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text,"Airtybc\n");
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text,"Def\n");
    let before=multi_seen(&state);
    let reply=state.lock().unwrap().dispatch(Request::SaveDocument{document_id:"main.tex".into(),expected_revision:3},"user");
    assert_eq!(reply.error.unwrap().code,"external_change"); assert_eq!(multi_seen(&state),before);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"foreign disk");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),b"def\n");
}

#[test]
fn m09_explicit_save_then_common_undo() {
    let (root,state,gate)=multi_setup("m09"); gate.commit_multi(multi_plan(&gate,&state),"agent").unwrap();
    request(&state,Request::SaveDocument{document_id:"main.tex".into(),expected_revision:2});
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"Abc\n");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),b"def\n");
    request(&state,Request::Undo);
    let a=state.lock().unwrap().workspace.snapshot("main.tex").unwrap();
    let b=state.lock().unwrap().workspace.snapshot("part.tex").unwrap();
    assert_eq!(a.text,"abc\n"); assert_eq!(b.text,"def\n"); assert_eq!((a.revision,b.revision),(3,3));
    assert!(a.dirty); assert!(!b.dirty); assert!(!state.lock().unwrap().workspace.can_undo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(),b"Abc\n");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(),b"def\n");
}

#[test]
fn m10_legacy_single_document_compatibility() {
    let (_,state,gate)=multi_setup("m10");
    let single=gate.prepare(vec![commit_edit("main.tex",1,0,1,"A")]).unwrap();
    let result=gate.commit(single,"agent").unwrap(); assert_eq!(result.revision,2); assert_eq!(result.id,"main.tex");
    let before=multi_seen(&state);
    assert_eq!(code(gate.commit(multi_plan(&gate,&state),"agent")),"transaction_single_document_required");
    assert_eq!(multi_seen(&state),before);
    request(&state,Request::Undo);
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text,"abc\n");
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().revision,1);
    assert!(!state.lock().unwrap().workspace.can_undo()); request(&state,Request::Redo);
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text,"Abc\n");
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text,"def\n");
}

// The memory/history contract is shared. Persistent recovery's existing
// platform refusal remains explicit rather than being treated as success.
#[test]
fn m03_portable_common_redo_group() {
    let (root, state, gate) = multi_setup("m03-memory");
    gate.commit_multi(multi_plan(&gate, &state), "agent").unwrap();
    request(&state, Request::Undo);
    for (id, text) in [("main.tex", "abc\n"), ("part.tex", "def\n")] {
        let document = state.lock().unwrap().workspace.snapshot(id).unwrap();
        assert_eq!(document.text, text);
        assert_eq!(document.revision, 3);
        assert_eq!(document.last_origin, "undo:agent");
    }
    request(&state, Request::Redo);
    for (id, text) in [("main.tex", "Abc\n"), ("part.tex", "Def\n")] {
        let document = state.lock().unwrap().workspace.snapshot(id).unwrap();
        assert_eq!(document.text, text);
        assert_eq!(document.revision, 4);
        assert_eq!(document.last_origin, "redo:agent");
    }
    assert!(!state.lock().unwrap().workspace.can_redo());
    assert!(state.lock().unwrap().workspace.can_undo());
    request(&state, Request::Undo);
    assert_eq!(state.lock().unwrap().workspace.snapshot("main.tex").unwrap().text, "abc\n");
    assert_eq!(state.lock().unwrap().workspace.snapshot("part.tex").unwrap().text, "def\n");
    assert!(!state.lock().unwrap().workspace.can_undo());
    assert!(state.lock().unwrap().workspace.can_redo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc\n");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(), b"def\n");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn m03_recovery_refuses_unconfirmed_platform() {
    let root = project("m03-unconfirmed", &[("main.tex", b"abc\n"), ("part.tex", b"def\n")]);
    let storage = root.parent().unwrap().join("m03-unsupported-recovery");
    assert!(!storage.exists(), "fresh unsupported-platform fixture");
    let error = match Workspace::open_recoverable(root.clone(), storage.clone()) {
        Err(error) => error,
        Ok(_) => panic!("unconfirmed persistent recovery must refuse"),
    };
    assert_eq!(error.code, "recovery_unconfirmed_platform");
    assert!(!storage.exists(), "refusal must not initialize persistent storage");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc\n");
    assert_eq!(fs::read(root.join("part.tex")).unwrap(), b"def\n");
}
