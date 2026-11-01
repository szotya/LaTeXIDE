#[path = "../src/services/recovery/mod.rs"]
mod recovery_service;

use latexide_contracts::recovery::{
    Checkpoint, DiskIdentity, JournalEntry, RecoveredDocument, RecoveryError, RecoveryPort,
};
use recovery_service::RecoveryService;

const R7: &[u8] = "nyugtázott r7 😀\n".as_bytes();
const R7_HASH: &str = "ad668bdd98f502c2e9aaacfdcea2e92dd93ddd766c787596ef7cf94673a16718";
fn checkpoint() -> Checkpoint {
    Checkpoint {
        schema: 1,
        sequence: 6,
        documents: vec![RecoveredDocument {
            document_id: "main.tex".into(),
            revision: 6,
            utf8: b"r6\n".to_vec(),
            dirty: true,
        }],
    }
}
fn r7() -> JournalEntry {
    JournalEntry {
        sequence: 7,
        document_id: "main.tex".into(),
        revision: 7,
        utf8: R7.to_vec(),
        checksum: R7_HASH.into(),
    }
}
#[test]
fn contract_v1_first_slice() {
    let disk = vec![DiskIdentity {
        path: "main.tex".into(),
        content_sha256: "0".repeat(64),
    }];
    let service = RecoveryService::new(7);
    let recovered = service.recover(checkpoint(), &[r7()], &disk).unwrap();
    assert_eq!(
        recovered.documents.len(),
        1,
        "acknowledged r7 must survive interruption"
    );
    assert_eq!(recovered.documents[0].utf8, R7);
    assert_eq!(recovered.documents[0].revision, 7);
    assert!(recovered.documents[0].dirty);
    assert_eq!(recovered.last_sequence, 7);

    let mut torn = r7();
    torn.sequence = 8;
    torn.revision = 8;
    torn.utf8.pop();
    assert_eq!(
        RecoveryService::new(8).recover(checkpoint(), &[r7(), torn], &disk),
        Err(RecoveryError::Corrupt)
    );
    assert_eq!(disk[0].content_sha256, "0".repeat(64));
}

use recovery_service::{
    RecoveryInput, RecoveryScope, RecoverySource, SourceError, recover_from_source,
};

#[derive(Clone)]
struct MemorySource(Result<RecoveryInput, std::io::ErrorKind>);
impl RecoverySource for MemorySource {
    fn read(&self) -> Result<RecoveryInput, std::io::ErrorKind> {
        self.0.clone()
    }
}
fn scope(session: &str) -> RecoveryScope {
    RecoveryScope {
        project_id: "synthetic-project".into(),
        session_id: session.into(),
    }
}
fn source(session: &str) -> MemorySource {
    MemorySource(Ok(RecoveryInput {
        scope: scope(session),
        checkpoint: checkpoint(),
        journal: vec![r7()],
    }))
}
fn corrupt(checkpoint: Checkpoint, journal: &[JournalEntry], ack: u64) {
    assert_eq!(
        RecoveryService::new(ack).recover(checkpoint, journal, &[]),
        Err(RecoveryError::Corrupt)
    );
}

#[test]
fn review_focus_r1() {
    let cp = checkpoint();
    let mut tail = r7();
    tail.sequence = 8;
    tail.revision = 8;
    tail.utf8 = vec![0xf0, 0x9f];
    corrupt(cp.clone(), &[r7(), tail], 8);
    let complete = RecoveryService::new(7)
        .recover(cp.clone(), &[r7()], &[])
        .unwrap();
    assert_eq!(complete.documents[0].utf8, R7);
    assert_eq!(complete.last_sequence, 7);
    let mut unacknowledged = r7();
    unacknowledged.sequence = 8;
    unacknowledged.revision = 8;
    corrupt(cp, &[r7(), unacknowledged], 7);
}

#[test]
fn review_focus_r2() {
    // These are raw-byte identity observations, not logical text digests or save permission.
    let disk = vec![DiskIdentity {
        path: "main.tex".into(),
        content_sha256: "a".repeat(64),
    }];
    let changed = vec![DiskIdentity {
        path: "main.tex".into(),
        content_sha256: "b".repeat(64),
    }];
    let old = RecoveryService::new(7)
        .recover(checkpoint(), &[r7()], &disk)
        .unwrap();
    let current = RecoveryService::new(7)
        .recover(checkpoint(), &[r7()], &changed)
        .unwrap();
    assert_eq!(current, old);
    assert_eq!(current.documents[0].utf8, R7);
    assert!(current.documents[0].dirty);
    assert_eq!(changed[0].content_sha256, "b".repeat(64));
}

#[test]
fn review_focus_r3() {
    // Disk inventory is not the retained editor-buffer quota. Undo is absent from v1.
    let disk: Vec<_> = (0..100)
        .map(|n| DiskIdentity {
            path: format!("chapter{n}.tex"),
            content_sha256: "0".repeat(64),
        })
        .collect();
    let result = RecoveryService::new(7)
        .recover(checkpoint(), &[r7()], &disk)
        .unwrap();
    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0].utf8, R7);
    assert!(result.documents[0].dirty);
    assert_eq!(result.documents[0].revision, 7);
}

#[test]
fn review_focus_r4() {
    for error in [
        std::io::ErrorKind::PermissionDenied,
        std::io::ErrorKind::StorageFull,
    ] {
        assert_eq!(
            recover_from_source(&scope("s1"), 7, &MemorySource(Err(error)), &[]),
            Err(SourceError::Unavailable(error))
        );
    }
    // An acknowledgement beyond available data must never appear successful.
    assert_eq!(
        recover_from_source(&scope("s1"), 8, &source("s1"), &[]),
        Err(SourceError::Recovery(RecoveryError::Corrupt))
    );
}

#[test]
fn review_focus_r5() {
    let correct = recover_from_source(&scope("s1"), 7, &source("s1"), &[]).unwrap();
    assert_eq!(correct.documents[0].utf8, R7);
    assert_eq!(
        recover_from_source(&scope("s1"), 7, &source("s2"), &[]),
        Err(SourceError::WrongScope)
    );
    let mut other_project = scope("s1");
    other_project.project_id = "other-project".into();
    assert_eq!(
        recover_from_source(&other_project, 7, &source("s1"), &[]),
        Err(SourceError::WrongScope)
    );
}

#[test]
fn rejects_checksum_mismatch_even_with_well_formed_hex() {
    for checksum in ["0".repeat(64), R7_HASH.to_uppercase(), "bad".into()] {
        let mut entry = r7();
        entry.checksum = checksum;
        corrupt(checkpoint(), &[entry], 7);
    }
    let mut entry = r7();
    entry.utf8[0] = b'N';
    corrupt(checkpoint(), &[entry], 7);
}

#[test]
fn rejects_sequence_gaps_reordering_duplicate_and_missing_replay() {
    corrupt(checkpoint(), &[], 7);
    corrupt(checkpoint(), &[r7()], 8);
    corrupt(checkpoint(), &[r7(), r7()], 8);
    let mut gap = r7();
    gap.sequence = 8;
    corrupt(checkpoint(), &[gap.clone()], 8);
    corrupt(checkpoint(), &[gap, r7()], 8);
    corrupt(checkpoint(), &[r7()], 5);
    corrupt(checkpoint(), &[r7()], 6);
    let mut cp = checkpoint();
    cp.sequence = u64::MAX;
    let result = RecoveryService::new(u64::MAX)
        .recover(cp.clone(), &[], &[])
        .unwrap();
    assert_eq!(result.last_sequence, u64::MAX);
    corrupt(cp, &[r7()], u64::MAX);
}

#[test]
fn revisions_are_per_document_contiguous_and_lossless_u64() {
    for rev in [0, 6, 8] {
        let mut e = r7();
        e.revision = rev;
        corrupt(checkpoint(), &[e], 7);
    }
    let mut cp = checkpoint();
    cp.documents[0].revision = 9_007_199_254_740_993;
    let mut entry = r7();
    entry.revision = 9_007_199_254_740_994;
    let result = RecoveryService::new(7)
        .recover(cp.clone(), &[entry], &[])
        .unwrap();
    assert_eq!(result.documents[0].revision, 9_007_199_254_740_994);
    cp.documents[0].revision = u64::MAX;
    corrupt(cp, &[r7()], 7);
}

#[test]
fn multiple_documents_keep_their_content_revision_dirty_flag_and_order() {
    let mut cp = checkpoint();
    cp.documents.push(RecoveredDocument {
        document_id: "chapters/árvíz.tex".into(),
        revision: 40,
        utf8: b"clean\n".to_vec(),
        dirty: false,
    });
    let result = RecoveryService::new(7)
        .recover(cp.clone(), &[r7()], &[])
        .unwrap();
    assert_eq!(result.documents[0].utf8, R7);
    assert_eq!(result.documents[1], cp.documents[1]);
    let mut entry = r7();
    entry.document_id = "chapters/árvíz.tex".into();
    entry.sequence = 8;
    entry.revision = 41;
    let result = RecoveryService::new(8)
        .recover(cp, &[r7(), entry], &[])
        .unwrap();
    assert_eq!(result.documents[0].revision, 7);
    assert_eq!(result.documents[1].revision, 41);
    assert!(result.documents[1].dirty);
    assert_eq!(result.last_sequence, 8);
}

#[test]
fn rejects_duplicate_unknown_or_nonrelative_document_identity() {
    let mut cp = checkpoint();
    cp.documents.push(cp.documents[0].clone());
    corrupt(cp, &[], 6);
    for id in [
        "",
        "../main.tex",
        "/main.tex",
        "a//b.tex",
        "a/./b.tex",
        "C:/main.tex",
        "a\\b.tex",
    ] {
        let mut cp = checkpoint();
        cp.documents[0].document_id = id.into();
        corrupt(cp, &[], 6);
    }
    let mut e = r7();
    e.document_id = "unknown.tex".into();
    corrupt(checkpoint(), &[e], 7);
    let mut cp = checkpoint();
    cp.documents[0].revision = 0;
    corrupt(cp, &[], 6);
}

#[test]
fn validates_schema_and_each_quota_without_truncation() {
    let mut cp = checkpoint();
    cp.schema = 2;
    assert_eq!(
        RecoveryService::new(7).recover(cp, &[r7()], &[]),
        Err(RecoveryError::UnsupportedSchema)
    );
    let mut cp = checkpoint();
    cp.documents[0].utf8 = vec![b'x'; 1_048_577];
    assert_eq!(
        RecoveryService::new(6).recover(cp, &[], &[]),
        Err(RecoveryError::Quota)
    );
    let mut cp = checkpoint();
    cp.documents = (0..11)
        .map(|i| RecoveredDocument {
            document_id: format!("d{i}.tex"),
            revision: 1,
            utf8: vec![],
            dirty: true,
        })
        .collect();
    assert_eq!(
        RecoveryService::new(6).recover(cp, &[], &[]),
        Err(RecoveryError::Quota)
    );
    let entries = vec![r7(); 1025];
    assert_eq!(
        RecoveryService::new(7).recover(checkpoint(), &entries, &[]),
        Err(RecoveryError::Quota)
    );
    let mut e = r7();
    e.utf8 = vec![b'x'; 1_048_577];
    assert_eq!(
        RecoveryService::new(7).recover(checkpoint(), &[e], &[]),
        Err(RecoveryError::Quota)
    );
    let disk = (0..1025)
        .map(|i| DiskIdentity {
            path: format!("d{i}.tex"),
            content_sha256: "0".repeat(64),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        RecoveryService::new(7).recover(checkpoint(), &[r7()], &disk),
        Err(RecoveryError::Quota)
    );
}

#[test]
fn aggregate_input_quota_includes_superseded_journal_bytes() {
    let mut cp = checkpoint();
    cp.documents = (0..10)
        .map(|n| RecoveredDocument {
            document_id: format!("d{n}.tex"),
            revision: 1,
            utf8: vec![b'a'; 1_048_576],
            dirty: true,
        })
        .collect();
    let entries: Vec<_> = (7..14)
        .map(|sequence| JournalEntry {
            sequence,
            document_id: "d0.tex".into(),
            revision: sequence - 5,
            utf8: vec![b'a'; 1_048_576],
            checksum: "9bc1b2a288b26af7257a36277ae3816a7d4f16e89c1e7e77d0a5c48bad62b360".into(),
        })
        .collect();
    assert_eq!(
        RecoveryService::new(13).recover(cp, &entries, &[]),
        Err(RecoveryError::Quota)
    );
}

#[test]
fn validates_disk_identity_but_never_uses_it_to_clear_dirty_state() {
    let duplicate = vec![
        DiskIdentity {
            path: "main.tex".into(),
            content_sha256: R7_HASH.into()
        };
        2
    ];
    assert_eq!(
        RecoveryService::new(7).recover(checkpoint(), &[r7()], &duplicate),
        Err(RecoveryError::Corrupt)
    );
    for (path, hash) in [("../main.tex", R7_HASH), ("main.tex", "bad")] {
        let disk = [DiskIdentity {
            path: path.into(),
            content_sha256: hash.into(),
        }];
        assert_eq!(
            RecoveryService::new(7).recover(checkpoint(), &[r7()], &disk),
            Err(RecoveryError::Corrupt)
        );
    }
    let disk = [DiskIdentity {
        path: "main.tex".into(),
        content_sha256: R7_HASH.into(),
    }];
    let result = RecoveryService::new(7)
        .recover(checkpoint(), &[r7()], &disk)
        .unwrap();
    assert!(result.documents[0].dirty);
}

#[test]
fn logical_unicode_feff_is_preserved_and_raw_line_endings_are_rejected() {
    let mut cp = checkpoint();
    cp.documents[0].utf8 = "\u{feff}a\u{301}😀\n".as_bytes().to_vec();
    let result = RecoveryService::new(6)
        .recover(cp.clone(), &[], &[])
        .unwrap();
    assert_eq!(result.documents[0].utf8, "\u{feff}a\u{301}😀\n".as_bytes());
    for invalid in [
        b"raw\r\n".to_vec(),
        b"old\r".to_vec(),
        b"nul\0".to_vec(),
        vec![0xf0, 0x9f],
    ] {
        cp.documents[0].utf8 = invalid;
        corrupt(cp.clone(), &[], 6);
    }
    for (bytes, hash) in [
        (
            b"raw\r\n".as_slice(),
            "5fef8d5fc44dbb7be9fc5a16502135ef24b36fa6d7e55bbf11d1309b0cb53e05",
        ),
        (
            b"nul\0".as_slice(),
            "cd0798f9e2941f84b9f162a267d0b91de9b3314dbc28cd8185d3f82e09eca76d",
        ),
        (
            &[0xf0, 0x9f],
            "3c1c36c746e9b9106e77cfc2ede3374d38a883e23fbcd3b20023e326008c5434",
        ),
        (
            "\u{feff}a\u{301}😀\n".as_bytes(),
            "17deebf5def82084f4888c84514eb46c27253e07e22d33a795df756739ae658a",
        ),
    ] {
        let mut e = r7();
        e.utf8 = bytes.to_vec();
        e.checksum = hash.into();
        let result = RecoveryService::new(7).recover(checkpoint(), &[e], &[]);
        if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            assert_eq!(result.unwrap().documents[0].utf8, bytes);
        } else {
            assert_eq!(result, Err(RecoveryError::Corrupt));
        }
    }
}

#[test]
fn independent_sha256_vectors_cover_padding_blocks_and_file_ceiling() {
    // Frozen expectations produced with Python hashlib, not the service hash implementation.
    let vectors: &[(usize, &str)] = &[
        (
            0,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            1,
            "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
        ),
        (
            55,
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            56,
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            63,
            "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34",
        ),
        (
            64,
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
        (
            65,
            "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0",
        ),
        (
            127,
            "c57e9278af78fa3cab38667bef4ce29d783787a2f731d4e12200270f0c32320a",
        ),
        (
            128,
            "6836cf13bac400e9105071cd6af47084dfacad4e5e302c94bfed24e013afb73e",
        ),
        (
            1048576,
            "9bc1b2a288b26af7257a36277ae3816a7d4f16e89c1e7e77d0a5c48bad62b360",
        ),
    ];
    for (length, hash) in vectors {
        let mut e = r7();
        e.utf8 = vec![b'a'; *length];
        e.checksum = (*hash).into();
        let result = RecoveryService::new(7)
            .recover(checkpoint(), &[e], &[])
            .unwrap();
        assert_eq!(result.documents[0].utf8.len(), *length);
        assert_eq!(result.documents[0].revision, 7);
    }
}
