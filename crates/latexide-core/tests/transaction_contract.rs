//! P02 preparation-only checks. HostWitness is inert evidence, never a runtime.
use latexide_core::services::transactions as transaction_service;
// Name-only re-export: fixtures and known vectors call the actual compiled library.
mod fixture_hash {
    pub use latexide_core::services::transactions::content_sha256 as hex;
}

use latexide_core::contracts::{
    CallContext, ContractError, RevisionedEdit, SnapshotFile, SnapshotLimits, WorkspaceSnapshot,
    common::MAX_FILE_BYTES,
    decode_request,
    transactions::{
        PrepareTransactionRequest, PreparedTransaction, TransactionError, TransactionsPort,
    },
};
use std::cell::Cell;
use transaction_service::TransactionService;

fn service() -> TransactionService {
    TransactionService::new(SnapshotLimits {
        max_files: 128,
        max_file_bytes: MAX_FILE_BYTES,
        max_total_bytes: 8 * MAX_FILE_BYTES,
    })
}
fn file(path: &str, revision: Option<u64>, text: &str) -> SnapshotFile {
    SnapshotFile {
        path: path.into(),
        revision,
        content_sha256: fixture_hash::hex(text.as_bytes()),
        utf8: text.as_bytes().to_vec(),
        dirty: revision.is_some(),
    }
}
fn snapshot() -> WorkspaceSnapshot {
    let nonce = std::env::var("P02_RUN_NONCE").unwrap_or_else(|_| "standalone".into());
    eprintln!("P02 fixture nonce={nonce}");
    WorkspaceSnapshot {
        id: format!("p02-{nonce}"),
        epoch: 19,
        files: vec![
            file("main.tex", Some(7), "A😀\n\u{feff}é"),
            file("fejezet ő.tex", Some(11), "beta\n"),
        ],
    }
}
fn edit(path: &str, revision: u64, start: u32, length: u32, text: &str) -> RevisionedEdit {
    RevisionedEdit {
        document_id: path.into(),
        expected_revision: revision,
        start_utf16: start,
        length_utf16: length,
        replacement: text.into(),
    }
}
fn valid_edits() -> Vec<RevisionedEdit> {
    vec![
        edit("main.tex", 7, 1, 2, "ő"),
        edit("fejezet ő.tex", 11, 0, 1, "B"),
    ]
}
#[derive(Clone, Debug, PartialEq)]
struct HostWitness {
    snapshot: WorkspaceSnapshot,
    raw_saved_baselines: Vec<Vec<u8>>,
    undo: Vec<String>,
    redo: Vec<String>,
}
fn witness() -> HostWitness {
    HostWitness {
        snapshot: snapshot(),
        // Synthetic physical BOM + CRLF metadata remains host-owned, outside v1 snapshot.
        raw_saved_baselines: vec![
            b"\xef\xbb\xbfA\xf0\x9f\x98\x80\r\n\xef\xbb\xbf\xc3\xa9".to_vec(),
            b"beta\r\n".to_vec(),
        ],
        undo: vec!["prior-agent-edit".into()],
        redo: vec!["prior-redo".into()],
    }
}

#[test]
fn contract_v1_first_slice() {
    let host = witness();
    let before = host.clone();
    let mut edits = valid_edits();
    edits[1].expected_revision = 10;
    assert_eq!(
        service().prepare_transaction(&host.snapshot, edits),
        Err(TransactionError::StaleRevision)
    );
    assert_eq!(host, before);
    let prepared = service()
        .prepare_transaction(&host.snapshot, valid_edits())
        .unwrap();
    assert_eq!(prepared.edits, valid_edits());
    assert_eq!(host, before);
}
#[test]
fn review_focus_r1() {
    let host = witness();
    let before = host.clone();
    let edits = vec![
        edit("main.tex", 7, 0, 1, "first"),
        edit("fejezet ő.tex", 11, 100, 1, "second"),
    ];
    assert_eq!(
        service().prepare_transaction(&host.snapshot, edits),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(host, before);
    assert!(
        service()
            .prepare_transaction(&host.snapshot, valid_edits())
            .is_ok()
    );
}
#[test]
fn review_focus_r2() {
    let s = snapshot();
    for edits in [
        vec![edit("main.tex", 7, 2, 0, "x")],
        vec![
            edit("main.tex", 7, 0, 3, "x"),
            edit("main.tex", 7, 1, 2, "y"),
        ],
    ] {
        assert_eq!(
            service().prepare_transaction(&s, edits),
            Err(TransactionError::InvalidRange)
        );
    }
    assert!(service().prepare_transaction(&s, valid_edits()).is_ok());
}
#[test]
fn review_focus_r3() {
    let mut host = witness();
    for f in &mut host.snapshot.files {
        f.dirty = false;
    }
    let before = host.clone();
    assert!(
        service()
            .prepare_transaction(&host.snapshot, valid_edits())
            .is_ok()
    );
    assert_eq!(
        service().prepare_transaction(&host.snapshot, vec![edit("main.tex", 6, 0, 1, "x")]),
        Err(TransactionError::StaleRevision)
    );
    assert_eq!(host, before); // Preparation only; no Save or undo implementation is claimed.
    assert_eq!(host.undo, vec!["prior-agent-edit"]);
    assert_eq!(host.redo, vec!["prior-redo"]);
}
#[test]
fn review_focus_r4() {
    let s = snapshot();
    let before = s.clone();
    let good = vec![edit("fejezet ő.tex", 11, 0, 4, "törzs")];
    assert_eq!(
        service()
            .prepare_transaction(&s, good.clone())
            .unwrap()
            .edits,
        good
    );
    for path in [
        "../outside.tex",
        "/outside.tex",
        "a/../main.tex",
        "C:/outside.tex",
        "a\\b.tex",
    ] {
        assert_eq!(
            service().prepare_transaction(&s, vec![edit(path, 7, 0, 0, "x")]),
            Err(TransactionError::OutsideRoot)
        );
    }
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("absent.tex", 7, 0, 0, "x")]),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(s, before); // No OS path resolution or symlink claim.
}
#[test]
fn review_focus_r5() {
    let mut host = witness();
    host.undo.extend((0..300).map(|n| format!("typing-{n}")));
    let before = host.clone();
    let port = service();
    for _ in 0..300 {
        assert!(
            port.prepare_transaction(&host.snapshot, valid_edits())
                .is_ok()
        );
    }
    assert_eq!(host, before);
    assert_eq!(host.undo.len(), 301);
    assert_eq!(host.undo.first().unwrap(), "prior-agent-edit");
}
#[test]
fn utf16_emoji_scalar_boundaries() {
    let s = snapshot();
    for (start, len) in [(2, 0), (1, 1), (2, 1)] {
        assert_eq!(
            service().prepare_transaction(&s, vec![edit("main.tex", 7, start, len, "x")]),
            Err(TransactionError::InvalidRange)
        );
    }
    for (start, len) in [(1, 2), (3, 0), (0, 3), (4, 2)] {
        assert!(
            service()
                .prepare_transaction(&s, vec![edit("main.tex", 7, start, len, "é😀")])
                .is_ok()
        );
    }
}
#[test]
fn logical_feff_lf_and_raw_crlf_boundary() {
    let mut s = snapshot();
    s.files[0] = file("main.tex", Some(7), "\u{feff}A😀\nZ");
    let before = s.clone();
    let edits = vec![edit("main.tex", 7, 0, 1, "\u{feff}\n")];
    assert_eq!(
        service()
            .prepare_transaction(&s, edits.clone())
            .unwrap()
            .edits,
        edits
    );
    assert_eq!(s, before);
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 7, 0, 0, "\r\n")]),
        Err(TransactionError::InvalidRange)
    );
    s.files[0] = file("main.tex", Some(7), "\u{feff}A\r\nZ");
    assert_eq!(
        service().prepare_transaction(&s, vec![]),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(
        String::from_utf8(s.files[0].utf8.clone()).unwrap(),
        "\u{feff}A\r\nZ"
    );
}
#[test]
fn duplicate_documents_rejected_including_unedited() {
    let mut s = snapshot();
    let duplicate = s.files[1].clone();
    s.files.push(duplicate);
    let before = s.clone();
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 7, 0, 1, "x")]),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(s, before);
}
#[test]
fn revision_zero_and_missing_rejected() {
    let mut s = snapshot();
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 0, 0, 0, "x")]),
        Err(TransactionError::StaleRevision)
    );
    s.files[0].revision = Some(0);
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 0, 0, 0, "x")]),
        Err(TransactionError::InvalidRange)
    );
    s.files[0].revision = None;
    s.files[0].dirty = false;
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 7, 0, 0, "x")]),
        Err(TransactionError::StaleRevision)
    );
    s.files[0].dirty = true;
    assert_eq!(
        service().prepare_transaction(&s, vec![]),
        Err(TransactionError::InvalidRange)
    );
}
#[test]
fn revision_u64_overflow_rejected() {
    let mut s = snapshot();
    for revision in [9_007_199_254_740_993, u64::MAX - 1] {
        s.files[0].revision = Some(revision);
        let edits = vec![edit("main.tex", revision, 0, 1, "x")];
        assert_eq!(
            service()
                .prepare_transaction(&s, edits.clone())
                .unwrap()
                .edits,
            edits
        );
    }
    s.files[0].revision = Some(u64::MAX);
    let before = s.clone();
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", u64::MAX, 0, 1, "x")]),
        Err(TransactionError::Quota)
    );
    assert_eq!(s, before);
    assert!(
        service()
            .prepare_transaction(&s, vec![edit("fejezet ő.tex", 11, 0, 1, "x")])
            .is_ok()
    );
}
#[test]
fn utf16_u32_overflow_and_oob() {
    let s = snapshot();
    for (start, length) in [(u32::MAX, 1), (1, u32::MAX), (7, 0), (6, 1)] {
        assert_eq!(
            service().prepare_transaction(&s, vec![edit("main.tex", 7, start, length, "x")]),
            Err(TransactionError::InvalidRange)
        );
    }
    assert!(
        service()
            .prepare_transaction(&s, vec![edit("main.tex", 7, 6, 0, "x")])
            .is_ok()
    );
}
#[test]
fn batch_1000_boundary() {
    let mut s = snapshot();
    s.files[0] = file("main.tex", Some(7), &"a".repeat(1001));
    let mut edits: Vec<_> = (0..1000).map(|n| edit("main.tex", 7, n, 0, "x")).collect();
    assert_eq!(
        service()
            .prepare_transaction(&s, edits.clone())
            .unwrap()
            .edits
            .len(),
        1000
    );
    edits.push(edit("main.tex", 7, 1000, 0, "x"));
    assert_eq!(
        service().prepare_transaction(&s, edits),
        Err(TransactionError::Quota)
    );
}
#[test]
fn per_file_1mib_boundaries() {
    let mut s = snapshot();
    s.files[0] = file("main.tex", Some(7), &"x".repeat(MAX_FILE_BYTES));
    assert!(
        service()
            .prepare_transaction(&s, vec![edit("main.tex", 7, 0, 1, "z")])
            .is_ok()
    );
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 7, MAX_FILE_BYTES as u32, 0, "z")]),
        Err(TransactionError::Quota)
    );
    s.files[0] = file("main.tex", Some(7), "");
    assert!(
        service()
            .prepare_transaction(
                &s,
                vec![edit("main.tex", 7, 0, 0, &"x".repeat(MAX_FILE_BYTES))]
            )
            .is_ok()
    );
    assert_eq!(
        service().prepare_transaction(
            &s,
            vec![edit("main.tex", 7, 0, 0, &"x".repeat(MAX_FILE_BYTES + 1))]
        ),
        Err(TransactionError::Quota)
    );
    s.files[0] = file("main.tex", Some(7), &"x".repeat(MAX_FILE_BYTES + 1));
    assert_eq!(
        service().prepare_transaction(&s, vec![]),
        Err(TransactionError::Quota)
    );
}
#[test]
fn result_size_uses_utf8_bytes() {
    let mut s = snapshot();
    s.files = vec![file("main.tex", Some(7), "😀")];
    let port = TransactionService::new(SnapshotLimits {
        max_files: 1,
        max_file_bytes: 4,
        max_total_bytes: 4,
    });
    assert!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 0, 2, "éé")])
            .is_ok()
    );
    assert_eq!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 0, 2, "ééx")]),
        Err(TransactionError::Quota)
    );
    assert!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 0, 2, "x")])
            .is_ok()
    );
}
#[test]
fn configured_snapshot_and_result_limits() {
    let mut s = snapshot();
    s.files = vec![
        file("main.tex", Some(7), "abc"),
        file("b.tex", Some(1), "def"),
    ];
    let port = TransactionService::new(SnapshotLimits {
        max_files: 2,
        max_file_bytes: 4,
        max_total_bytes: 6,
    });
    assert!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 0, 1, "z")])
            .is_ok()
    );
    assert_eq!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 3, 0, "z")]),
        Err(TransactionError::Quota)
    );
    let port = TransactionService::new(SnapshotLimits {
        max_files: 2,
        max_file_bytes: 3,
        max_total_bytes: 8,
    });
    assert_eq!(
        port.prepare_transaction(&s, vec![edit("main.tex", 7, 3, 0, "z")]),
        Err(TransactionError::Quota)
    );
    for limits in [
        SnapshotLimits {
            max_files: 1,
            max_file_bytes: 4,
            max_total_bytes: 8,
        },
        SnapshotLimits {
            max_files: 2,
            max_file_bytes: 4,
            max_total_bytes: 5,
        },
    ] {
        assert_eq!(
            TransactionService::new(limits).prepare_transaction(&s, vec![]),
            Err(TransactionError::Quota)
        );
    }
}

const REQUEST: &[u8] = br#"{"version":{"major":1,"minor":0},"package_id":"P02","request_id":"synthetic-call","snapshot_id":"call-snapshot","payload":{"snapshot":{"id":"call-snapshot","epoch":1,"files":[{"path":"main.tex","revision":7,"content_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","utf8":[90],"dirty":true}]},"edits":[{"document_id":"main.tex","expected_revision":7,"start_utf16":0,"length_utf16":1,"replacement":"x"}]}}"#;
struct ObservedPort {
    inner: TransactionService,
    calls: Cell<usize>,
}
impl TransactionsPort for ObservedPort {
    fn prepare_transaction(
        &self,
        snapshot: &WorkspaceSnapshot,
        edits: Vec<RevisionedEdit>,
    ) -> Result<PreparedTransaction, TransactionError> {
        self.calls.set(self.calls.get() + 1);
        self.inner.prepare_transaction(snapshot, edits)
    }
}
#[derive(Debug, PartialEq)]
enum GuardError {
    Boundary(ContractError),
    Service(TransactionError),
}
// Test-only host seam; the real runtime/authenticated dispatcher is not installed here.
fn guarded(
    port: &impl TransactionsPort,
    context: &CallContext,
    trusted: &WorkspaceSnapshot,
) -> Result<PreparedTransaction, GuardError> {
    let decoded = decode_request::<PrepareTransactionRequest>(REQUEST, context)
        .map_err(GuardError::Boundary)?;
    port.prepare_transaction(trusted, decoded.payload.edits)
        .map_err(GuardError::Service)
}
fn call_snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        id: "call-snapshot".into(),
        epoch: 1,
        files: vec![file("main.tex", Some(7), "A")],
    }
}
#[test]
fn cancel_precondition_through_frozen_decoder() {
    let port = ObservedPort {
        inner: service(),
        calls: Cell::new(0),
    };
    let mut context = CallContext {
        active_snapshot: Some("call-snapshot".into()),
        cancelled: true,
        ..Default::default()
    };
    let trusted = call_snapshot();
    let before = trusted.clone();
    assert_eq!(
        guarded(&port, &context, &trusted),
        Err(GuardError::Boundary(ContractError::Cancelled))
    );
    assert_eq!(port.calls.get(), 0);
    context.cancelled = false;
    assert_eq!(
        guarded(&port, &context, &trusted).unwrap().base_snapshot,
        "call-snapshot"
    );
    assert_eq!(port.calls.get(), 1);
    assert_eq!(trusted, before);
}
#[test]
fn deadline_and_stale_context_preconditions() {
    let port = ObservedPort {
        inner: service(),
        calls: Cell::new(0),
    };
    let trusted = call_snapshot();
    let mut context = CallContext {
        active_snapshot: Some("call-snapshot".into()),
        elapsed_ms: 10,
        deadline_ms: Some(10),
        ..Default::default()
    };
    assert_eq!(
        guarded(&port, &context, &trusted),
        Err(GuardError::Boundary(ContractError::Deadline))
    );
    context.elapsed_ms = 9;
    context.active_snapshot = Some("old-snapshot".into());
    assert_eq!(
        guarded(&port, &context, &trusted),
        Err(GuardError::Boundary(ContractError::StaleSnapshot))
    );
    assert_eq!(port.calls.get(), 0);
    context.active_snapshot = Some("call-snapshot".into());
    assert!(guarded(&port, &context, &trusted).is_ok());
    assert_eq!(port.calls.get(), 1);
}
#[test]
fn overlap_and_equal_start_rejected() {
    let s = snapshot();
    for edits in [
        vec![
            edit("main.tex", 7, 0, 3, "a"),
            edit("main.tex", 7, 1, 2, "b"),
        ],
        vec![
            edit("main.tex", 7, 1, 0, "a"),
            edit("main.tex", 7, 1, 0, "b"),
        ],
    ] {
        assert_eq!(
            service().prepare_transaction(&s, edits),
            Err(TransactionError::InvalidRange)
        );
    }
    assert!(
        service()
            .prepare_transaction(
                &s,
                vec![
                    edit("main.tex", 7, 0, 1, "a"),
                    edit("fejezet ő.tex", 11, 0, 1, "b")
                ]
            )
            .is_ok()
    );
}
#[test]
fn adjacent_edits_allowed_preserve_input_order() {
    let s = snapshot();
    let edits = vec![
        edit("main.tex", 7, 3, 1, "\n\u{feff}"),
        edit("main.tex", 7, 1, 2, "😀"),
        edit("main.tex", 7, 0, 1, "Z"),
    ];
    let before = s.clone();
    assert_eq!(
        service()
            .prepare_transaction(&s, edits.clone())
            .unwrap()
            .edits,
        edits
    );
    assert_eq!(s, before);
}
#[test]
fn deterministic_output_and_complete_binding() {
    let s = snapshot();
    let edits = valid_edits();
    let port = service();
    let p = port.prepare_transaction(&s, edits.clone()).unwrap();
    assert_eq!(p, port.prepare_transaction(&s, edits.clone()).unwrap());
    assert_eq!(p.base_snapshot, s.id);
    assert_eq!(p.edits, edits);
    let mut variants = Vec::new();
    let mut x = s.clone();
    x.epoch += 1;
    variants.push(x);
    let mut x = s.clone();
    x.id.push('x');
    variants.push(x);
    let mut x = s.clone();
    x.files[1].utf8[0] = b'Z';
    variants.push(x); // Same claimed digest; bind actual bytes too.
    let mut x = s.clone();
    x.files[1].dirty = false;
    variants.push(x);
    let mut x = s.clone();
    x.files[1].content_sha256 = "a".repeat(64);
    variants.push(x);
    for changed in variants {
        assert_ne!(
            p.transaction_id,
            port.prepare_transaction(&changed, edits.clone())
                .unwrap()
                .transaction_id
        );
    }
    let mut reordered = s.clone();
    reordered.files.reverse();
    assert_eq!(
        p,
        port.prepare_transaction(&reordered, edits.clone()).unwrap()
    );
    let mut changed = edits.clone();
    changed[0].replacement.push('x');
    assert_ne!(
        p.transaction_id,
        port.prepare_transaction(&s, changed)
            .unwrap()
            .transaction_id
    );
    let mut ordered = edits.clone();
    ordered.reverse();
    assert_ne!(
        p.transaction_id,
        port.prepare_transaction(&s, ordered)
            .unwrap()
            .transaction_id
    );
    let mut max_epoch = s.clone();
    max_epoch.epoch = u64::MAX;
    assert_eq!(
        port.prepare_transaction(&max_epoch, edits.clone()).unwrap(),
        port.prepare_transaction(&max_epoch, edits).unwrap()
    );
}
#[test]
fn malformed_snapshot_and_text_rejected() {
    let s = snapshot();
    for id in ["", "bad\n", &"x".repeat(257)] {
        let mut x = s.clone();
        x.id = id.into();
        assert_eq!(
            service().prepare_transaction(&x, vec![]),
            Err(TransactionError::InvalidRange)
        );
    }
    for path in ["../outside", "/outside", "a//b", "a/./b"] {
        let mut x = s.clone();
        x.files[1].path = path.into();
        assert_eq!(
            service().prepare_transaction(&x, vec![]),
            Err(TransactionError::OutsideRoot)
        );
    }
    for text in [vec![0xff], b"x\0y".to_vec(), b"x\r\ny".to_vec()] {
        let mut x = s.clone();
        x.files[1].utf8 = text;
        assert_eq!(
            service().prepare_transaction(&x, vec![edit("main.tex", 7, 0, 1, "x")]),
            Err(TransactionError::InvalidRange)
        );
    }
    let mut x = s.clone();
    x.files[1].content_sha256 = "A".repeat(64);
    assert_eq!(
        service().prepare_transaction(&x, vec![]),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(
        service().prepare_transaction(&s, vec![edit("main.tex", 7, 0, 0, "x\0y")]),
        Err(TransactionError::InvalidRange)
    );
}
#[test]
fn empty_batch_is_read_only() {
    let host = witness();
    let before = host.clone();
    let port = service();
    let p = port.prepare_transaction(&host.snapshot, vec![]).unwrap();
    assert_eq!(p.edits, vec![]);
    assert_eq!(p.base_snapshot, host.snapshot.id);
    assert_eq!(p, port.prepare_transaction(&host.snapshot, vec![]).unwrap());
    assert_eq!(host, before);
    let mut malformed = host.snapshot.clone();
    malformed.id.clear();
    assert_eq!(
        port.prepare_transaction(&malformed, vec![]),
        Err(TransactionError::InvalidRange)
    );
}
#[test]
fn sha256_known_vectors() {
    let vectors: Vec<(Vec<u8>, &str)> = vec![
        (
            vec![],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".to_vec(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            vec![b'a'; 55],
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            vec![b'a'; 56],
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            vec![b'a'; 64],
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
        (
            (0..=255u8).collect(),
            "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880",
        ),
    ];
    for (input, expected) in vectors {
        assert_eq!(fixture_hash::hex(&input), expected);
    }
}

// Current-main forwardport: real owned service + the core's actual v1 contract re-export.
// These controls exercise immutable values; they never install a Runtime or apply/save text.
#[test]
fn current_main_second_file_stale_revalidation() {
    let port = service();
    let original = snapshot();
    let plan = port.prepare_transaction(&original, valid_edits()).unwrap();
    let mut current = original.clone();
    current.files[1].revision = Some(12);
    let before = current.clone();
    assert_eq!(
        port.revalidate_transaction(&current, &plan),
        Err(TransactionError::StaleRevision)
    );
    assert_eq!(current, before);
    assert_eq!(port.revalidate_transaction(&original, &plan), Ok(()));
    let mut reordered = original.clone();
    reordered.files.reverse();
    assert_eq!(port.revalidate_transaction(&reordered, &plan), Ok(()));
}

#[test]
fn current_main_same_content_new_revision() {
    let port = service();
    let original = snapshot();
    let plan = port.prepare_transaction(&original, valid_edits()).unwrap();
    let mut current = original.clone();
    current.files[0].revision = Some(8);
    assert_eq!(current.files[0].utf8, original.files[0].utf8);
    assert_eq!(current.files[0].content_sha256, original.files[0].content_sha256);
    assert_eq!(
        port.revalidate_transaction(&current, &plan),
        Err(TransactionError::StaleRevision)
    );

    // Bind unedited files and every identity field, even when the claimed digest is unchanged.
    let one_file_plan = port
        .prepare_transaction(&original, vec![edit("main.tex", 7, 0, 1, "Z")])
        .unwrap();
    let mut variants = Vec::new();
    let mut changed = original.clone();
    changed.files[1].revision = Some(12);
    variants.push(changed);
    let mut changed = original.clone();
    changed.files[1].utf8[0] = b'Z';
    variants.push(changed);
    let mut changed = original.clone();
    changed.files[1].content_sha256 = "0".repeat(64);
    variants.push(changed);
    let mut changed = original.clone();
    changed.files[1].dirty = false;
    variants.push(changed);
    let mut changed = original.clone();
    changed.epoch += 1;
    variants.push(changed);
    let mut changed = original.clone();
    changed.id.push('x');
    variants.push(changed);
    for changed in variants {
        let before = changed.clone();
        assert_eq!(
            port.revalidate_transaction(&changed, &one_file_plan),
            Err(TransactionError::StaleRevision)
        );
        assert_eq!(changed, before);
    }
    assert_eq!(original, snapshot());
}

#[test]
fn current_main_missing_document() {
    let port = service();
    let original = snapshot();
    let plan = port.prepare_transaction(&original, valid_edits()).unwrap();
    let mut current = original.clone();
    current.files.remove(1);
    let before = current.clone();
    assert_eq!(
        port.revalidate_transaction(&current, &plan),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(current, before);
    assert_eq!(port.revalidate_transaction(&original, &plan), Ok(()));
}

#[test]
fn current_main_aggregate_projected_quota() {
    let mut s = snapshot();
    s.files = vec![file("main.tex", Some(7), "abc"), file("b.tex", Some(11), "def")];
    let limits = |max_total_bytes| SnapshotLimits {
        max_files: 2,
        max_file_bytes: 4,
        max_total_bytes,
    };
    let edits = vec![edit("main.tex", 7, 3, 0, "x"), edit("b.tex", 11, 3, 0, "y")];
    let generous = TransactionService::new(limits(8));
    let strict = TransactionService::new(limits(7));
    let plan = generous.prepare_transaction(&s, edits.clone()).unwrap();
    let before = s.clone();
    assert_eq!(strict.revalidate_transaction(&s, &plan), Err(TransactionError::Quota));
    // Each single edit fits; their combined projected total exceeds seven bytes.
    for edit in &edits {
        assert!(strict.prepare_transaction(&s, vec![edit.clone()]).is_ok());
    }
    assert_eq!(strict.prepare_transaction(&s, edits), Err(TransactionError::Quota));
    assert_eq!(generous.revalidate_transaction(&s, &plan), Ok(()));
    assert_eq!(s, before);
}

#[test]
fn current_main_astral_range_revalidation() {
    let port = service();
    let s = snapshot();
    let plan = port.prepare_transaction(&s, valid_edits()).unwrap();
    let mut split_surrogate = plan.clone();
    split_surrogate.edits[0].start_utf16 = 2;
    split_surrogate.edits[0].length_utf16 = 0;
    let before = s.clone();
    assert_eq!(
        port.revalidate_transaction(&s, &split_surrogate),
        Err(TransactionError::InvalidRange)
    );
    assert_eq!(port.revalidate_transaction(&s, &plan), Ok(()));
    assert_eq!(s, before);
    assert_eq!(plan.edits[0].start_utf16, 1);
    assert_eq!(plan.edits[0].length_utf16, 2);
}

#[test]
fn current_main_empty_and_noop_plan() {
    let port = service();
    let s = snapshot();
    let empty = port.prepare_transaction(&s, vec![]).unwrap();
    let mut wrong_base = empty.clone();
    wrong_base.base_snapshot.push('x');
    assert_eq!(
        port.revalidate_transaction(&s, &wrong_base),
        Err(TransactionError::StaleRevision)
    );
    let before = s.clone();
    assert_eq!(port.revalidate_transaction(&s, &empty), Ok(()));
    let noop = port
        .prepare_transaction(&s, vec![edit("main.tex", 7, 0, 1, "A")])
        .unwrap();
    assert_eq!(port.revalidate_transaction(&s, &noop), Ok(()));
    let mut wrong_label = noop.clone();
    wrong_label.transaction_id.push('0');
    assert_eq!(
        port.revalidate_transaction(&s, &wrong_label),
        Err(TransactionError::StaleRevision)
    );
    assert_eq!(s, before);
    let empty_workspace = WorkspaceSnapshot { id: "empty-main".into(), epoch: 0, files: vec![] };
    let plan = port.prepare_transaction(&empty_workspace, vec![]).unwrap();
    assert_eq!(port.revalidate_transaction(&empty_workspace, &plan), Ok(()));
    let mut newer = empty_workspace.clone();
    newer.epoch = 1;
    assert_eq!(port.revalidate_transaction(&newer, &plan), Err(TransactionError::StaleRevision));
}

#[test]
fn external_api_fresh_snapshot_prepare_and_revalidate() {
    let port = latexide_core::services::transactions::TransactionService::new(SnapshotLimits {
        max_files: 2,
        max_file_bytes: 32,
        max_total_bytes: 64,
    });
    let nonce = std::env::var("P02_RUN_NONCE").unwrap_or_else(|_| "standalone-canonical".into());
    let fresh = WorkspaceSnapshot {
        id: format!("external-fresh-{nonce}"),
        epoch: 3,
        files: vec![
            file("main.tex", Some(1), "\u{feff}😀\nA"),
            file("notes.tex", Some(9), "képlet\n"),
        ],
    };
    let before = fresh.clone();
    let edits = vec![edit("main.tex", 1, 1, 2, "ő"), edit("notes.tex", 9, 0, 1, "K")];
    let plan = port.prepare_transaction(&fresh, edits.clone()).unwrap();
    assert_eq!(plan.base_snapshot, fresh.id);
    assert_eq!(plan.edits, edits);
    assert_eq!(port.revalidate_transaction(&fresh, &plan), Ok(()));
    assert_eq!(fresh, before);
    assert_eq!(fresh.files[0].revision, Some(1));
    assert_eq!(fresh.files[1].revision, Some(9));
}

#[test]
fn external_api_stale_revalidation_then_fresh_plan() {
    let port = latexide_core::services::transactions::TransactionService::new(SnapshotLimits {
        max_files: 128,
        max_file_bytes: MAX_FILE_BYTES,
        max_total_bytes: 8 * MAX_FILE_BYTES,
    });
    let original = snapshot();
    let old = port.prepare_transaction(&original, valid_edits()).unwrap();
    let old_before = old.clone();
    let mut current = original.clone();
    current.files[1].revision = Some(12);
    current.epoch += 1;
    current.id.push_str("-new");
    let before = current.clone();
    assert_eq!(current.files[1].utf8, original.files[1].utf8);
    assert_eq!(
        port.revalidate_transaction(&current, &old),
        Err(TransactionError::StaleRevision)
    );
    let mut edits = valid_edits();
    edits[1].expected_revision = 12;
    let fresh = port.prepare_transaction(&current, edits).unwrap();
    assert_eq!(port.revalidate_transaction(&current, &fresh), Ok(()));
    assert_ne!(fresh.transaction_id, old.transaction_id);
    assert_eq!(old, old_before);
    assert_eq!(current, before);
}
