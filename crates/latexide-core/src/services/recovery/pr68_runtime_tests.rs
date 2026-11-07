//! PR68 own sessionless Runtime regression/characterization controls.
//! These tests never start Session, IPC, C callers, GUI or a compiler.
use super::Runtime;
use latexide_core::{Position, RecoveryObserver, RecoveryPhase, TextEdit, Workspace};
use latexide_protocol::{Reply, Request};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}
fn nonce() -> String {
    run_fixture().nonce.clone()
}
struct RunFixture {
    task_root: PathBuf,
    cases: PathBuf,
    nonce: String,
}
fn run_fixture() -> &'static RunFixture {
    static FIXTURE: OnceLock<RunFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let vars = ["P00_PR68_TASK_ROOT", "P00_PR68_ROOT", "P00_PR68_NONCE"].map(std::env::var_os);
        if vars.iter().all(Option::is_none) {
            // Bare Cargo gets a real exclusive private fixture, never a skip.
            let mut bytes = [0u8; 32];
            fs::File::open("/dev/urandom")
                .unwrap()
                .read_exact(&mut bytes)
                .unwrap();
            let nonce = hex(&bytes);
            let task_root = std::env::temp_dir()
                .canonicalize()
                .unwrap()
                .join(format!("pr68-standalone-{}-{nonce}", std::process::id()));
            fs::create_dir(&task_root).unwrap();
            fs::set_permissions(&task_root, fs::Permissions::from_mode(0o700)).unwrap();
            let tmp = task_root.join("tmp");
            fs::create_dir(&tmp).unwrap();
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o700)).unwrap();
            let cases = tmp.join("cases");
            fs::create_dir(&cases).unwrap();
            fs::set_permissions(&cases, fs::Permissions::from_mode(0o700)).unwrap();
            RunFixture {
                task_root,
                cases,
                nonce,
            }
        } else {
            assert!(
                vars.iter().all(Option::is_some),
                "partial F05 harness environment; all three variables are required"
            );
            RunFixture {
                task_root: PathBuf::from(vars[0].as_ref().unwrap())
                    .canonicalize()
                    .unwrap(),
                cases: PathBuf::from(vars[1].as_ref().unwrap())
                    .canonicalize()
                    .unwrap(),
                nonce: vars[2]
                    .as_ref()
                    .unwrap()
                    .to_str()
                    .expect("ASCII nonce")
                    .to_owned(),
            }
        }
    })
}

struct CaseWatch {
    done: Arc<(Mutex<bool>, std::sync::Condvar)>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl CaseWatch {
    fn start() -> Self {
        let done = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let child = done.clone();
        let join = std::thread::spawn(move || {
            let (lock, wake) = &*child;
            let guard = lock.lock().unwrap();
            let (guard, wait) = wake
                .wait_timeout_while(guard, Duration::from_secs(9), |done| !*done)
                .unwrap();
            if wait.timed_out() && !*guard {
                std::process::exit(124)
            }
        });
        Self {
            done,
            join: Some(join),
        }
    }
}
impl Drop for CaseWatch {
    fn drop(&mut self) {
        let (lock, wake) = &*self.done;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        self.join.take().unwrap().join().unwrap();
    }
}

fn fixture(id: &str, body: &[u8]) -> (PathBuf, PathBuf, CaseWatch) {
    let n = nonce();
    assert!(n.len() == 64 && n.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        std::any::type_name::<Runtime>(),
        "latexide_ffi::runtime::Runtime"
    );
    let root = &run_fixture().task_root;
    let parent = &run_fixture().cases;
    assert!(parent.starts_with(root.join("tmp")));
    let index = id
        .strip_prefix('r')
        .and_then(|v| v.parse::<u8>().ok())
        .expect("known case");
    assert!((1..=26).contains(&index) && id == format!("r{index:02}"));
    let base = parent.join(id);
    fs::create_dir(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    let watch = CaseWatch::start();
    println!("PR68_CASE\t{n}\t{id}");
    let project = base.join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("main.tex"), body).unwrap();
    (base, project, watch)
}
fn runtime(project: &Path) -> Runtime {
    let r = Runtime::open_project(project.to_path_buf()).unwrap();
    assert!(r.session_descriptor.as_os_str().is_empty());
    assert!(r.workspace.recovery_status().enabled);
    r
}
fn call(r: &mut Runtime, q: Request) -> Reply {
    let reply = r.dispatch(q, "agent");
    let raw = serde_json::to_vec(&reply).unwrap();
    if raw.len() <= 8192 {
        println!(
            "PR68_REPLY\t{}\t{}",
            nonce(),
            std::str::from_utf8(&raw).unwrap()
        );
    } else {
        println!(
            "PR68_REPLY_DIGEST\t{}\t{}\t{}\t{}",
            nonce(),
            reply.ok,
            raw.len(),
            latexide_core::recovery::durable::raw_digest(&raw)
        );
    }
    reply
}
fn ok(r: &mut Runtime, q: Request) -> serde_json::Value {
    let reply = call(r, q);
    assert!(reply.ok, "expected success, got {:?}", reply.error);
    reply.result.unwrap()
}
fn open(r: &mut Runtime) -> u64 {
    ok(
        r,
        Request::OpenDocument {
            path: "main.tex".into(),
        },
    )["revision"]
        .as_u64()
        .unwrap()
}
fn edit(r: &mut Runtime, rev: u64, text: &str) -> u64 {
    let before = r.workspace.snapshot("main.tex").unwrap();
    assert!(
        !before.text.contains('\n'),
        "single-line replacement helper"
    );
    ok(
        r,
        Request::ApplyEdits {
            document_id: "main.tex".into(),
            expected_revision: rev,
            edits: vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: before.text.encode_utf16().count() as u32,
                },
                text: text.into(),
            }],
        },
    )["revision"]
        .as_u64()
        .unwrap()
}
fn save(r: &mut Runtime, rev: u64) -> Reply {
    call(
        r,
        Request::SaveDocument {
            document_id: "main.tex".into(),
            expected_revision: rev,
        },
    )
}
fn state(r: &Runtime, label: &str) {
    let d = r.workspace.snapshot("main.tex").unwrap();
    let body = d.text.as_bytes();
    let digest = latexide_core::recovery::durable::raw_digest(body);
    println!(
        "PR68_STATE\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        nonce(),
        label,
        hex(d.id.as_bytes()),
        d.revision,
        d.dirty,
        digest,
        hex(body)
    );
}
fn disk(p: &Path, label: &str) {
    let b = fs::read(p).unwrap();
    println!(
        "PR68_DISK\t{}\t{}\t{}\t{}",
        nonce(),
        label,
        hex(p.to_str().unwrap().as_bytes()),
        hex(&b)
    )
}
static REPLACEMENT: AtomicUsize = AtomicUsize::new(0);
fn replace_file(project: &Path, body: &[u8]) {
    let p = project.join("main.tex");
    let before = fs::metadata(&p).unwrap();
    let retired = project.join(format!(
        "retained-main-{}.tex",
        REPLACEMENT.fetch_add(1, Ordering::SeqCst)
    ));
    fs::rename(&p, retired).unwrap();
    fs::write(&p, body).unwrap();
    let after = fs::metadata(&p).unwrap();
    assert_ne!((before.dev(), before.ino()), (after.dev(), after.ino()));
    disk(&p, "replacement");
}
fn current(storage: &Path) -> PathBuf {
    let p = fs::read(storage.join("current")).unwrap();
    assert!(p.len() > 40);
    let n = u64::from_le_bytes(p[32..40].try_into().unwrap()) as usize;
    let name = std::str::from_utf8(&p[40..40 + n]).unwrap();
    assert!(name.len() == 39 && name.starts_with("cohort-"));
    storage.join(name)
}
fn own_file(path: &Path, body: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(body).unwrap();
    f.sync_all().unwrap()
}
fn assert_error(reply: Reply, code: &str) {
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, code)
}
#[test]
fn r01_identical_replacement_explicit_reopen_save_preserves_acknowledged_undo() {
    let (_, p, _case_watch) = fixture("r01", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "acknowledged");
    state(&r, "before-replacement");
    replace_file(&p, b"old");
    assert_error(save(&mut r, rev), "external_change");
    let seq = r.workspace.recovery_status().sequence;
    assert_eq!(open(&mut r), rev);
    assert_eq!(
        r.workspace.recovery_status().sequence,
        seq + 1,
        "explicit reopen must durably rebind only identical saved bytes"
    );
    state(&r, "after-reopen");
    let reply = save(&mut r, rev);
    assert!(
        reply.ok,
        "identical replacement + explicit reopen has no Save resolution: {:?}",
        reply.error
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"acknowledged");
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert!(r.workspace.snapshot("main.tex").unwrap().dirty);
    state(&r, "undo-after-save");
    disk(&p.join("main.tex"), "after-undo");
    drop(r);
    let r = runtime(&p);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert!(r.workspace.can_redo());
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"acknowledged");
}
#[test]
fn r02_restored_dirty_buffer_reopen_rebinds_identical_file_without_discard() {
    let (_, p, _case_watch) = fixture("r02", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    drop(r);
    replace_file(&p, b"old");
    let mut r = runtime(&p);
    assert!(r.workspace.recovery_status().restored);
    assert_error(save(&mut r, rev), "external_change");
    assert_eq!(open(&mut r), rev);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    state(&r, "restored-and-reopened");
    assert!(
        save(&mut r, rev).ok,
        "explicit reopen must resolve the same-byte file replacement"
    );
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"dirty");
}
#[test]
fn r03_clean_reopen_persists_identity_without_revision_origin_or_history_reset() {
    let (_, p, _case_watch) = fixture("r03", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "saved");
    assert!(save(&mut r, rev).ok);
    let before = serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap();
    let seq = r.workspace.recovery_status().sequence;
    replace_file(&p, b"saved");
    assert_eq!(open(&mut r), rev);
    assert_eq!(
        serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap(),
        before
    );
    assert_eq!(r.workspace.recovery_status().sequence, seq + 1);
    assert!(r.workspace.can_undo());
    drop(r);
    let mut r = runtime(&p);
    assert!(save(&mut r, rev).ok);
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert!(r.workspace.snapshot("main.tex").unwrap().dirty);
    disk(&p.join("main.tex"), "saved-after-undo");
}
#[test]
fn r04_different_disk_bytes_explicit_reopen_never_grants_overwrite() {
    let (_, p, _case_watch) = fixture("r04", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    replace_file(&p, b"foreign");
    let seq = r.workspace.recovery_status().sequence;
    assert_eq!(open(&mut r), rev);
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_error(save(&mut r, rev), "external_change");
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"foreign");
    state(&r, "conflict-preserved");
}
#[test]
fn r05_equivalent_logical_text_with_different_bom_eol_is_not_an_identical_baseline() {
    let original = "\u{feff}old\r\n";
    let (_, p, _case_watch) = fixture("r05", original.as_bytes());
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = ok(
        &mut r,
        Request::ApplyEdits {
            document_id: "main.tex".into(),
            expected_revision: rev,
            edits: vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 3,
                },
                text: "dirty".into(),
            }],
        },
    )["revision"]
        .as_u64()
        .unwrap();
    replace_file(&p, b"old\n");
    let seq = r.workspace.recovery_status().sequence;
    assert_eq!(open(&mut r), rev);
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_error(save(&mut r, rev), "external_change");
    let d = r.workspace.snapshot("main.tex").unwrap();
    assert!(d.bom && matches!(d.line_ending, latexide_core::LineEnding::Crlf));
    assert_eq!(d.text, "dirty\n");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old\n");
    state(&r, "physical-conflict");
}
#[test]
fn r06_outside_symlink_cannot_rebind_or_be_saved() {
    let (base, p, _case_watch) = fixture("r06", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    fs::rename(p.join("main.tex"), p.join("retained-main.tex")).unwrap();
    let outside = base.join("outside-project.tex");
    fs::write(&outside, b"old").unwrap();
    symlink(&outside, p.join("main.tex")).unwrap();
    assert_error(
        call(
            &mut r,
            Request::OpenDocument {
                path: "main.tex".into(),
            },
        ),
        "path_escape",
    );
    assert_error(save(&mut r, rev), "path_escape");
    assert_eq!(fs::read(&outside).unwrap(), b"old");
    state(&r, "symlink-refused");
    disk(&outside, "outside-unchanged");
}
struct Fault {
    armed: Arc<AtomicBool>,
    phase: RecoveryPhase,
}
impl RecoveryObserver for Fault {
    fn phase(&mut self, p: RecoveryPhase) -> std::io::Result<()> {
        if p == self.phase && self.armed.load(Ordering::SeqCst) {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    }
}
fn observed(p: &Path, phase: RecoveryPhase) -> (Runtime, Arc<AtomicBool>) {
    let armed = Arc::new(AtomicBool::new(false));
    let w = Workspace::open_recoverable_with_observer(
        p.to_path_buf(),
        p.join(".latexide/recovery"),
        Box::new(Fault {
            armed: armed.clone(),
            phase,
        }),
    )
    .unwrap();
    let r = Runtime::new(w);
    assert!(r.session_descriptor.as_os_str().is_empty());
    (r, armed)
}
#[test]
fn r07_rebind_before_publish_failure_rolls_back_identity_text_history_and_sequence() {
    let (_, p, _case_watch) = fixture("r07", b"old");
    let (mut r, armed) = observed(&p, RecoveryPhase::BeforePublish);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    replace_file(&p, b"old");
    let before = serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap();
    let seq = r.workspace.recovery_status().sequence;
    armed.store(true, Ordering::SeqCst);
    assert_error(
        call(
            &mut r,
            Request::OpenDocument {
                path: "main.tex".into(),
            },
        ),
        "recovery_io",
    );
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(
        serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap(),
        before
    );
    assert_error(save(&mut r, rev), "external_change");
    armed.store(false, Ordering::SeqCst);
    assert_eq!(open(&mut r), rev);
    assert!(save(&mut r, rev).ok);
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"dirty");
}
#[test]
fn r08_rebind_uncertain_publication_blocks_mutation_until_complete_state_reopen() {
    let (_, p, _case_watch) = fixture("r08", b"old");
    let (mut r, armed) = observed(&p, RecoveryPhase::SelectorRenamed);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    replace_file(&p, b"old");
    armed.store(true, Ordering::SeqCst);
    assert_error(
        call(
            &mut r,
            Request::OpenDocument {
                path: "main.tex".into(),
            },
        ),
        "recovery_commit_uncertain",
    );
    assert!(r.workspace.recovery_status().blocked);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    assert_error(save(&mut r, rev), "recovery_commit_uncertain");
    state(&r, "uncertain-complete");
    drop(r);
    let mut r = runtime(&p);
    assert!(!r.workspace.recovery_status().blocked);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().revision, rev);
    assert!(save(&mut r, rev).ok);
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
}
#[test]
fn r09_second_replacement_after_explicit_rebind_still_requires_a_new_reopen() {
    let (_, p, _case_watch) = fixture("r09", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    replace_file(&p, b"old");
    let seq = r.workspace.recovery_status().sequence;
    open(&mut r);
    assert_eq!(r.workspace.recovery_status().sequence, seq + 1);
    replace_file(&p, b"old");
    assert_error(save(&mut r, rev), "external_change");
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    state(&r, "second-replacement-refused");
}
#[test]
fn r10_rebind_does_not_make_stale_revision_edit_or_save_valid() {
    let (_, p, _case_watch) = fixture("r10", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    replace_file(&p, b"old");
    let seq = r.workspace.recovery_status().sequence;
    assert_eq!(open(&mut r), rev);
    assert_eq!(r.workspace.recovery_status().sequence, seq + 1);
    assert_error(save(&mut r, rev - 1), "revision_conflict");
    assert_error(
        call(
            &mut r,
            Request::ApplyEdits {
                document_id: "main.tex".into(),
                expected_revision: rev - 1,
                edits: vec![],
            },
        ),
        "revision_conflict",
    );
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    assert_eq!(r.workspace.recovery_status().sequence, seq + 1);
}
#[test]
fn r11_project_rename_restores_acknowledged_state_and_history() {
    let (base, p, _case_watch) = fixture("r11", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    edit(&mut r, rev, "acknowledged");
    drop(r);
    let before = fs::metadata(&p).unwrap();
    let moved = base.join("moved-project");
    fs::rename(&p, &moved).unwrap();
    let after = fs::metadata(&moved).unwrap();
    assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
    let mut r = runtime(&moved);
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "acknowledged"
    );
    assert!(r.workspace.snapshot("main.tex").unwrap().dirty);
    assert_eq!(fs::read(moved.join("main.tex")).unwrap(), b"old");
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    drop(r);
    fs::rename(&moved, &p).unwrap();
    let mut r = runtime(&p);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    ok(&mut r, Request::Redo);
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "acknowledged"
    );
    state(&r, "original-location-recovered");
}
fn copy_tree(a: &Path, b: &Path) {
    fs::create_dir(b).unwrap();
    fs::set_permissions(b, fs::Permissions::from_mode(0o700)).unwrap();
    for e in fs::read_dir(a).unwrap() {
        let e = e.unwrap();
        let to = b.join(e.file_name());
        let kind = e.file_type().unwrap();
        assert!(!kind.is_symlink());
        if kind.is_dir() {
            copy_tree(&e.path(), &to)
        } else {
            fs::copy(e.path(), to).unwrap();
        }
    }
}
#[test]
fn r12_foreign_project_store_copy_is_not_accepted_despite_equal_document_bytes() {
    let (base, p, _case_watch) = fixture("r12", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    edit(&mut r, rev, "acknowledged");
    drop(r);
    let foreign = base.join("other-project");
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("main.tex"), b"old").unwrap();
    fs::create_dir(foreign.join(".latexide")).unwrap();
    fs::set_permissions(foreign.join(".latexide"), fs::Permissions::from_mode(0o700)).unwrap();
    copy_tree(
        &p.join(".latexide/recovery"),
        &foreign.join(".latexide/recovery"),
    );
    let e = match Runtime::open_project(foreign.clone()) {
        Ok(_) => panic!("foreign store accepted"),
        Err(e) => e,
    };
    assert_eq!(e.code, "recovery_corrupt");
    assert_eq!(fs::read(foreign.join("main.tex")).unwrap(), b"old");
    disk(&foreign.join("main.tex"), "foreign-project-unchanged");
    let r = runtime(&p);
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "acknowledged"
    );
}
fn foreign_entry_case(id: &str, name: &str, cohort: bool) {
    let (_, p, _case_watch) = fixture(id, b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "acknowledged");
    let storage = p.join(".latexide/recovery");
    let dir = if cohort {
        let q = storage.join(format!("cohort-{}", "f".repeat(32)));
        assert_ne!(q, current(&storage));
        fs::create_dir(&q).unwrap();
        fs::set_permissions(&q, fs::Permissions::from_mode(0o700)).unwrap();
        q
    } else {
        storage
    };
    let foreign = dir.join(name);
    own_file(&foreign, b"foreign-preserve");
    let seq = r.workspace.recovery_status().sequence;
    let reply = call(
        &mut r,
        Request::ApplyEdits {
            document_id: "main.tex".into(),
            expected_revision: rev,
            edits: vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 12,
                },
                text: "accepted".into(),
            }],
        },
    );
    assert!(reply.ok, "{:?}", reply.error);
    assert_eq!(r.workspace.recovery_status().sequence, seq + 1);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "accepted");
    assert_eq!(fs::read(&foreign).unwrap(), b"foreign-preserve");
    state(&r, "foreign-retained-with-edit-accepted");
    disk(&foreign, "foreign-retained");
    drop(r);
    let r = runtime(&p);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "accepted");
    assert!(r.workspace.can_undo());
    assert_eq!(fs::read(foreign).unwrap(), b"foreign-preserve");
}
#[test]
fn r13_unknown_root_file_is_retained_without_blocking_edits() {
    foreign_entry_case("r13", "notes.txt", false)
}
#[test]
fn r14_ds_store_is_retained_without_blocking_edits() {
    foreign_entry_case("r14", ".DS_Store", false)
}
#[test]
fn r15_unknown_unselected_cohort_is_retained_without_blocking_edits() {
    foreign_entry_case("r15", "notes.txt", true)
}
#[test]
fn r16_recovery_setup_failure_reports_explicit_in_memory_mode() {
    let (_, p, _case_watch) = fixture("r16", b"old");
    own_file(&p.join(".latexide"), b"foreign-blocking-file");
    let mut r = Runtime::open_project(p.clone()).unwrap();
    assert!(!r.workspace.recovery_status().enabled);
    assert_eq!(
        r.workspace.recovery_status().last_error.as_deref(),
        Some("recovery_storage")
    );
    let info = ok(&mut r, Request::WorkspaceInfo);
    assert_eq!(info["recovery"]["enabled"], false);
    assert_eq!(info["recovery"]["last_error"], "recovery_storage");
    let rev = open(&mut r);
    edit(&mut r, rev, "in-memory");
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "in-memory");
    assert_eq!(
        fs::read(p.join(".latexide")).unwrap(),
        b"foreign-blocking-file"
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    disk(&p.join(".latexide"), "setup-foreign-file-retained");
}
#[test]
fn r17_selected_corrupt_state_never_falls_back_to_valid_previous() {
    let (_, p, _case_watch) = fixture("r17", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "acknowledged");
    edit(&mut r, rev, "latest");
    drop(r);
    let storage = p.join(".latexide/recovery");
    let target = current(&storage).join("runtime-state.bin");
    let original = fs::read(&target).unwrap();
    own_file(&storage.join("preserved-selected-copy"), &original);
    let mut corrupt = original;
    corrupt[32] ^= 1;
    fs::write(&target, &corrupt).unwrap();
    let e = match Runtime::open_project(p.clone()) {
        Ok(_) => panic!("corrupt selected state silently accepted/fell back"),
        Err(e) => e,
    };
    assert_eq!(e.code, "recovery_corrupt");
    assert_eq!(fs::read(&target).unwrap(), corrupt);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    disk(&target, "selected-corrupt-retained");
}
#[test]
fn r18_selected_state_symlink_with_valid_bytes_is_refused_and_retained() {
    let (base, p, _case_watch) = fixture("r18", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    edit(&mut r, rev, "acknowledged");
    drop(r);
    let target = current(&p.join(".latexide/recovery")).join("runtime-state.bin");
    let bytes = fs::read(&target).unwrap();
    let retained = target.with_file_name("preserved-runtime-state.bin");
    fs::rename(&target, retained).unwrap();
    let outside = base.join("own-outside-state.bin");
    own_file(&outside, &bytes);
    symlink(&outside, &target).unwrap();
    let e = match Runtime::open_project(p.clone()) {
        Ok(_) => panic!("selected foreign symlink accepted"),
        Err(e) => e,
    };
    assert_eq!(e.code, "recovery_storage");
    assert!(
        fs::symlink_metadata(&target)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&outside).unwrap(), bytes);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    println!(
        "PR68_LIMIT\t{}\tselected_symlink\t{}\t{}",
        nonce(),
        e.code,
        e.message
    );
}
struct Counts(Arc<AtomicUsize>);
impl RecoveryObserver for Counts {
    fn phase(&mut self, p: RecoveryPhase) -> std::io::Result<()> {
        if p == RecoveryPhase::Published {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
fn latency_case(id: &str, size: usize) {
    let (_, p, _case_watch) = fixture(id, &vec![b'A'; size]);
    let commits = Arc::new(AtomicUsize::new(0));
    let w = Workspace::open_recoverable_with_observer(
        p.clone(),
        p.join(".latexide/recovery"),
        Box::new(Counts(commits.clone())),
    )
    .unwrap();
    let mut r = Runtime::new(w);
    let mut rev = open(&mut r);
    let prior = commits.load(Ordering::SeqCst);
    let r = Mutex::new(r);
    for (i, c) in ["B", "C", "D", "E"].iter().enumerate() {
        let mut guard = r.lock().unwrap();
        let start = Instant::now();
        let reply = guard.dispatch(
            Request::ApplyEdits {
                document_id: "main.tex".into(),
                expected_revision: rev,
                edits: vec![TextEdit {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 1,
                    },
                    text: (*c).into(),
                }],
            },
            "agent",
        );
        let nanos = start.elapsed().as_nanos();
        assert!(
            reply.ok,
            "measured real durable edit failed: {:?}",
            reply.error
        );
        rev = reply.result.unwrap()["revision"].as_u64().unwrap();
        let seq = guard.workspace.recovery_status().sequence;
        drop(guard);
        let selected = current(&p.join(".latexide/recovery"));
        let state_bytes = fs::metadata(selected.join("runtime-state.bin"))
            .unwrap()
            .len();
        let checkpoint_bytes = fs::metadata(selected.join("checkpoint-1.rec"))
            .unwrap()
            .len();
        println!(
            "PR68_LATENCY\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            nonce(),
            size,
            i,
            nanos,
            state_bytes,
            checkpoint_bytes,
            seq
        );
    }
    assert_eq!(
        commits.load(Ordering::SeqCst) - prior,
        4,
        "each changed edit still synchronously publishes before its successful Rust reply"
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), vec![b'A'; size]);
    let guard = r.lock().unwrap();
    assert!(guard.workspace.snapshot("main.tex").unwrap().dirty);
}
#[test]
fn r19_small_own_runtime_locked_edit_latency_is_descriptive() {
    latency_case("r19", 1024)
}
#[test]
fn r20_file_ceiling_own_runtime_locked_edit_latency_is_descriptive() {
    latency_case("r20", 1024 * 1024)
}
#[test]
fn r21_rebind_preserves_unicode_feff_bom_crlf_multidocument_save_and_undo() {
    let original = "\u{feff}\u{feff}A😀\r\nB\r\n";
    let (_, p, _case_watch) = fixture("r21", original.as_bytes());
    fs::write(p.join("b.tex"), b"B").unwrap();
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let a = ok(
        &mut r,
        Request::ApplyEdits {
            document_id: "main.tex".into(),
            expected_revision: rev,
            edits: vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 1,
                },
                end: Position {
                    line: 0,
                    character: 2,
                },
                text: "X".into(),
            }],
        },
    )["revision"]
        .as_u64()
        .unwrap();
    let b = ok(
        &mut r,
        Request::OpenDocument {
            path: "b.tex".into(),
        },
    )["revision"]
        .as_u64()
        .unwrap();
    ok(
        &mut r,
        Request::ApplyEdits {
            document_id: "b.tex".into(),
            expected_revision: b,
            edits: vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
                text: "B1".into(),
            }],
        },
    );
    replace_file(&p, original.as_bytes());
    assert_eq!(open(&mut r), a);
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "\u{feff}X😀\nB\n"
    );
    assert!(
        save(&mut r, a).ok,
        "explicit identical-byte rebind cannot discard or strand Unicode dirty state"
    );
    assert_eq!(
        fs::read(p.join("main.tex")).unwrap(),
        "\u{feff}\u{feff}X😀\r\nB\r\n".as_bytes()
    );
    ok(&mut r, Request::Undo);
    assert_eq!(r.workspace.snapshot("b.tex").unwrap().text, "B");
    ok(&mut r, Request::Undo);
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "\u{feff}A😀\nB\n"
    );
    assert!(r.workspace.snapshot("main.tex").unwrap().dirty);
    assert_eq!(fs::read(p.join("b.tex")).unwrap(), b"B");
    state(&r, "unicode-undo");
    disk(&p.join("main.tex"), "unicode-saved");
    drop(r);
    let r = runtime(&p);
    assert!(r.workspace.can_redo());
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "\u{feff}A😀\nB\n"
    );
}
#[test]
fn r22_same_inode_repeated_explicit_open_is_not_a_new_commit() {
    let (_, p, _case_watch) = fixture("r22", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    let seq = r.workspace.recovery_status().sequence;
    assert_eq!(open(&mut r), rev);
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "dirty");
    assert!(r.workspace.can_undo());
    state(&r, "same-inode-no-new-commit");
}

fn storage_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, p: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for e in fs::read_dir(p).unwrap() {
            let e = e.unwrap();
            let kind = e.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                walk(root, &e.path(), out)
            } else {
                out.push((
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    fs::read(e.path()).unwrap(),
                ));
            }
        }
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out.sort();
    out
}
#[test]
fn r23_replaced_project_root_cannot_rebind_into_a_copied_foreign_store() {
    let (base, p, _case_watch) = fixture("r23", b"old");
    let mut r = runtime(&p);
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "acknowledged");
    let original_meta = fs::metadata(&p).unwrap();
    let original = base.join("retained-original-project");
    fs::rename(&p, &original).unwrap();
    fs::create_dir(&p).unwrap();
    fs::write(p.join("main.tex"), b"old").unwrap();
    let changed_meta = fs::metadata(&p).unwrap();
    assert_ne!(
        (original_meta.dev(), original_meta.ino()),
        (changed_meta.dev(), changed_meta.ino())
    );
    fs::create_dir(p.join(".latexide")).unwrap();
    fs::set_permissions(p.join(".latexide"), fs::Permissions::from_mode(0o700)).unwrap();
    let storage = p.join(".latexide/recovery");
    copy_tree(&original.join(".latexide/recovery"), &storage);
    let before = storage_bytes(&storage);
    let seq = r.workspace.recovery_status().sequence;
    let reply = call(
        &mut r,
        Request::OpenDocument {
            path: "main.tex".into(),
        },
    );
    if !reply.ok {
        assert_eq!(reply.error.unwrap().code, "recovery_corrupt");
    }
    assert_eq!(
        r.workspace.recovery_status().sequence,
        seq,
        "file rebind must not authorize a different project-root identity"
    );
    assert_eq!(
        storage_bytes(&storage),
        before,
        "foreign copied store must not be written/pruned by the old Runtime"
    );
    assert_eq!(
        r.workspace.snapshot("main.tex").unwrap().text,
        "acknowledged"
    );
    assert!(r.workspace.can_undo());
    assert_error(save(&mut r, rev), "external_change");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    state(&r, "foreign-project-root-refused");
    disk(&p.join("main.tex"), "foreign-root-file-unchanged");
}

// Publisher integration controls; original r01-r23 assertions remain unchanged.
#[test]
fn r24_save_precheck_messages_are_hungarian_without_state_or_write_changes() {
    let (_, p, _case_watch) = fixture("r24", b"old");
    let mut r = runtime(&p);
    let seq = r.workspace.recovery_status().sequence;
    let reply = call(
        &mut r,
        Request::SaveDocument {
            document_id: "main.tex".into(),
            expected_revision: 0,
        },
    );
    assert!(!reply.ok);
    let e = reply.error.unwrap();
    assert_eq!(e.code, "document_not_open");
    assert_eq!(e.message, "A dokumentum nincs megnyitva.");
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    let rev = open(&mut r);
    let rev = edit(&mut r, rev, "dirty");
    let before = serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap();
    let seq = r.workspace.recovery_status().sequence;
    let stale = save(&mut r, rev - 1);
    assert!(!stale.ok);
    let e = stale.error.unwrap();
    assert_eq!(e.code, "revision_conflict");
    assert_eq!(e.message, "A mentés várt revíziója elavult.");
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    fs::write(p.join("main.tex"), b"foreign").unwrap();
    let conflict = save(&mut r, rev);
    assert!(!conflict.ok);
    let e = conflict.error.unwrap();
    assert_eq!(e.code, "external_change");
    assert_eq!(
        e.message,
        "A lemezbeli alapállapot megváltozott; nem történt írás."
    );
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(
        serde_json::to_value(r.workspace.snapshot("main.tex").unwrap()).unwrap(),
        before
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"foreign");
    assert!(r.workspace.can_undo());
}
#[test]
fn r25_direct_workspace_identical_reopen_preserves_save_and_recovered_undo() {
    let (_, p, _case_watch) = fixture("r25", b"old");
    let mut w = Workspace::open_recoverable(p.clone(), p.join(".latexide/recovery")).unwrap();
    let d = w.open_document("main.tex").unwrap();
    let d = w
        .apply_edits(
            "main.tex",
            d.revision,
            vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 3,
                },
                text: "dirty".into(),
            }],
            "agent",
        )
        .unwrap();
    replace_file(&p, b"old");
    assert_eq!(
        w.save_document("main.tex", d.revision).unwrap_err().code,
        "external_change"
    );
    let seq = w.recovery_status().sequence;
    let reopened = w.open_document("main.tex").unwrap();
    assert_eq!(reopened.revision, d.revision);
    assert_eq!(reopened.text, "dirty");
    assert_eq!(w.recovery_status().sequence, seq + 1);
    w.save_document("main.tex", d.revision).unwrap();
    w.undo().unwrap();
    assert_eq!(w.snapshot("main.tex").unwrap().text, "old");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"dirty");
    drop(w);
    let w = Workspace::open_recoverable(p.clone(), p.join(".latexide/recovery")).unwrap();
    assert_eq!(w.snapshot("main.tex").unwrap().text, "old");
    assert!(w.can_redo());
}
#[test]
fn r26_direct_workspace_different_physical_bytes_never_gain_save_authority() {
    let (_, p, _case_watch) = fixture("r26", b"old");
    let mut w = Workspace::open_recoverable(p.clone(), p.join(".latexide/recovery")).unwrap();
    let d = w.open_document("main.tex").unwrap();
    let d = w
        .apply_edits(
            "main.tex",
            d.revision,
            vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 3,
                },
                text: "dirty".into(),
            }],
            "agent",
        )
        .unwrap();
    let before = serde_json::to_value(w.snapshot("main.tex").unwrap()).unwrap();
    let seq = w.recovery_status().sequence;
    replace_file(&p, b"foreign");
    w.open_document("main.tex").unwrap();
    assert_eq!(w.recovery_status().sequence, seq);
    assert_eq!(
        serde_json::to_value(w.snapshot("main.tex").unwrap()).unwrap(),
        before
    );
    assert_eq!(
        w.save_document("main.tex", d.revision).unwrap_err().code,
        "external_change"
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"foreign");
    assert!(w.can_undo());
}
