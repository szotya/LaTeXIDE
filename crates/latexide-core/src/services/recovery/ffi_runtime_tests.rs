#![cfg(target_os = "macos")]
use super::Runtime;
use latexide_core::{Position, TextEdit};
use latexide_protocol::Request;
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);
fn project() -> PathBuf {
    let parent = std::env::var("P01_RUNTIME_SCRATCH")
        .map(|p| PathBuf::from(p).join("tmp"))
        .unwrap_or_else(|_| std::env::temp_dir().canonicalize().unwrap());
    let p = parent.join(format!(
        "ffi-runtime-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(p.join("main.tex"), b"old").unwrap();
    p
}
fn open(r: &mut Runtime) -> u64 {
    r.dispatch(
        Request::OpenDocument {
            path: "main.tex".into(),
        },
        "user",
    )
    .result
    .unwrap()["revision"]
        .as_u64()
        .unwrap()
}
fn edit(rev: u64) -> Request {
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
            text: "new".into(),
        }],
    }
}
#[test]
fn native_and_agent_dispatch_share_the_restored_workspace_and_save_undo() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    let reply = r.dispatch(edit(rev), "agent");
    assert!(reply.ok);
    let rev = reply.result.unwrap()["revision"].as_u64().unwrap();
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"old");
    drop(r);
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let reply = r.dispatch(
        Request::ReadDocument {
            document_id: "main.tex".into(),
            revision: Some(rev),
            offset_bytes: 0,
            limit_bytes: 64,
        },
        "user",
    );
    assert!(reply.ok);
    assert_eq!(
        reply.result.unwrap()["text"],
        "new",
        "native runtime factory must restore the agent's acknowledged edit"
    );
    assert!(
        r.dispatch(
            Request::SaveDocument {
                document_id: "main.tex".into(),
                expected_revision: rev
            },
            "agent"
        )
        .ok
    );
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"new");
    assert!(r.dispatch(Request::Undo, "user").ok);
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "old");
    assert!(r.workspace.snapshot("main.tex").unwrap().dirty);
    assert_eq!(fs::read(p.join("main.tex")).unwrap(), b"new");
}
#[test]
fn native_composition_guard_is_preserved_with_durable_agent_access() {
    let p = project();
    let mut r = Runtime::open_project(p.clone()).unwrap();
    let rev = open(&mut r);
    assert!(
        r.dispatch(
            Request::BeginComposition {
                document_id: "main.tex".into(),
                expected_revision: rev
            },
            "user"
        )
        .ok
    );
    let sequence = r.workspace.recovery_status().sequence;
    assert_eq!(
        r.dispatch(edit(rev), "agent").error.unwrap().code,
        "composition_active"
    );
    assert_eq!(r.workspace.recovery_status().sequence, sequence);
    let edits = vec![TextEdit {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 3,
        },
        text: "IME".into(),
    }];
    assert!(
        r.dispatch(
            Request::EndComposition {
                document_id: "main.tex".into(),
                expected_revision: rev,
                edits
            },
            "user"
        )
        .ok
    );
    drop(r);
    let mut r = Runtime::open_project(p).unwrap();
    assert_eq!(r.workspace.snapshot("main.tex").unwrap().text, "IME");
    assert!(r.dispatch(Request::Undo, "agent").ok);
}

#[path = "pr68_runtime_tests.rs"]
mod pr68_review;

#[path = "review_followup_tests.rs"]
mod review_followup;
