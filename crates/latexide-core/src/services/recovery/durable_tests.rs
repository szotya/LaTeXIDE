use crate::recovery_service::durable::*;
use crate::{R7, checkpoint, r7};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::PathBuf};
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn fixture(name: &str) -> PathBuf {
    let root = scratch().join("tmp").join(format!(
        "{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}
fn nonce() -> String {
    static NONCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NONCE
        .get_or_init(|| {
            std::env::var("P01_RUN_NONCE").unwrap_or_else(|_| {
                use std::io::Read;
                let mut bytes = [0u8; 32];
                std::fs::File::open("/dev/urandom")
                    .unwrap()
                    .read_exact(&mut bytes)
                    .unwrap();
                bytes.iter().map(|b| format!("{b:02x}")).collect()
            })
        })
        .clone()
}
fn scratch() -> PathBuf {
    if let Ok(path) = std::env::var("P01_DURABLE_SCRATCH") {
        return path.into();
    }
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| {
        let p = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "latexide-recovery-{}-{}",
            std::process::id(),
            &nonce()[..12]
        ));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(p.join("tmp")).unwrap();
        p
    })
    .clone()
}
fn scope() -> StoreScope {
    StoreScope {
        project_id: "synthetic-project".into(),
        session_id: "synthetic-session".into(),
        nonce: nonce(),
    }
}

#[test]
fn durable_initial_directory_rejects_ancestor_link_before_outside_side_effects() {
    use std::os::unix::fs::symlink;
    let root = fixture("ancestor-initial-directory");
    let outside = root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("sentinel"), b"unchanged").unwrap();
    symlink(&outside, root.join("parent")).unwrap();
    assert!(ensure_private_path(&root.join("parent/missing/recovery")).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
}

#[test]
fn durable_first_slice_real_write_reopen_r7() {
    let parent = fixture("first-red-green");
    let (store, a6) = JournalStore::create(
        &parent,
        "store",
        scope(),
        checkpoint(),
        vec![Encoding {
            bom: false,
            crlf: false,
        }],
    )
    .expect("actual private checkpoint write must succeed");
    let a7 = store.append(&a6, r7()).unwrap();
    drop(store);
    let store = JournalStore::open(&parent.join("store"), scope()).unwrap();
    let recovered = store.strict(&a7).unwrap();
    assert_eq!(recovered.documents[0].utf8, R7);
    assert_eq!(recovered.documents[0].revision, 7);
    assert!(recovered.documents[0].dirty);
    assert_eq!(recovered.last_sequence, 7);
}

use latexide_contracts::recovery::{JournalEntry, RecoveredDocument, RecoveryError};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, symlink};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn setup(name: &str) -> (JournalStore, AckReceipt) {
    let p = fixture(name);
    let (s, a) = JournalStore::create(
        &p,
        "store",
        scope(),
        checkpoint(),
        vec![Encoding {
            bom: false,
            crlf: false,
        }],
    )
    .unwrap();
    let a = s.append(&a, r7()).unwrap();
    (s, a)
}
fn e8() -> JournalEntry {
    let text = "még nem nyugtázott r8 😀\n".as_bytes();
    JournalEntry {
        sequence: 8,
        document_id: "main.tex".into(),
        revision: 8,
        utf8: text.to_vec(),
        checksum: raw_digest(text),
    }
}
fn jp(s: &JournalStore, a: &AckReceipt) -> PathBuf {
    s.root().join(format!("journal-{}.rec", a.generation))
}
fn cp(s: &JournalStore, a: &AckReceipt) -> PathBuf {
    s.root().join(format!("checkpoint-{}.rec", a.generation))
}
fn private_write(p: &std::path::Path, bytes: &[u8]) {
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(p)
        .unwrap();
    f.write_all(bytes).unwrap();
    f.flush().unwrap();
    f.sync_all().unwrap();
}
fn offsets(raw: &[u8]) -> Vec<usize> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < raw.len() {
        out.push(at);
        let n = u32::from_le_bytes(raw[at + 13..at + 17].try_into().unwrap()) as usize;
        at += 17 + n + 64;
    }
    assert_eq!(at, raw.len());
    out
}
fn scope_end(raw: &[u8], frame: usize) -> usize {
    let mut p = frame + 17;
    for _ in 0..3 {
        let n = u32::from_le_bytes(raw[p..p + 4].try_into().unwrap()) as usize;
        p += 4 + n;
    }
    p
}
fn seal(raw: &mut [u8], frame: usize) {
    let n = u32::from_le_bytes(raw[frame + 13..frame + 17].try_into().unwrap()) as usize;
    let end = frame + 17 + n;
    let h = raw_digest(&raw[frame..end]);
    raw[end..end + 64].copy_from_slice(h.as_bytes());
}
fn reseal_ack(a: &AckReceipt, raw: &[u8]) -> AckReceipt {
    let mut a = a.clone();
    a.journal_bytes = raw.len() as u64;
    a.journal_sha256 = raw_digest(raw);
    a.last_frame_sha256 = String::from_utf8(raw[raw.len() - 64..].to_vec()).unwrap();
    a
}

#[test]
fn durable_modes_actual_reopen_and_observed_receipt_roundtrip() {
    let (s, a) = setup("modes");
    assert_eq!(fs::metadata(s.root()).unwrap().mode() & 0o777, 0o700);
    for p in [jp(&s, &a), cp(&s, &a)] {
        let m = fs::metadata(p).unwrap();
        assert_eq!(m.mode() & 0o777, 0o600);
        assert_eq!(m.nlink(), 1);
    }
    let raw = a.encode().unwrap();
    assert_eq!(AckReceipt::decode(&raw).unwrap(), a);
    let reopened = JournalStore::open(s.root(), scope()).unwrap();
    assert_eq!(reopened.strict(&a).unwrap().documents[0].utf8, R7);
}

#[test]
fn durable_actual_tail_is_never_implicitly_acknowledged_or_truncated() {
    for tail in [
        b"L".as_slice(),
        b"LXRECOV\0\x01\0\0\0\x03".as_slice(),
        b"synthetic torn payload".as_slice(),
    ] {
        let (s, a) = setup("torn-tail");
        let path = jp(&s, &a);
        let mut raw = fs::read(&path).unwrap();
        raw.extend(tail);
        private_write(&path, &raw);
        let strict = s.strict(&a).unwrap_err();
        assert_eq!(strict.v1_error(), RecoveryError::Corrupt);
        let r = s.recover_explicit_prefix(&a).unwrap();
        assert_eq!(r.original_error, strict);
        assert_eq!(r.v1_error, RecoveryError::Corrupt);
        assert_eq!(r.recovered.recovery.documents[0].utf8, R7);
        assert_eq!(r.rejected_tail_bytes, tail.len() as u64);
        assert_eq!(r.rejected_tail_sha256, raw_digest(tail));
        assert_eq!(
            r.acknowledged_receipt_sha256,
            raw_digest(&a.encode().unwrap())
        );
        assert_eq!(fs::read(path).unwrap(), raw);
        assert!(s.append(&a, e8()).is_err());
    }
}

#[test]
fn durable_fully_written_r8_needs_a_separate_observed_ack() {
    let (s, a7) = setup("not-observed");
    let a8 = s.append(&a7, e8()).unwrap();
    assert!(s.strict(&a7).is_err());
    assert_eq!(
        s.recover_explicit_prefix(&a7)
            .unwrap()
            .recovered
            .recovery
            .last_sequence,
        7
    );
    assert_eq!(s.strict(&a8).unwrap().last_sequence, 8);
    assert!(s.recover_explicit_prefix(&a8).is_err());
    let mut forged = a8;
    forged.sequence = 9;
    assert!(s.strict(&forged).is_err());
}

#[test]
fn durable_checkpoint_and_raw_receipt_reject_truncated_head_payload_checksum() {
    let (s, a) = setup("cp-truncation");
    let path = cp(&s, &a);
    let original = fs::read(&path).unwrap();
    for n in [
        0,
        1,
        7,
        8,
        12,
        16,
        17,
        original.len() - 65,
        original.len() - 64,
        original.len() - 1,
    ] {
        private_write(&path, &original[..n]);
        assert!(s.strict(&a).is_err());
        assert!(s.recover_explicit_prefix(&a).is_err());
    }
    private_write(&path, &original);
    let bytes = a.encode().unwrap();
    for n in [0, 1, 7, 12, 16, bytes.len() - 65, bytes.len() - 1] {
        assert!(AckReceipt::decode(&bytes[..n]).is_err());
    }
    let mut bad = bytes.clone();
    bad[8..12].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        AckReceipt::decode(&bad),
        Err(DurableError::UnsupportedVersion(2))
    );
    let mut bad = bytes;
    bad[13..17].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(AckReceipt::decode(&bad), Err(DurableError::Quota));
}

#[test]
fn durable_raw_framing_binds_metadata_scope_sequence_revision_and_content() {
    for case in [
        "gap",
        "revision",
        "unknown-doc",
        "scope",
        "generation",
        "encoding",
        "payload",
        "checksum",
        "version",
        "kind",
        "length",
    ] {
        let (s, a) = setup(case);
        let path = jp(&s, &a);
        let mut raw = fs::read(&path).unwrap();
        let frame = offsets(&raw)[1];
        let start = scope_end(&raw, frame);
        let seq = start + 8 + 4 + 64;
        let id = seq + 8 + 4;
        let rev = id + 8;
        match case {
            "gap" => raw[seq..seq + 8].copy_from_slice(&9u64.to_le_bytes()),
            "revision" => raw[rev..rev + 8].copy_from_slice(&9u64.to_le_bytes()),
            "unknown-doc" => raw[id..id + 8].copy_from_slice(b"evil.tex"),
            "scope" => raw[frame + 21] = b'X',
            "generation" => raw[start..start + 8].copy_from_slice(&2u64.to_le_bytes()),
            "encoding" => raw[rev + 8] = 1,
            "payload" => raw[rev + 8 + 2 + 4] ^= 1,
            "checksum" => {
                let n = raw.len();
                raw[n - 1] ^= 1;
            }
            "version" => raw[frame + 8..frame + 12].copy_from_slice(&2u32.to_le_bytes()),
            "kind" => raw[frame + 12] = 99,
            "length" => raw[frame + 13..frame + 17].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => unreachable!(),
        }
        if !["checksum", "payload", "length"].contains(&case) {
            seal(&mut raw, frame);
        }
        // Re-seal the *fixture* receipt to exercise parser/semantic checks after valid
        // byte binding. This is not evidence that a caller received this forged ACK.
        let forged = reseal_ack(&a, &raw);
        private_write(&path, &raw);
        let error = s.strict(&forged).expect_err(case);
        if case == "version" {
            assert_eq!(error, DurableError::UnsupportedVersion(2));
        }
        if case == "length" {
            assert_eq!(error, DurableError::Quota);
        }
    }
}

#[test]
fn durable_raw_duplicate_reordered_and_missing_records_fail_closed() {
    for case in ["duplicate", "reorder", "missing"] {
        let (s, a7) = setup(case);
        let a8 = s.append(&a7, e8()).unwrap();
        let path = jp(&s, &a8);
        let raw = fs::read(&path).unwrap();
        let off = offsets(&raw);
        let head = &raw[..off[1]];
        let first = &raw[off[1]..off[2]];
        let second = &raw[off[2]..];
        let raw = match case {
            "duplicate" => [head, first, first].concat(),
            "reorder" => [head, second, first].concat(),
            _ => [head, second].concat(),
        };
        let forged = reseal_ack(&a8, &raw);
        private_write(&path, &raw);
        assert_eq!(
            s.strict(&forged).unwrap_err().v1_error(),
            RecoveryError::Corrupt
        );
    }
}

#[test]
fn durable_producer_rejects_semantic_errors_before_writing() {
    for (name, mut e) in [
        ("gap", e8()),
        ("revision", e8()),
        ("unknown", e8()),
        ("checksum", e8()),
    ] {
        match name {
            "gap" => e.sequence = 9,
            "revision" => e.revision = 9,
            "unknown" => e.document_id = "unknown.tex".into(),
            _ => e.checksum = "0".repeat(64),
        }
        let (s, a) = setup(name);
        let before = fs::read(jp(&s, &a)).unwrap();
        assert!(s.append(&a, e).is_err());
        assert_eq!(fs::read(jp(&s, &a)).unwrap(), before);
        assert_eq!(s.strict(&a).unwrap().last_sequence, 7);
    }
}

#[test]
fn durable_wrong_scope_version_receipt_and_byte_substitution_rejected() {
    let (s, a) = setup("scope-receipt");
    let mut wrong = scope();
    wrong.session_id = "other-session".into();
    assert_eq!(
        JournalStore::open(s.root(), wrong)
            .unwrap()
            .strict(&a)
            .unwrap_err(),
        DurableError::WrongScope
    );
    let mut wrong = a.clone();
    wrong.scope.nonce = "a".repeat(64);
    assert_eq!(s.strict(&wrong).unwrap_err(), DurableError::WrongScope);
    let mut wrong = a.clone();
    wrong.checkpoint_sha256 = "0".repeat(64);
    assert!(s.strict(&wrong).is_err());
    let mut wrong = a.clone();
    wrong.journal_bytes += 1;
    assert!(s.strict(&wrong).is_err());
    let mut wrong = a.clone();
    wrong.last_frame_sha256 = "0".repeat(64);
    assert!(s.strict(&wrong).is_err());
    let mut bytes = a.encode().unwrap();
    let n = bytes.len();
    bytes[n - 1] ^= 1;
    assert!(AckReceipt::decode(&bytes).is_err());
}

#[test]
fn durable_unicode_feff_bom_crlf_metadata_multiple_documents_rotate_without_loss() {
    let p = fixture("unicode");
    let mut initial = checkpoint();
    initial.documents[0].utf8 = "\u{feff}a\u{301}😀\n".as_bytes().to_vec();
    initial.documents.push(RecoveredDocument {
        document_id: "árvíz.tex".into(),
        revision: 40,
        utf8: b"clean\n".to_vec(),
        dirty: false,
    });
    let enc = vec![
        Encoding {
            bom: true,
            crlf: true,
        },
        Encoding {
            bom: false,
            crlf: false,
        },
    ];
    let (s, a6) = JournalStore::create(&p, "store", scope(), initial.clone(), enc.clone()).unwrap();
    let a7 = s.append(&a6, r7()).unwrap();
    let a7new = s.checkpoint(&a7).unwrap();
    assert_eq!(a7new.generation, 2);
    assert_eq!(a7new.sequence, 7);
    assert_eq!(s.read_with_metadata(&a7new).unwrap().encodings, enc);
    assert_eq!(s.strict(&a7new).unwrap().documents[1], initial.documents[1]);
    assert_eq!(s.strict(&a7).unwrap(), s.strict(&a7new).unwrap());
    assert!(cp(&s, &a7).exists() && jp(&s, &a7).exists());
    let mut e = e8();
    e.document_id = "árvíz.tex".into();
    e.revision = 41;
    e.utf8 = "\u{feff}még 😀\n".as_bytes().to_vec();
    e.checksum = raw_digest(&e.utf8);
    let a8 = s.append(&a7new, e.clone()).unwrap();
    let r = s.read_with_metadata(&a8).unwrap();
    assert_eq!(r.recovery.documents[1].utf8, e.utf8);
    assert_eq!(r.encodings, enc);
    assert_eq!(r.recovery.documents[0].revision, 7);
}

#[test]
fn durable_storage_and_encoded_file_quotas_preserve_previous_receipt() {
    let p = fixture("quota");
    let mut c = checkpoint();
    c.documents[0].utf8 = vec![b'a'; 1_048_576];
    let e = JournalStore::create(
        &p,
        "too-large-encoded",
        scope(),
        c,
        vec![Encoding {
            bom: true,
            crlf: false,
        }],
    )
    .unwrap_err();
    assert_eq!(e.v1_error(), RecoveryError::Quota);
    let mut c = checkpoint();
    c.documents = (0..11)
        .map(|i| RecoveredDocument {
            document_id: format!("d{i}.tex"),
            revision: 1,
            utf8: vec![],
            dirty: true,
        })
        .collect();
    assert_eq!(
        JournalStore::create(
            &p,
            "too-many",
            scope(),
            c,
            vec![
                Encoding {
                    bom: false,
                    crlf: false
                };
                11
            ]
        )
        .unwrap_err()
        .v1_error(),
        RecoveryError::Quota
    );
    let (s, a) = setup("append-quota");
    let mut e = e8();
    e.utf8 = vec![b'a'; 1_048_577];
    e.checksum = raw_digest(&e.utf8);
    assert_eq!(
        s.append(&a, e).unwrap_err().v1_error(),
        RecoveryError::Quota
    );
    assert_eq!(s.strict(&a).unwrap().last_sequence, 7);
    let mut bad = a.clone();
    bad.journal_bytes = MAX_STORAGE_BYTES as u64 + 1;
    assert_eq!(
        s.strict(&bad).unwrap_err(),
        DurableError::Corrupt("ack receipt shape")
    );
}

#[test]
fn durable_symlink_path_escape_permission_and_hardlink_fixtures_rejected() {
    let p = fixture("path-fixture");
    let outside = p.join("own-sentinel");
    private_write(&outside, b"own synthetic sentinel");
    let err = JournalStore::create(
        &p,
        "../escape",
        scope(),
        checkpoint(),
        vec![Encoding {
            bom: false,
            crlf: false,
        }],
    )
    .unwrap_err();
    assert_eq!(err, DurableError::UnsafePath);
    symlink(&p, p.join("alias")).unwrap();
    assert_eq!(
        JournalStore::open(&p.join("alias"), scope()).unwrap_err(),
        DurableError::UnsafePath
    );
    for name in ["checkpoint", "journal", "hardlink", "mode"] {
        let (s, a) = setup(name);
        let path = if name == "checkpoint" {
            cp(&s, &a)
        } else {
            jp(&s, &a)
        };
        if name == "mode" {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
            if name == "hardlink" {
                fs::hard_link(&outside, &path).unwrap();
            } else {
                symlink(&outside, &path).unwrap();
            }
        }
        assert_eq!(s.strict(&a).unwrap_err(), DurableError::UnsafePath);
    }
    assert_eq!(fs::read(outside).unwrap(), b"own synthetic sentinel");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        JournalStore::open(&p, scope()).unwrap_err(),
        DurableError::UnsafePath
    );
}

struct Inject {
    phase: Phase,
    kind: std::io::ErrorKind,
    seen: Vec<Phase>,
}
impl Hooks for Inject {
    fn reached(&mut self, p: Phase) -> std::io::Result<()> {
        self.seen.push(p);
        if p == self.phase {
            Err(std::io::Error::from(self.kind))
        } else {
            Ok(())
        }
    }
}
#[test]
fn durable_injected_write_sync_rename_parent_errors_never_return_ack() {
    for (phase, checkpoint_op, kind) in [
        (Phase::JournalWrite, false, std::io::ErrorKind::StorageFull),
        (
            Phase::JournalSync,
            false,
            std::io::ErrorKind::PermissionDenied,
        ),
        (Phase::JournalParentSync, false, std::io::ErrorKind::Other),
        (
            Phase::CheckpointWrite,
            true,
            std::io::ErrorKind::StorageFull,
        ),
        (Phase::CheckpointSync, true, std::io::ErrorKind::Other),
        (
            Phase::CheckpointRename,
            true,
            std::io::ErrorKind::PermissionDenied,
        ),
        (Phase::CheckpointParentSync, true, std::io::ErrorKind::Other),
    ] {
        let (s, a) = setup("injected");
        let mut h = Inject {
            phase,
            kind,
            seen: vec![],
        };
        let result = if checkpoint_op {
            s.checkpoint_with(&a, &mut h)
        } else {
            s.append_with(&a, e8(), &mut h)
        };
        assert_eq!(result, Err(DurableError::Io { phase, kind }));
        assert_eq!(h.seen.last(), Some(&phase));
        let r = match s.strict(&a) {
            Ok(r) => r,
            Err(_) => s.recover_explicit_prefix(&a).unwrap().recovered.recovery,
        };
        assert_eq!(r.last_sequence, 7);
        assert_eq!(r.documents[0].utf8, R7);
    }
}

struct Trace(Vec<Phase>);
impl Hooks for Trace {
    fn reached(&mut self, p: Phase) -> std::io::Result<()> {
        self.0.push(p);
        Ok(())
    }
}
#[test]
fn durable_write_flush_sync_rename_parent_sync_precedes_ack_in_actual_io() {
    let (s, a) = setup("order");
    let mut h = Trace(vec![]);
    let a8 = s.append_with(&a, e8(), &mut h).unwrap();
    assert_eq!(
        h.0,
        vec![
            Phase::JournalWrite,
            Phase::JournalPartiallyWritten,
            Phase::JournalWritten,
            Phase::JournalFlushed,
            Phase::JournalSync,
            Phase::JournalSynced,
            Phase::JournalParentSync,
            Phase::JournalParentSynced,
            Phase::BeforeAck
        ]
    );
    assert_eq!(s.strict(&a8).unwrap().last_sequence, 8);
    let mut h = Trace(vec![]);
    s.checkpoint_with(&a8, &mut h).unwrap();
    assert!(h.0.windows(3).any(|w| w
        == [
            Phase::CheckpointRenamed,
            Phase::CheckpointParentSync,
            Phase::CheckpointParentSynced
        ]));
    assert_eq!(h.0.last(), Some(&Phase::BeforeAck));
}

unsafe extern "C" {
    fn _exit(code: i32) -> !;
}
fn own_exit(code: i32) -> ! {
    unsafe { _exit(code) }
}
struct ExitAt {
    phase: String,
    trace: PathBuf,
}
impl Hooks for ExitAt {
    fn reached(&mut self, p: Phase) -> std::io::Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.trace)?;
        writeln!(f, "{p:?}")?;
        f.flush()?;
        if self.phase == format!("{p:?}") {
            own_exit(73)
        }
        Ok(())
    }
}
fn observe_ack(parent: &std::path::Path, name: &str, a: &AckReceipt) {
    private_write(&parent.join(name), &a.encode().unwrap());
    File::open(parent).unwrap().sync_all().unwrap();
}
#[test]
#[ignore = "own subprocess fixture driver; invoked explicitly with --ignored and nonce"]
fn durable_crash_child() {
    let root = PathBuf::from(std::env::var("P01_CHILD_ROOT").expect("owned parent root"));
    let mode = std::env::var("P01_CHILD_MODE").unwrap();
    let phase = std::env::var("P01_CHILD_PHASE").unwrap();
    assert!(root.starts_with(scratch()));
    assert!(nonce().len() == 64);
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(9));
        own_exit(124)
    });
    let (s, a6) = JournalStore::create(
        &root,
        "store",
        scope(),
        checkpoint(),
        vec![Encoding {
            bom: false,
            crlf: false,
        }],
    )
    .unwrap();
    let a7 = s.append(&a6, r7()).unwrap();
    observe_ack(&root, "observed-r7.receipt", &a7);
    let mut h = ExitAt {
        phase,
        trace: root.join("phases.log"),
    };
    let ack = if mode == "append" {
        s.append_with(&a7, e8(), &mut h).unwrap()
    } else {
        assert_eq!(mode, "checkpoint");
        s.checkpoint_with(&a7, &mut h).unwrap()
    };
    observe_ack(&root, "observed-after-return.receipt", &ack);
    own_exit(73)
}
#[test]
fn durable_own_child_exit_matrix_preserves_last_caller_observed_ack() {
    let append = [
        "JournalWrite",
        "JournalPartiallyWritten",
        "JournalWritten",
        "JournalFlushed",
        "JournalSync",
        "JournalSynced",
        "JournalParentSync",
        "JournalParentSynced",
        "BeforeAck",
        "after_return",
    ];
    let checkpoint = [
        "CheckpointWrite",
        "CheckpointPartiallyWritten",
        "CheckpointWritten",
        "CheckpointFlushed",
        "CheckpointSync",
        "CheckpointSynced",
        "CheckpointRename",
        "CheckpointRenamed",
        "CheckpointParentSync",
        "CheckpointParentSynced",
        "JournalWrite",
        "JournalPartiallyWritten",
        "JournalWritten",
        "JournalFlushed",
        "JournalSync",
        "JournalSynced",
        "JournalRename",
        "JournalRenamed",
        "JournalParentSync",
        "JournalParentSynced",
        "BeforeAck",
        "after_return",
    ];
    let exe = std::env::current_exe().unwrap();
    let exe_digest = raw_digest(&fs::read(&exe).unwrap());
    let mut records = String::new();
    let mut count = 0;
    for (mode, phases) in [
        ("append", append.as_slice()),
        ("checkpoint", checkpoint.as_slice()),
    ] {
        for phase in phases {
            let root = fixture(&format!("child-{mode}-{phase}"));
            let stdout = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(root.join("child.log"))
                .unwrap();
            let stderr = stdout.try_clone().unwrap();
            let mut child = Command::new(&exe)
                .args([
                    "--ignored",
                    "--exact",
                    "durable_contract::durable_crash_child",
                    "--nocapture",
                ])
                .env_clear()
                .env("P01_CHILD_ROOT", &root)
                .env("P01_CHILD_MODE", mode)
                .env("P01_CHILD_PHASE", phase)
                .env("P01_DURABLE_SCRATCH", scratch())
                .env("P01_RUN_NONCE", nonce())
                .env("TMPDIR", std::env::var("TMPDIR").unwrap())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .unwrap();
            let start = Instant::now();
            let status = loop {
                if let Some(s) = child.try_wait().unwrap() {
                    break s;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "own child deadline; its self-exit watchdog is 9s"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(
                status.code(),
                Some(73),
                "{mode}/{phase}: {}",
                fs::read_to_string(root.join("child.log")).unwrap()
            );
            assert!(start.elapsed() < Duration::from_secs(10));
            let a7 =
                AckReceipt::decode(&fs::read(root.join("observed-r7.receipt")).unwrap()).unwrap();
            let after = root.join("observed-after-return.receipt");
            assert_eq!(after.exists(), *phase == "after_return");
            let a = if after.exists() {
                AckReceipt::decode(&fs::read(&after).unwrap()).unwrap()
            } else {
                a7.clone()
            };
            let s = JournalStore::open(&root.join("store"), scope()).unwrap();
            let strict = s.strict(&a);
            let rejected = strict.is_err();
            let r = match strict {
                Ok(r) => r,
                Err(e) => {
                    assert_eq!(e.v1_error(), RecoveryError::Corrupt);
                    let p = s.recover_explicit_prefix(&a).unwrap();
                    assert_eq!(p.original_error, e);
                    p.recovered.recovery
                }
            };
            let expected = if mode == "append" && *phase == "after_return" {
                8
            } else {
                7
            };
            assert_eq!(r.last_sequence, expected);
            assert_eq!(r.documents[0].revision, expected);
            assert!(r.documents[0].dirty);
            if expected == 7 {
                assert_eq!(r.documents[0].utf8, R7);
            }
            records.push_str(&format!("mode={mode} phase={phase} exit=73 observed_sequence={} recovered_sequence={} strict_rejected={} duration_ms={} binary_sha256={} nonce={} fixture={}\n",a.sequence,r.last_sequence,rejected,start.elapsed().as_millis(),exe_digest,nonce(),root.display()));
            count += 1;
        }
    }
    assert_eq!(count, 32);
    let p = fixture("child-matrix-summary");
    private_write(&p.join("32-process-boundary-cases.txt"), records.as_bytes());
    println!(
        "P01_CHILD_CASES=32 PASS=32 FAIL=0 ERROR=0 SKIP=0 nonce={} evidence={}",
        nonce(),
        p.display()
    );
}

#[test]
fn durable_acknowledged_journal_cut_is_not_salvaged() {
    let (s, a) = setup("acknowledged-cut");
    let path = jp(&s, &a);
    let raw = fs::read(&path).unwrap();
    let off = offsets(&raw)[1];
    for n in [
        0,
        1,
        7,
        12,
        16,
        17,
        off + 1,
        off + 17,
        raw.len() - 65,
        raw.len() - 1,
    ] {
        private_write(&path, &raw[..n]);
        assert_eq!(s.strict(&a).unwrap_err().v1_error(), RecoveryError::Corrupt);
        assert!(s.recover_explicit_prefix(&a).is_err());
    }
}

#[test]
fn durable_quota_retained_generations_are_bounded_without_deleting_old_ack() {
    let p = fixture("retained-quota");
    let mut c = checkpoint();
    c.documents = (0..8)
        .map(|n| RecoveredDocument {
            document_id: format!("d{n}.tex"),
            revision: 1,
            utf8: vec![b'a'; 1_048_576],
            dirty: true,
        })
        .collect();
    let (s, a) = JournalStore::create(
        &p,
        "store",
        scope(),
        c,
        vec![
            Encoding {
                bom: false,
                crlf: false
            };
            8
        ],
    )
    .unwrap();
    let before = fs::read_dir(s.root()).unwrap().count();
    assert_eq!(s.checkpoint(&a).unwrap_err(), DurableError::Quota);
    assert_eq!(fs::read_dir(s.root()).unwrap().count(), before);
    assert_eq!(s.strict(&a).unwrap().documents.len(), 8);
}
#[test]
fn durable_quota_1025th_record_is_rejected_before_storage_append() {
    let (s, a) = setup("record-quota");
    let path = jp(&s, &a);
    let raw = fs::read(&path).unwrap();
    let off = offsets(&raw)[1];
    let mut output = raw[..off].to_vec();
    let mut previous = String::from_utf8(output[output.len() - 64..].to_vec()).unwrap();
    for sequence in 7u64..1031 {
        let mut f = raw[off..].to_vec();
        let start = scope_end(&f, 0);
        let seq = start + 8 + 4 + 64;
        let rev = seq + 8 + 4 + 8;
        f[start + 8 + 4..start + 8 + 4 + 64].copy_from_slice(previous.as_bytes());
        f[seq..seq + 8].copy_from_slice(&sequence.to_le_bytes());
        f[rev..rev + 8].copy_from_slice(&sequence.to_le_bytes());
        seal(&mut f, 0);
        previous = String::from_utf8(f[f.len() - 64..].to_vec()).unwrap();
        output.extend(f);
    }
    let mut forged = reseal_ack(&a, &output);
    forged.sequence = 1030;
    private_write(&path, &output);
    assert_eq!(s.strict(&forged).unwrap().last_sequence, 1030);
    let mut e = e8();
    e.sequence = 1031;
    e.revision = 1031;
    assert_eq!(
        s.append(&forged, e).unwrap_err().v1_error(),
        RecoveryError::Quota
    );
    assert!(
        fs::read(&path).unwrap() == output,
        "record quota must reject before write"
    );
}
