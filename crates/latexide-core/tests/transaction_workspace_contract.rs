//! Actual public Workspace + canonical TransactionService bridge controls.
//! Fixture files are private to this cohort. No provider implementation is imported here.
//! Workspace entry: `python3 integration/registry/verify.py --profile shared`.
//! See CONTRIBUTING.md for mandatory caller-owned P02 fixture/nonce provisioning.
use latexide_core::{LineEnding, Position, TextEdit, Workspace};
use latexide_core::contracts::{RevisionedEdit, SnapshotLimits};
use latexide_core::contracts::transactions::TransactionError;
use latexide_core::services::transactions::{TransactionService, WorkspaceTransactionError};
use std::{fs, path::{Path, PathBuf}};

fn project(label: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let base = PathBuf::from(std::env::var_os("P02_WORKSPACE_FIXTURES").expect("own T9 fixture root"));
    let image = std::env::current_exe().expect("own test image");
    let image = image.file_stem().and_then(|name| name.to_str()).expect("own test image name");
    let pid = std::process::id();
    let root = base.join(image).join(pid.to_string()).join(label);
    assert!(!root.exists(), "fresh fixture, no case replay");
    fs::create_dir_all(&root).unwrap();
    for (name, bytes) in files {
        fs::write(root.join(name), bytes).unwrap();
    }
    eprintln!("P02_WORKSPACE_FIXTURE nonce={} pid={pid} image={image} label={label} root={}",
        std::env::var("P02_RUN_NONCE").unwrap(), root.display());
    root
}

fn open(root: &Path) -> Workspace {
    let workspace = Workspace::open(root.to_path_buf()).unwrap();
    eprintln!("P02_WORKSPACE_OPEN nonce={} root={}",
        std::env::var("P02_RUN_NONCE").unwrap(), root.display());
    workspace
}

fn service() -> TransactionService {
    TransactionService::new(SnapshotLimits {
        max_files: 128,
        max_file_bytes: latexide_core::MAX_FILE_BYTES,
        max_total_bytes: 8 * latexide_core::MAX_FILE_BYTES,
    })
}

fn edit(id: &str, revision: u64, start: u32, length: u32, replacement: &str) -> RevisionedEdit {
    RevisionedEdit {
        document_id: id.into(), expected_revision: revision,
        start_utf16: start, length_utf16: length, replacement: replacement.into(),
    }
}

fn change(workspace: &mut Workspace, id: &str, end: u32, text: &str) {
    let revision = workspace.snapshot(id).unwrap().revision;
    workspace.apply_edits(id, revision, vec![TextEdit {
        start: Position { line: 0, character: 0 },
        end: Position { line: 0, character: end },
        text: text.into(),
    }], "agent").unwrap();
}

fn host_error(error: WorkspaceTransactionError, code: &str) {
    match error {
        WorkspaceTransactionError::Workspace(error) => assert_eq!(error.code, code),
        other => panic!("expected host {code}, got {other:?}"),
    }
}

fn plan_error(error: WorkspaceTransactionError, expected: TransactionError) {
    match error {
        WorkspaceTransactionError::Transaction(error) => assert_eq!(error, expected),
        other => panic!("expected planning {expected:?}, got {other:?}"),
    }
}

#[test]
fn w01_fresh_plan_from_actual_workspace() {
    let root = project("w01-fresh", &[("main.tex", b"abc")]);
    let mut workspace = open(&root);
    let document = workspace.open_document("main.tex").unwrap();
    let owner = workspace.transaction_snapshot().unwrap();
    for unsupported in ["draft?.tex", "trailing.", "bad:name.tex"] {
        #[cfg(target_os = "macos")]
        fs::write(root.join(unsupported), b"retained unsupported file").unwrap();
        let error = workspace.open_document(unsupported).unwrap_err();
        assert_eq!(error.code, "invalid_path");
        assert!(error.message.contains(unsupported));
        assert_eq!(workspace.transaction_snapshot().unwrap(), owner);
    }
    let edits = vec![edit("main.tex", document.revision, 0, 1, "A")];
    let port = service();
    let prepared = port.prepare_workspace_transaction(&workspace, edits.clone()).unwrap();
    assert_eq!(prepared.snapshot(), &owner);
    assert_eq!(prepared.plan().base_snapshot, owner.id);
    assert_eq!(prepared.plan().edits, edits);
    port.revalidate_workspace_transaction(&workspace, &prepared).unwrap();
    assert_eq!(workspace.transaction_snapshot().unwrap(), owner);
    assert_eq!(workspace.snapshot("main.tex").unwrap().text, "abc");
    assert!(!workspace.can_undo());
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn w02_two_open_documents_bind_complete_plan() {
    let root = project("w02-two", &[("a.tex", b"abc"), ("b.tex", b"def")]);
    let mut workspace = open(&root);
    let b = workspace.open_document("b.tex").unwrap();
    let a = workspace.open_document("a.tex").unwrap();
    let port = service();
    let edits = vec![edit("b.tex", b.revision, 0, 1, "D"), edit("a.tex", a.revision, 0, 1, "A")];
    let prepared = port.prepare_workspace_transaction(&workspace, edits.clone()).unwrap();
    assert_eq!(prepared.snapshot().files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a.tex", "b.tex"]);
    assert_eq!(prepared.snapshot(), &workspace.transaction_snapshot().unwrap());
    assert_eq!(prepared.plan().edits, edits);
    port.revalidate_workspace_transaction(&workspace, &prepared).unwrap();
    assert_eq!(workspace.snapshot("a.tex").unwrap().text, "abc");
    assert_eq!(workspace.snapshot("b.tex").unwrap().text, "def");
    assert_eq!(fs::read(root.join("a.tex")).unwrap(), b"abc");
    assert_eq!(fs::read(root.join("b.tex")).unwrap(), b"def");
}

#[test]
fn w03_stale_edit_and_changed_owner_refuse() {
    let root = project("w03-stale", &[("main.tex", b"abc")]);
    let mut workspace = open(&root);
    let document = workspace.open_document("main.tex").unwrap();
    let before = workspace.transaction_snapshot().unwrap();
    let port = service();
    plan_error(port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", document.revision - 1, 0, 1, "X")]).unwrap_err(), TransactionError::StaleRevision);
    assert_eq!(workspace.transaction_snapshot().unwrap(), before);
    let old = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", document.revision, 0, 1, "A")]).unwrap();
    change(&mut workspace, "main.tex", 3, "xyz");
    host_error(port.revalidate_workspace_transaction(&workspace, &old).unwrap_err(), "stale_snapshot");
    let current = workspace.snapshot("main.tex").unwrap();
    let fresh = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", current.revision, 0, 1, "X")]).unwrap();
    port.revalidate_workspace_transaction(&workspace, &fresh).unwrap();
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"abc");
}

#[test]
fn w04_same_root_reopen_rejects_aba() {
    let root = project("w04-aba", &[("main.tex", b"abc")]);
    let mut first = open(&root);
    let document = first.open_document("main.tex").unwrap();
    let port = service();
    let old = port.prepare_workspace_transaction(&first,
        vec![edit("main.tex", document.revision, 0, 1, "A")]).unwrap();
    drop(first);
    let mut reopened = open(&root);
    let document = reopened.open_document("main.tex").unwrap();
    let owner = reopened.transaction_snapshot().unwrap();
    assert_eq!(owner.files, old.snapshot().files);
    assert_eq!(owner.epoch, old.snapshot().epoch);
    assert_ne!(owner.id, old.snapshot().id);
    host_error(port.revalidate_workspace_transaction(&reopened, &old).unwrap_err(), "stale_snapshot");
    let fresh = port.prepare_workspace_transaction(&reopened,
        vec![edit("main.tex", document.revision, 0, 1, "A")]).unwrap();
    port.revalidate_workspace_transaction(&reopened, &fresh).unwrap();
}

#[test]
fn w05_foreign_workspace_rejects_identical_content() {
    let first_root = project("w05-source", &[("main.tex", b"abc")]);
    let second_root = project("w05-foreign", &[("main.tex", b"abc")]);
    let mut first = open(&first_root);
    let mut foreign = open(&second_root);
    let document = first.open_document("main.tex").unwrap();
    foreign.open_document("main.tex").unwrap();
    let port = service();
    let prepared = port.prepare_workspace_transaction(&first,
        vec![edit("main.tex", document.revision, 0, 1, "A")]).unwrap();
    let foreign_owner = foreign.transaction_snapshot().unwrap();
    assert_eq!(foreign_owner.files, prepared.snapshot().files);
    assert_eq!(foreign_owner.epoch, prepared.snapshot().epoch);
    assert_ne!(foreign_owner.id, prepared.snapshot().id);
    host_error(port.revalidate_workspace_transaction(&foreign, &prepared).unwrap_err(), "stale_snapshot");
    port.revalidate_workspace_transaction(&first, &prepared).unwrap();
}

#[test]
fn w06_astral_utf16_preserves_physical_encoding() {
    let mut disk = vec![0xef, 0xbb, 0xbf];
    disk.extend_from_slice("A😀\u{feff}\r\nB\r\n".as_bytes());
    let root = project("w06-unicode", &[("main.tex", &disk)]);
    let mut workspace = open(&root);
    let document = workspace.open_document("main.tex").unwrap();
    assert!(document.bom);
    assert_eq!(document.line_ending, LineEnding::Crlf);
    let port = service();
    plan_error(port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", document.revision, 2, 0, "X")]).unwrap_err(), TransactionError::InvalidRange);
    let prepared = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", document.revision, 1, 2, "🌍")]).unwrap();
    assert_eq!(prepared.snapshot().files[0].utf8, "A😀\u{feff}\nB\n".as_bytes());
    assert_eq!(prepared.plan().edits[0].start_utf16, 1);
    assert_eq!(prepared.plan().edits[0].length_utf16, 2);
    port.revalidate_workspace_transaction(&workspace, &prepared).unwrap();
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
}

#[test]
fn w07_capture_and_projected_quota_refuse_without_mutation() {
    let root = project("w07-quota", &[("a.tex", b"abc"), ("b.tex", b"def")]);
    let mut workspace = open(&root);
    let a = workspace.open_document("a.tex").unwrap();
    let b = workspace.open_document("b.tex").unwrap();
    let before = workspace.transaction_snapshot().unwrap();
    let one_file = TransactionService::new(SnapshotLimits { max_files: 1, max_file_bytes: 4, max_total_bytes: 8 });
    host_error(one_file.prepare_workspace_transaction(&workspace, vec![]).unwrap_err(), "snapshot_quota");
    let small_total = TransactionService::new(SnapshotLimits { max_files: 2, max_file_bytes: 4, max_total_bytes: 5 });
    host_error(small_total.prepare_workspace_transaction(&workspace, vec![]).unwrap_err(), "snapshot_quota");
    let strict = TransactionService::new(SnapshotLimits { max_files: 2, max_file_bytes: 4, max_total_bytes: 7 });
    let edits = vec![edit("a.tex", a.revision, 3, 0, "x"), edit("b.tex", b.revision, 3, 0, "y")];
    plan_error(strict.prepare_workspace_transaction(&workspace, edits.clone()).unwrap_err(), TransactionError::Quota);
    let exact = TransactionService::new(SnapshotLimits { max_files: 2, max_file_bytes: 4, max_total_bytes: 8 });
    let prepared = exact.prepare_workspace_transaction(&workspace, edits).unwrap();
    exact.revalidate_workspace_transaction(&workspace, &prepared).unwrap();
    assert_eq!(workspace.transaction_snapshot().unwrap(), before);
    assert_eq!(fs::read(root.join("a.tex")).unwrap(), b"abc");
    assert_eq!(fs::read(root.join("b.tex")).unwrap(), b"def");
    assert!(!workspace.can_undo());
}

#[test]
fn w08_save_then_undo_invalidates_and_refreshes_plan() {
    let root = project("w08-save-undo", &[("main.tex", b"abc")]);
    let mut workspace = open(&root);
    workspace.open_document("main.tex").unwrap();
    change(&mut workspace, "main.tex", 3, "saved");
    let port = service();
    let dirty = workspace.snapshot("main.tex").unwrap();
    let before_save = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", dirty.revision, 0, 0, "X")]).unwrap();
    let saved = workspace.save_document("main.tex", dirty.revision).unwrap();
    assert_eq!(saved.revision, dirty.revision);
    assert_eq!(saved.text, dirty.text);
    assert!(!saved.dirty);
    assert!(workspace.can_undo());
    host_error(port.revalidate_workspace_transaction(&workspace, &before_save).unwrap_err(), "stale_snapshot");
    let saved_plan = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", saved.revision, 0, 0, "X")]).unwrap();
    port.revalidate_workspace_transaction(&workspace, &saved_plan).unwrap();
    assert!(saved_plan.snapshot().epoch > before_save.snapshot().epoch);
    let undone = workspace.undo().unwrap();
    assert_eq!(undone.text, "abc");
    assert!(undone.dirty);
    assert!(undone.revision > saved.revision);
    assert!(workspace.can_redo());
    host_error(port.revalidate_workspace_transaction(&workspace, &saved_plan).unwrap_err(), "stale_snapshot");
    let fresh = port.prepare_workspace_transaction(&workspace,
        vec![edit("main.tex", undone.revision, 0, 1, "A")]).unwrap();
    port.revalidate_workspace_transaction(&workspace, &fresh).unwrap();
    assert_eq!(fresh.snapshot().files[0].utf8, b"abc");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"saved");

    // Accepted maximum origins, including a scalar cut by the label budget,
    // must remain undoable. Legacy single-document history is covered too.
    for grouped in [true, false] {
        let mut origins = vec!["a".repeat(16384), "é".repeat(8192)];
        if !grouped { origins.push("legacy".repeat(3000)); }
        for origin in origins {
            let before = workspace.snapshot("main.tex").unwrap();
            let edits = vec![TextEdit {start: Position {line: 0, character: 0},
                end: Position {line: 0, character: 0}, text: "x".into()}];
            if grouped {
                workspace.apply_edit_group(vec![("main.tex".into(), before.revision, edits)], &origin).unwrap();
            } else {
                workspace.apply_edits("main.tex", before.revision, edits, &origin).unwrap();
            }
            assert_eq!(workspace.snapshot("main.tex").unwrap().last_origin, origin);
            let undo = workspace.undo().unwrap();
            assert_eq!(undo.text, before.text);
            assert!(undo.last_origin.starts_with("undo:") && undo.last_origin.len() <= 16384);
            assert!(origin.starts_with(&undo.last_origin[5..]));
            let redo = workspace.redo().unwrap();
            assert_eq!(redo.text, format!("x{}", before.text));
            assert!(redo.last_origin.starts_with("redo:") && redo.last_origin.len() <= 16384);
            assert!(origin.starts_with(&redo.last_origin[5..]));
            // A second cycle still derives from the unchanged original origin.
            assert_eq!(workspace.undo().unwrap().last_origin, undo.last_origin);
            assert_eq!(workspace.redo().unwrap().last_origin, redo.last_origin);
        }
    }
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"saved");
}
