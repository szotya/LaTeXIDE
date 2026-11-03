#![cfg(target_os = "macos")]
use latexide_core::{Position, TextEdit, Workspace};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
fn fixture() -> (PathBuf, PathBuf) {
    let parent = std::env::var("P01_RUNTIME_SCRATCH")
        .map(|p| PathBuf::from(p).join("tmp"))
        .unwrap_or_else(|_| std::env::temp_dir().canonicalize().unwrap());
    let base = parent.join(format!(
        "runtime-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    let root = base.join("project");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("main.tex"), b"original").unwrap();
    (root, base.join("recovery"))
}
fn replace(revision: u64, w: &mut Workspace, text: &str) -> u64 {
    let old = w.snapshot("main.tex").unwrap();
    let end = old.text.encode_utf16().count() as u32;
    w.apply_edits(
        "main.tex",
        revision,
        vec![TextEdit {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: end,
            },
            text: text.into(),
        }],
        "agent",
    )
    .unwrap()
    .revision
}
#[test]
fn durable_runtime_restart_explicit_save_then_undo() {
    let (root, storage) = fixture();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let d = w.open_document("main.tex").unwrap();
    replace(d.revision, &mut w, "agent");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"original");
    drop(w);
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    let d = w.open_document("main.tex").unwrap();
    assert_eq!(
        d.text, "agent",
        "acknowledged dirty state must restore into the single Workspace"
    );
    assert!(d.dirty);
    w.save_document(&d.id, d.revision).unwrap();
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"agent");
    let d = w.undo().unwrap();
    assert_eq!(d.text, "original");
    assert!(d.dirty);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"agent");
    w.save_document(&d.id, d.revision).unwrap();
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"original");
}

use latexide_core::{RecoveryObserver, RecoveryPhase};
use std::sync::{Arc, atomic::AtomicBool};
struct Fault {
    armed: Arc<AtomicBool>,
    phase: RecoveryPhase,
    skip: usize,
}
impl RecoveryObserver for Fault {
    fn phase(&mut self, p: RecoveryPhase) -> std::io::Result<()> {
        if p == self.phase && self.armed.load(Ordering::SeqCst) {
            if self.skip > 0 {
                self.skip -= 1;
            } else {
                return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            }
        }
        Ok(())
    }
}
#[test]
fn recovery_failure_before_publish_rolls_back_text_revision_and_undo_then_can_retry() {
    let (root, storage) = fixture();
    let armed = Arc::new(AtomicBool::new(false));
    let mut w = Workspace::open_recoverable_with_observer(
        root.clone(),
        storage.clone(),
        Box::new(Fault {
            armed: armed.clone(),
            phase: RecoveryPhase::BeforePublish,
            skip: 0,
        }),
    )
    .unwrap();
    let d = w.open_document("main.tex").unwrap();
    let r = replace(d.revision, &mut w, "accepted");
    armed.store(true, Ordering::SeqCst);
    let e = w
        .apply_edits(
            "main.tex",
            r,
            vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 8,
                },
                text: "rejected".into(),
            }],
            "agent",
        )
        .unwrap_err();
    assert_eq!(e.code, "recovery_io");
    assert_eq!(w.snapshot("main.tex").unwrap().text, "accepted");
    assert_eq!(w.snapshot("main.tex").unwrap().revision, r);
    armed.store(false, Ordering::SeqCst);
    replace(r, &mut w, "retry");
    assert_eq!(w.undo().unwrap().text, "accepted");
    drop(w);
    let mut w = Workspace::open_recoverable(root, storage).unwrap();
    assert_eq!(w.undo().unwrap().text, "original");
}
#[test]
fn recovery_publication_uncertain_preserves_complete_candidate_and_blocks_more_changes() {
    let (root, storage) = fixture();
    let armed = Arc::new(AtomicBool::new(false));
    let mut w = Workspace::open_recoverable_with_observer(
        root.clone(),
        storage.clone(),
        Box::new(Fault {
            armed: armed.clone(),
            phase: RecoveryPhase::SelectorRenamed,
            skip: 0,
        }),
    )
    .unwrap();
    let d = w.open_document("main.tex").unwrap();
    armed.store(true, Ordering::SeqCst);
    let e = w
        .apply_edits(
            &d.id,
            d.revision,
            vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 8,
                },
                text: "published".into(),
            }],
            "agent",
        )
        .unwrap_err();
    assert_eq!(e.code, "recovery_commit_uncertain");
    assert!(w.recovery_status().blocked);
    assert_eq!(w.snapshot(&d.id).unwrap().text, "published");
    assert_eq!(w.undo().unwrap_err().code, "recovery_commit_uncertain");
    drop(w);
    let mut w = Workspace::open_recoverable(root, storage).unwrap();
    assert_eq!(w.snapshot("main.tex").unwrap().text, "published");
    assert_eq!(w.undo().unwrap().text, "original");
}
#[test]
fn save_finalization_failure_reports_disk_saved_and_preserves_undo_after_restart() {
    let (root, storage) = fixture();
    let armed = Arc::new(AtomicBool::new(false));
    let mut w = Workspace::open_recoverable_with_observer(
        root.clone(),
        storage.clone(),
        Box::new(Fault {
            armed: armed.clone(),
            phase: RecoveryPhase::BeforePublish,
            skip: 1,
        }),
    )
    .unwrap();
    let d = w.open_document("main.tex").unwrap();
    let r = replace(d.revision, &mut w, "saved");
    armed.store(true, Ordering::SeqCst);
    assert_eq!(
        w.save_document("main.tex", r).unwrap_err().code,
        "save_completed_recovery_failed"
    );
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"saved");
    assert!(!w.snapshot("main.tex").unwrap().dirty);
    assert!(w.can_undo());
    drop(w);
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    let d = w.snapshot("main.tex").unwrap();
    assert!(!d.dirty);
    assert_eq!(w.undo().unwrap().text, "original");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"saved");
}
#[test]
fn restored_external_disk_content_or_identity_never_gets_overwritten() {
    for same_bytes in [false, true] {
        let (root, storage) = fixture();
        let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
        let d = w.open_document("main.tex").unwrap();
        replace(d.revision, &mut w, "dirty");
        drop(w);
        fs::rename(root.join("main.tex"), root.join("old.tex")).unwrap();
        let bytes = if same_bytes {
            b"original".as_slice()
        } else {
            b"external".as_slice()
        };
        fs::write(root.join("main.tex"), bytes).unwrap();
        let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
        let d = w.snapshot("main.tex").unwrap();
        assert_eq!(d.text, "dirty");
        assert_eq!(
            w.save_document(&d.id, d.revision).unwrap_err().code,
            "external_change"
        );
        assert_eq!(fs::read(root.join("main.tex")).unwrap(), bytes);
        assert!(w.can_undo());
    }
}
#[test]
fn recovery_restores_multiple_document_undo_redo_and_save_baselines() {
    let (root, storage) = fixture();
    fs::write(root.join("b.tex"), b"B").unwrap();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let a = w.open_document("main.tex").unwrap();
    let b = w.open_document("b.tex").unwrap();
    let ar = replace(a.revision, &mut w, "A1");
    w.apply_edits(
        &b.id,
        b.revision,
        vec![TextEdit {
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
        "user",
    )
    .unwrap();
    w.save_document(&a.id, ar).unwrap();
    w.undo().unwrap();
    drop(w);
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    assert!(w.can_redo());
    let b = w.redo().unwrap();
    assert_eq!(b.text, "B1");
    assert!(b.dirty);
    assert_eq!(w.undo().unwrap().text, "B");
    let a = w.undo().unwrap();
    assert_eq!(a.text, "original");
    assert!(a.dirty);
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"A1");
    assert_eq!(fs::read(root.join("b.tex")).unwrap(), b"B");
}
#[test]
fn recovery_preserves_logical_feff_bom_crlf_and_utf16_failure_atomicity() {
    let (root, storage) = fixture();
    let raw = "\u{feff}\u{feff}A😀\r\nB\r\n";
    fs::write(root.join("main.tex"), raw.as_bytes()).unwrap();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let d = w.open_document("main.tex").unwrap();
    let seq = w.recovery_status().sequence;
    assert!(
        w.apply_edits(
            &d.id,
            d.revision,
            vec![TextEdit {
                start: Position {
                    line: 0,
                    character: 3
                },
                end: Position {
                    line: 0,
                    character: 4
                },
                text: "bad".into()
            }],
            "agent"
        )
        .is_err()
    );
    assert_eq!(w.recovery_status().sequence, seq);
    let d = w
        .apply_edits(
            &d.id,
            d.revision,
            vec![TextEdit {
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
            "agent",
        )
        .unwrap();
    let rev = d.revision;
    drop(w);
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    let d = w.snapshot("main.tex").unwrap();
    assert_eq!(d.revision, rev);
    assert_eq!(d.text, "\u{feff}X😀\nB\n");
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), raw.as_bytes());
    w.save_document(&d.id, rev).unwrap();
    assert_eq!(
        fs::read(root.join("main.tex")).unwrap(),
        "\u{feff}\u{feff}X😀\r\nB\r\n".as_bytes()
    );
    assert_eq!(w.undo().unwrap().text, "\u{feff}A😀\nB\n");
}
#[test]
fn recovery_has_one_kernel_writer_and_releases_on_drop_or_open_failure() {
    let (root, storage) = fixture();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    w.open_document("main.tex").unwrap();
    assert!(
        matches!(Workspace::open_recoverable(root.clone(),storage.clone()),Err(e)if e.code=="recovery_in_use")
    );
    drop(w);
    let bytes = fs::read(storage.join("current")).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    fs::write(storage.join("current"), corrupt).unwrap();
    assert!(Workspace::open_recoverable(root.clone(), storage.clone()).is_err());
    fs::write(storage.join("current"), bytes).unwrap();
    assert!(Workspace::open_recoverable(root, storage).is_ok());
}
#[test]
fn corrupt_current_metadata_does_not_fall_back_to_an_older_state() {
    let (root, storage) = fixture();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let d = w.open_document("main.tex").unwrap();
    replace(d.revision, &mut w, "latest");
    drop(w);
    let ptr = fs::read(storage.join("current")).unwrap();
    let n = u64::from_le_bytes(ptr[32..40].try_into().unwrap()) as usize;
    let name = std::str::from_utf8(&ptr[40..40 + n]).unwrap();
    let file = storage.join(name).join("runtime-state.bin");
    let mut bytes = fs::read(&file).unwrap();
    bytes[32] ^= 1;
    fs::write(file, bytes).unwrap();
    assert!(
        matches!(Workspace::open_recoverable(root.clone(),storage),Err(e)if e.code=="recovery_corrupt")
    );
    assert_eq!(fs::read(root.join("main.tex")).unwrap(), b"original");
}
#[test]
fn runtime_retention_is_bounded_across_more_than_32_edits_without_losing_history() {
    let (root, storage) = fixture();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let d = w.open_document("main.tex").unwrap();
    let mut r = d.revision;
    for n in 0..40 {
        r = replace(r, &mut w, &format!("edit-{n}"));
    }
    let cohorts = fs::read_dir(&storage)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("cohort-")
        })
        .count();
    assert_eq!(cohorts, 2);
    drop(w);
    let mut w = Workspace::open_recoverable(root, storage).unwrap();
    assert_eq!(w.snapshot("main.tex").unwrap().text, "edit-39");
    assert_eq!(w.undo().unwrap().text, "edit-38");
}

#[test]
fn recovered_multiedit_history_remains_reversible_after_save() {
    let (root, storage) = fixture();
    fs::write(root.join("main.tex"), b"abcdef").unwrap();
    let mut w = Workspace::open_recoverable(root.clone(), storage.clone()).unwrap();
    let d = w.open_document("main.tex").unwrap();
    let d = w
        .apply_edits(
            &d.id,
            d.revision,
            vec![
                TextEdit {
                    start: Position {
                        line: 0,
                        character: 1,
                    },
                    end: Position {
                        line: 0,
                        character: 2,
                    },
                    text: "XX".into(),
                },
                TextEdit {
                    start: Position {
                        line: 0,
                        character: 4,
                    },
                    end: Position {
                        line: 0,
                        character: 5,
                    },
                    text: "YY".into(),
                },
            ],
            "agent",
        )
        .unwrap();
    w.save_document(&d.id, d.revision).unwrap();
    drop(w);
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    assert_eq!(w.undo().unwrap().text, "abcdef");
    assert_eq!(w.redo().unwrap().text, "aXXcdYYf");
    assert!(!w.snapshot("main.tex").unwrap().dirty);
}
unsafe extern "C" {
    fn _exit(code: i32) -> !;
}
fn own_exit(code: i32) -> ! {
    unsafe { _exit(code) }
}
struct ExitPoint {
    phase: RecoveryPhase,
    target: usize,
    count: usize,
}
impl RecoveryObserver for ExitPoint {
    fn phase(&mut self, p: RecoveryPhase) -> std::io::Result<()> {
        if p == self.phase {
            self.count += 1;
            if self.count == self.target {
                own_exit(73)
            }
        }
        Ok(())
    }
}
#[test]
#[ignore = "own single-runtime process-boundary driver; parent selects it explicitly"]
fn runtime_crash_child() {
    let root = PathBuf::from(std::env::var("P01_CHILD_PROJECT").unwrap());
    let storage = PathBuf::from(std::env::var("P01_CHILD_STORAGE").unwrap());
    let case = std::env::var("P01_CHILD_CASE").unwrap();
    assert_eq!(std::env::var("P01_CHILD_NONCE").unwrap().len(), 64);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(9));
        own_exit(124)
    });
    let (phase, target) = match case.as_str() {
        "edit_before_publish" => (RecoveryPhase::BeforePublish, 2),
        "edit_after_publish" => (RecoveryPhase::Published, 2),
        "save_intent_published" => (RecoveryPhase::Published, 3),
        "save_final_prepared" => (RecoveryPhase::Prepared, 4),
        "undo_after_save" => (RecoveryPhase::Published, 5),
        _ => panic!("unknown own case"),
    };
    let mut w = Workspace::open_recoverable_with_observer(
        root,
        storage,
        Box::new(ExitPoint {
            phase,
            target,
            count: 0,
        }),
    )
    .unwrap();
    let d = w.open_document("main.tex").unwrap();
    let rev = replace(d.revision, &mut w, "agent");
    w.save_document("main.tex", rev).unwrap();
    w.undo().unwrap();
    panic!("boundary did not exit")
}
#[test]
fn runtime_own_process_exit_restores_committed_state_save_intent_and_undo() {
    use std::os::unix::fs::OpenOptionsExt;
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut random = [0u8; 32];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut random)
        .unwrap();
    let nonce: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let exe = std::env::current_exe().unwrap();
    let binary = latexide_core::recovery::durable::raw_digest(&fs::read(&exe).unwrap());
    let mut lines = String::new();
    for case in [
        "edit_before_publish",
        "edit_after_publish",
        "save_intent_published",
        "save_final_prepared",
        "undo_after_save",
    ] {
        let (root, storage) = fixture();
        let log = root.parent().unwrap().join("child.log");
        let output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&log)
            .unwrap();
        let err = output.try_clone().unwrap();
        let mut child = Command::new(&exe)
            .args([
                "--ignored",
                "--exact",
                "runtime_contract::runtime_crash_child",
                "--nocapture",
            ])
            .env_clear()
            .env("P01_CHILD_PROJECT", &root)
            .env("P01_CHILD_STORAGE", &storage)
            .env("P01_CHILD_CASE", case)
            .env("P01_CHILD_NONCE", &nonce)
            .env("TMPDIR", std::env::temp_dir())
            .stdout(Stdio::from(output))
            .stderr(Stdio::from(err))
            .spawn()
            .unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "own 9s watchdog must terminate child"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(
            status.code(),
            Some(73),
            "{case}: {}",
            fs::read_to_string(log).unwrap()
        );
        let disk = fs::read(root.join("main.tex")).unwrap();
        let disk_saved = ["save_final_prepared", "undo_after_save"].contains(&case);
        assert_eq!(
            disk,
            if disk_saved {
                b"agent".as_slice()
            } else {
                b"original".as_slice()
            }
        );
        let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
        let d = w.snapshot("main.tex").unwrap();
        let expected = if ["edit_before_publish", "undo_after_save"].contains(&case) {
            "original"
        } else {
            "agent"
        };
        assert_eq!(d.text, expected);
        if case != "edit_before_publish" && case != "undo_after_save" {
            let u = w.undo().unwrap();
            assert_eq!(u.text, "original");
            assert_eq!(u.dirty, disk_saved);
        }
        if case == "undo_after_save" {
            assert!(d.dirty);
            assert_eq!(w.redo().unwrap().text, "agent");
        }
        assert_eq!(fs::read(root.join("main.tex")).unwrap(), disk);
        lines.push_str(&format!(
            "case={case} exit=73 duration_ms={} binary_sha256={binary} nonce={nonce} project={}\n",
            start.elapsed().as_millis(),
            root.display()
        ));
    }
    let evidence = fixture()
        .0
        .parent()
        .unwrap()
        .join("runtime-process-cases.txt");
    let mut f = fs::File::create(&evidence).unwrap();
    f.write_all(lines.as_bytes()).unwrap();
    f.sync_all().unwrap();
    println!(
        "P01_RUNTIME_CHILD_CASES=5 PASS=5 FAIL=0 ERROR=0 nonce={nonce} evidence={}",
        evidence.display()
    );
}

#[test]
fn pending_save_reconciliation_never_accepts_a_symlink_outside_the_project() {
    use std::os::unix::fs::symlink;
    let (root, storage) = fixture();
    let armed = Arc::new(AtomicBool::new(false));
    let mut w = Workspace::open_recoverable_with_observer(
        root.clone(),
        storage.clone(),
        Box::new(Fault {
            armed: armed.clone(),
            phase: RecoveryPhase::BeforePublish,
            skip: 1,
        }),
    )
    .unwrap();
    let d = w.open_document("main.tex").unwrap();
    let rev = replace(d.revision, &mut w, "agent");
    armed.store(true, Ordering::SeqCst);
    assert_eq!(
        w.save_document("main.tex", rev).unwrap_err().code,
        "save_completed_recovery_failed"
    );
    drop(w);
    let outside = root.parent().unwrap().join("own-outside-sentinel");
    fs::write(&outside, b"agent").unwrap();
    fs::remove_file(root.join("main.tex")).unwrap();
    symlink(&outside, root.join("main.tex")).unwrap();
    let mut w = Workspace::open_recoverable(root.clone(), storage).unwrap();
    let d = w.snapshot("main.tex").unwrap();
    assert!(d.dirty, "outside bytes cannot establish a saved baseline");
    assert_eq!(
        w.save_document("main.tex", d.revision).unwrap_err().code,
        "path_escape"
    );
    assert_eq!(fs::read(outside).unwrap(), b"agent");
}
