//! Real Runtime regressions for PR68's six follow-up findings.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::{collections::BTreeMap, path::Path};

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(p) = todo.pop() {
        for e in fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if fs::symlink_metadata(&p).unwrap().is_dir() {
                todo.push(p);
            } else {
                result.insert(
                    p.strip_prefix(root).unwrap().to_str().unwrap().into(),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    result
}
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    fs::set_permissions(to, fs::metadata(from).unwrap().permissions()).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &dest)
        } else {
            fs::copy(&p, dest).unwrap();
        }
    }
}
fn edited(r: &mut Runtime, revision: u64) -> u64 {
    let reply = r.dispatch(edit(revision), "review6");
    assert!(reply.ok, "{:?}", reply.error);
    reply.result.unwrap()["revision"].as_u64().unwrap()
}

#[test]
fn review6_move_restores_dirty_undo_and_explicit_save() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    let rev = edited(&mut r, rev);
    drop(r);
    let moved = p.with_extension("moved");
    fs::rename(&p, &moved).unwrap();
    let mut r =
        Runtime::open_project(moved.clone()).expect("moving a project must retain recovery");
    let d = r.workspace.snapshot("main.tex").unwrap();
    assert_eq!((d.text.as_str(), d.revision, d.dirty), ("new", rev, true));
    assert_eq!(fs::read(moved.join("main.tex")).unwrap(), b"old");
    assert!(r.dispatch(Request::Undo, "review6").ok);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert!(r.dispatch(Request::Redo, "review6").ok);
    let rev = r.workspace.snapshot("main.tex").unwrap().revision;
    assert!(
        r.dispatch(
            Request::SaveDocument {
                document_id: "main.tex".into(),
                expected_revision: rev
            },
            "review6"
        )
        .ok
    );
    assert_eq!(fs::read(moved.join("main.tex")).unwrap(), b"new");
}

#[test]
fn review6_unknown_entries_are_retained_without_blocking_edit_save_undo() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    let storage = p.join(".latexide/recovery");
    fs::write(storage.join(".DS_Store"), b"foreign").unwrap();
    let foreign = storage.join(format!("cohort-{}", "f".repeat(32)));
    fs::create_dir(&foreign).unwrap();
    fs::set_permissions(&foreign, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(foreign.join("notes.txt"), b"retain entire cohort").unwrap();
    let rev = edited(&mut r, rev);
    assert!(
        r.dispatch(
            Request::SaveDocument {
                document_id: "main.tex".into(),
                expected_revision: rev
            },
            "review6"
        )
        .ok
    );
    assert!(r.dispatch(Request::Undo, "review6").ok);
    assert_eq!(fs::read(storage.join(".DS_Store")).unwrap(), b"foreign");
    assert_eq!(
        fs::read(foreign.join("notes.txt")).unwrap(),
        b"retain entire cohort"
    );
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"new");
}

#[test]
fn review6_replaced_project_cannot_commit_into_a_copied_store() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    let rev = edited(&mut r, rev);
    let mut request = edit(rev);
    if let Request::ApplyEdits { edits, .. } = &mut request {
        edits[0].text = "end".into();
    }
    assert!(r.dispatch(request, "review6").ok);
    assert!(r.dispatch(Request::Undo, "review6").ok);
    let rev = r.workspace.snapshot("main.tex").unwrap().revision;
    let retained = p.with_extension("retained");
    fs::rename(&p, &retained).unwrap();
    copy_tree(&retained, &p);
    assert_ne!(
        fs::metadata(&p).unwrap().ino(),
        fs::metadata(&retained).unwrap().ino()
    );
    let before = tree(&p);
    let mut request = edit(rev);
    if let Request::ApplyEdits { edits, .. } = &mut request {
        edits[0].text = "bad".into();
    }
    let reply = r.dispatch(request, "review6");
    assert!(
        !reply.ok,
        "the writer lock belongs to the original store inode"
    );
    assert!(
        tree(&p) == before,
        "refusal must precede cleanup and writes"
    );
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "new");
    for request in [Request::Undo, Request::Redo] {
        assert!(!r.dispatch(request, "review6").ok);
        assert!(tree(&p) == before);
        assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "new");
        assert!(r.workspace.can_undo() && r.workspace.can_redo());
    }
}

#[test]
fn review6_unavailable_recovery_keeps_project_usable_and_reports_reason() {
    let p = project();
    fs::write(p.join(".latexide"), b"foreign blocking file").unwrap();
    let mut r = Runtime::open_project(p.clone()).expect("setup failure must allow in-memory work");
    let status = r.workspace.recovery_status();
    assert!(!status.enabled && status.last_error.is_some());
    let rev = open(&mut r);
    let rev = edited(&mut r, rev);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    assert!(
        r.dispatch(
            Request::SaveDocument {
                document_id: "main.tex".into(),
                expected_revision: rev
            },
            "review6"
        )
        .ok
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"new");
    assert_eq!(
        fs::read(p.join(".latexide")).unwrap(),
        b"foreign blocking file"
    );
}

#[test]
fn review6_busy_recovery_keeps_second_view_in_memory_without_store_writes() {
    let p = project();
    let mut first = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut first);
    edited(&mut first, rev);
    let before = tree(&p.join(".latexide/recovery"));
    let mut second =
        Runtime::open_project(p.clone()).expect("a busy writer must not block viewing the project");
    assert!(!second.workspace.recovery_status().enabled);
    assert_eq!(
        second.workspace.recovery_status().last_error.as_deref(),
        Some("recovery_in_use")
    );
    open(&mut second);
    assert_eq!(second.workspace.snapshot("main.tex").unwrap().text, "old");
    assert_eq!(tree(&p.join(".latexide/recovery")), before);
    assert_eq!(first.workspace.snapshot("main.tex").unwrap().text, "new");
}

#[test]
fn review6_small_edits_avoid_full_checkpoint_rewrites_and_restore_history() {
    let p = project();
    fs::write(p.join("main.tex"), vec![b'A'; 512 * 1024]).unwrap();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let mut rev = open(&mut r);
    let storage = p.join(".latexide/recovery");
    let base = fs::read(storage.join("current")).unwrap();
    let before: usize = tree(&storage).values().map(Vec::len).sum();
    for c in b'B'..=b'M' {
        let reply = r.dispatch(
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
                    text: (c as char).to_string(),
                }],
            },
            "review6",
        );
        assert!(reply.ok, "{:?}", reply.error);
        rev = reply.result.unwrap()["revision"].as_u64().unwrap();
    }
    assert_eq!(
        fs::read(storage.join("current")).unwrap(),
        base,
        "small edits must append to the selected checkpoint"
    );
    let after: usize = tree(&storage).values().map(Vec::len).sum();
    assert!(
        after < before + 64 * 1024,
        "one-byte edits must not journal whole documents/history"
    );
    drop(r);
    let mut r = Runtime::open_project(p.clone()).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().revision, rev);
    assert!(
        r.workspace
            .snapshot("main.tex")
            .unwrap()
            .text
            .starts_with('M')
    );
    assert!(r.dispatch(Request::Undo, "review6").ok);
    assert!(
        r.workspace
            .snapshot("main.tex")
            .unwrap()
            .text
            .starts_with('L')
    );
    assert!(r.dispatch(Request::Redo, "review6").ok);
    assert!(
        r.workspace
            .snapshot("main.tex")
            .unwrap()
            .text
            .starts_with('M')
    );
    assert_eq!(
        fs::read(p.join("main.tex")).unwrap(),
        vec![b'A'; 512 * 1024]
    );
}

fn selected(root: &Path) -> PathBuf {
    let bytes = fs::read(root.join("current")).unwrap();
    let len = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
    root.join(std::str::from_utf8(&bytes[40..40 + len]).unwrap())
}

#[test]
fn review6_missing_edit_log_never_silently_restores_an_older_checkpoint() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    edited(&mut r, rev);
    drop(r);
    let log = selected(&p.join(".latexide/recovery")).join("runtime-edits.rec");
    fs::rename(&log, log.with_extension("retained")).unwrap();
    assert!(
        Runtime::open_project(p).is_err(),
        "a selected incremental log is mandatory even when its base checkpoint is valid"
    );
}

#[test]
fn review6_torn_selected_log_is_rejected_without_baseline_fallback() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    edited(&mut r, rev);
    drop(r);
    let log = selected(&p.join(".latexide/recovery")).join("runtime-edits.rec");
    let original = fs::read(&log).unwrap();
    fs::write(&log, &original[..original.len() - 1]).unwrap();
    assert!(Runtime::open_project(p.clone()).is_err());
    assert_eq!(fs::read(log).unwrap(), original[..original.len() - 1]);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
}

#[test]
fn review6_unselected_raw_tail_is_retained_and_never_replayed() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    let rev = edited(&mut r, rev);
    drop(r);
    let log = selected(&p.join(".latexide/recovery")).join("runtime-edits.rec");
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all(b"unselected raw tail")
        .unwrap();
    let raw = fs::read(&log).unwrap();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "new");
    let mut request = edit(rev);
    if let Request::ApplyEdits { edits, .. } = &mut request {
        edits[0].text = "end".into();
    }
    assert!(r.dispatch(request, "review6").ok);
    assert_eq!(
        fs::read(&log).unwrap(),
        raw,
        "checkpointing must preserve the unselected old tail"
    );
    drop(r);
    let r = Runtime::open_project(p.clone()).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "end");
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
}

#[test]
fn review6_incremental_history_eviction_preserves_unicode_encoding_and_256_undos() {
    let p = project();
    fs::write(p.join("main.tex"), "\u{feff}0000😀\r\n".as_bytes()).unwrap();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let mut rev = open(&mut r);
    for n in 1..=270 {
        let reply = r.dispatch(
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
                        character: 4,
                    },
                    text: format!("{n:04}"),
                }],
            },
            "review6",
        );
        assert!(reply.ok, "{:?}", reply.error);
        rev = reply.result.unwrap()["revision"].as_u64().unwrap();
    }
    drop(r);
    let mut r = Runtime::open_project(p.clone()).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "0270😀\n");
    for _ in 0..256 {
        assert!(r.dispatch(Request::Undo, "review6").ok);
    }
    assert!(!r.workspace.can_undo());
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "0014😀\n");
    assert_eq!(
        fs::read(p.join("main.tex")).unwrap(),
        "\u{feff}0000😀\r\n".as_bytes()
    );
}

#[test]
fn review6_missing_or_corrupt_selected_receipt_is_never_an_in_memory_fallback() {
    for kind in ["missing", "torn", "corrupt"] {
        let p = project();
        let mut r = Runtime::open_project(p.clone()).unwrap();
        let rev = open(&mut r);
        edited(&mut r, rev);
        drop(r);
        let head = selected(&p.join(".latexide/recovery")).join("runtime-head.rec");
        let raw = fs::read(&head).unwrap();
        match kind {
            "missing" => fs::rename(&head, head.with_extension("retained")).unwrap(),
            "torn" => fs::write(&head, &raw[..raw.len() - 1]).unwrap(),
            _ => {
                let mut corrupt = raw;
                corrupt[32] ^= 1;
                fs::write(&head, corrupt).unwrap();
            }
        }
        assert!(
            Runtime::open_project(p.clone()).is_err(),
            "selected receipt {kind}"
        );
        assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    }
}

struct PrepareFault(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl latexide_core::RecoveryObserver for PrepareFault {
    fn phase(&mut self, p: latexide_core::RecoveryPhase) -> std::io::Result<()> {
        if p == latexide_core::RecoveryPhase::BeforePublish && self.0.load(Ordering::SeqCst) {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    }
}
#[test]
fn review6_prepared_delta_failure_preserves_selected_receipt_and_allows_reopen() {
    let p = project();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let w = latexide_core::Workspace::open_recoverable_with_observer(
        p.clone(),
        p.join(".latexide/recovery"),
        Box::new(PrepareFault(armed.clone())),
    )
    .unwrap();
    let mut r = Runtime::new(w);
    let rev = open(&mut r);
    let rev = edited(&mut r, rev);
    let dir = selected(&p.join(".latexide/recovery"));
    let head = dir.join("runtime-head.rec");
    let log = dir.join("runtime-edits.rec");
    let before = fs::read(&head).unwrap();
    let len = fs::metadata(&log).unwrap().len();
    let seq = r.workspace.recovery_status().sequence;
    armed.store(true, Ordering::SeqCst);
    let mut request = edit(rev);
    if let Request::ApplyEdits { edits, .. } = &mut request {
        edits[0].text = "bad".into();
    }
    let reply = r.dispatch(request, "review6");
    assert!(!reply.ok);
    assert_eq!(reply.error.unwrap().code, "recovery_io");
    assert_eq!(fs::read(&head).unwrap(), before);
    assert!(fs::metadata(&log).unwrap().len() > len);
    assert_eq!(r.workspace.recovery_status().sequence, seq);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "new");
    drop(r);
    let mut r = Runtime::open_project(p.clone()).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "new");
    let mut request = edit(rev);
    if let Request::ApplyEdits { edits, .. } = &mut request {
        edits[0].text = "end".into();
    }
    assert!(r.dispatch(request, "review6").ok);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "end");
    assert_eq!(fs::read(head).unwrap(), before);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
}
