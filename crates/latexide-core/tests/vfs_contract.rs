#[path = "../src/services/vfs/mod.rs"]
mod vfs_service;

use latexide_contracts::{
    SnapshotLimits,
    vfs::{DirtyOverlay, VfsEntry, VfsError, VfsPort, VfsSnapshot},
};
use std::{cell::RefCell, rc::Rc};
use vfs_service::{
    DiskBaseline, DiskObservation, EntryAccess, Observation, ObservationProducer, OverlayConflict,
    OverlayObservation, VfsService,
};

// SHA-256 expectations are independent Python hashlib results for synthetic bytes.
const OLD: &str = "cba06b5736faf67e54b07b561eae94395e774c517a7d910a54369e1263ccfbd4";
const NEW: &str = "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437";
const DIRTY: &str = "d0c92c764c5647d43d7166efc3e7d350751de063c1388bd2e113fab4865529d3";

#[derive(Clone)]
struct SyntheticProducer(Rc<RefCell<Result<Observation, VfsError>>>);
impl SyntheticProducer {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Ok(Observation {
            complete: true,
            disk: vec![],
            overlays: vec![],
        }))))
    }
    fn set(&self, observation: Observation) {
        *self.0.borrow_mut() = Ok(observation);
    }
}
impl ObservationProducer for SyntheticProducer {
    fn observe(&self, _: &SnapshotLimits) -> Result<Observation, VfsError> {
        self.0.borrow().clone()
    }
}

fn limits() -> SnapshotLimits {
    SnapshotLimits {
        max_files: 128,
        max_file_bytes: 1024 * 1024,
        max_total_bytes: 8 * 1024 * 1024,
    }
}
fn empty() -> VfsSnapshot {
    VfsSnapshot {
        epoch: 0,
        entries: vec![],
    }
}
fn entry(path: &str, hash: &str, size: u64) -> VfsEntry {
    VfsEntry {
        path: path.into(),
        content_sha256: hash.into(),
        size,
    }
}
fn disk(path: &str, identity: &str, utf8: &[u8]) -> DiskObservation {
    DiskObservation {
        path: path.into(),
        identity: identity.into(),
        utf8: utf8.into(),
        mtime_ns: 42,
        access: EntryAccess::RegularWithinRoot,
    }
}
fn overlay(
    path: &str,
    revision: u64,
    utf8: &[u8],
    identity: &str,
    hash: &str,
) -> OverlayObservation {
    OverlayObservation {
        path: path.into(),
        revision,
        utf8: utf8.into(),
        base: Some(DiskBaseline {
            identity: identity.into(),
            content_sha256: hash.into(),
        }),
    }
}

// Catches metadata-only caching, disk precedence, and loss of runtime revision.
#[test]
fn contract_v1_first_slice() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    producer.set(Observation {
        complete: true,
        disk: vec![disk("chapter.tex", "chapter:1", b"old")],
        overlays: vec![],
    });
    let before = service
        .reconcile(&empty(), vec![entry("chapter.tex", OLD, 3)], vec![])
        .unwrap();
    producer.set(Observation {
        complete: true,
        disk: vec![disk("chapter.tex", "chapter:1", b"new")],
        overlays: vec![],
    });
    let changed = service
        .reconcile(&before, vec![entry("chapter.tex", NEW, 3)], vec![])
        .unwrap();
    assert_eq!(changed.entries, vec![entry("chapter.tex", NEW, 3)]);
    assert_eq!(changed.epoch, before.epoch + 1);
    let revision = 9_007_199_254_740_993;
    producer.set(Observation {
        complete: true,
        disk: vec![disk("chapter.tex", "chapter:1", b"new")],
        overlays: vec![overlay("chapter.tex", revision, b"dirty", "chapter:1", NEW)],
    });
    let overlaid = service
        .reconcile(
            &changed,
            vec![entry("chapter.tex", NEW, 3)],
            vec![DirtyOverlay {
                path: "chapter.tex".into(),
                revision,
                content_sha256: DIRTY.into(),
            }],
        )
        .unwrap();
    assert_eq!(overlaid.entries, vec![entry("chapter.tex", DIRTY, 5)]);
    let manifest = service.manifest().unwrap();
    assert_eq!(manifest.overlays[0].revision, revision);
    assert_eq!(manifest.overlays[0].conflict, None);
    assert_eq!(manifest.disk[0].content_sha256, NEW);

    // A second same-size/same-mtime observation changes disk underneath an
    // already retained dirty overlay, without advancing its runtime revision.
    producer.set(Observation {
        complete: true,
        disk: vec![disk("chapter.tex", "chapter:1", b"old")],
        overlays: vec![overlay("chapter.tex", revision, b"dirty", "chapter:1", NEW)],
    });
    let refreshed = service
        .reconcile(
            &overlaid,
            vec![entry("chapter.tex", OLD, 3)],
            vec![dirty_request("chapter.tex", revision)],
        )
        .unwrap();
    assert_eq!(refreshed.entries, overlaid.entries);
    assert_eq!(refreshed.epoch, overlaid.epoch + 1);
    let manifest = service.manifest().unwrap();
    assert_eq!(manifest.disk[0].content_sha256, OLD);
    assert_eq!(manifest.overlays[0].revision, revision);
    assert_eq!(
        manifest.overlays[0].conflict,
        Some(OverlayConflict::DiskChanged)
    );
}

fn observe(files: Vec<DiskObservation>, overlays: Vec<OverlayObservation>) -> Observation {
    Observation {
        complete: true,
        disk: files,
        overlays,
    }
}
fn dirty_request(path: &str, revision: u64) -> DirtyOverlay {
    DirtyOverlay {
        path: path.into(),
        revision,
        content_sha256: DIRTY.into(),
    }
}
fn start() -> (
    SyntheticProducer,
    VfsService<SyntheticProducer>,
    VfsSnapshot,
) {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:1", b"old")],
        vec![],
    ));
    let snapshot = service
        .reconcile(&empty(), vec![entry("chapter.tex", OLD, 3)], vec![])
        .unwrap();
    assert_eq!(snapshot.entries, vec![entry("chapter.tex", OLD, 3)]);
    (producer, service, snapshot)
}

// Catches merging a complete rescan as a partial event list (lost deletes/renames).
#[test]
fn review_focus_r1() {
    let (producer, service, previous) = start();
    producer.set(observe(
        vec![
            disk("renamed.tex", "chapter:1", b"old"),
            disk("added.tex", "added:1", b"new"),
        ],
        vec![],
    ));
    let restored = service
        .reconcile(
            &previous,
            vec![entry("renamed.tex", OLD, 3), entry("added.tex", NEW, 3)],
            vec![],
        )
        .unwrap();
    assert_eq!(
        restored.entries,
        vec![entry("added.tex", NEW, 3), entry("renamed.tex", OLD, 3)]
    );
    assert_eq!(restored.epoch, previous.epoch + 1);
    assert_eq!(service.manifest().unwrap().disk[1].identity, "chapter:1");
    producer.set(observe(vec![], vec![]));
    let deleted = service.reconcile(&restored, vec![], vec![]).unwrap();
    assert!(deleted.entries.is_empty());
    assert_eq!(deleted.epoch, restored.epoch + 1);
}

// Catches implicit lowercase/NFC folding and silent aliasing of one producer identity.
#[test]
fn review_focus_r2() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    let paths = ["A.tex", "a.tex", "cafe\u{301}.tex", "café.tex"];
    producer.set(observe(
        paths
            .iter()
            .enumerate()
            .map(|(i, p)| disk(p, &format!("file:{i}"), b"old"))
            .collect(),
        vec![],
    ));
    let result = service
        .reconcile(
            &empty(),
            paths.iter().rev().map(|p| entry(p, OLD, 3)).collect(),
            vec![],
        )
        .unwrap();
    assert_eq!(
        result
            .entries
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        paths
    );
    let before = service.manifest().unwrap();
    producer.set(observe(
        vec![
            disk("A.tex", "same:1", b"old"),
            disk("a.tex", "same:1", b"old"),
        ],
        vec![],
    ));
    assert_eq!(
        service.reconcile(
            &result,
            vec![entry("A.tex", OLD, 3), entry("a.tex", OLD, 3)],
            vec![]
        ),
        Err(VfsError::IdentityConflict)
    );
    assert_eq!(service.manifest().unwrap(), before);
}

// Catches lexical root escapes, platform-sensitive separators, and unapproved links.
#[test]
fn review_focus_r3() {
    let (producer, service, previous) = start();
    let before = service.manifest().unwrap();
    for path in [
        "",
        "/outside.tex",
        "../outside.tex",
        "a/../b.tex",
        "a/./b.tex",
        "a//b.tex",
        "a/",
        "./a.tex",
        "C:/a.tex",
        "a\\b.tex",
        "a:stream",
        "a\0b",
        "a\nb",
    ] {
        producer.set(observe(vec![disk(path, "file:1", b"old")], vec![]));
        assert_eq!(
            service.reconcile(&previous, vec![entry(path, OLD, 3)], vec![]),
            Err(VfsError::OutsideRoot),
            "{path:?}"
        );
        assert_eq!(service.manifest().unwrap(), before);
    }
    for access in [
        EntryAccess::Symlink,
        EntryAccess::OutsideRoot,
        EntryAccess::Unknown,
    ] {
        let mut file = disk("chapter.tex", "chapter:1", b"old");
        file.access = access;
        producer.set(observe(vec![file], vec![]));
        assert_eq!(
            service.reconcile(&previous, vec![entry("chapter.tex", OLD, 3)], vec![]),
            Err(VfsError::OutsideRoot)
        );
        assert_eq!(service.manifest().unwrap(), before);
    }
}

// Catches losing dirty data after disk deletion or applying recreated disk over it.
#[test]
fn review_focus_r4() {
    let (producer, service, previous) = start();
    let edit = overlay("chapter.tex", 7, b"dirty", "chapter:1", OLD);
    producer.set(observe(vec![], vec![edit.clone()]));
    let deleted = service
        .reconcile(&previous, vec![], vec![dirty_request("chapter.tex", 7)])
        .unwrap();
    assert_eq!(deleted.entries, vec![entry("chapter.tex", DIRTY, 5)]);
    assert_eq!(
        service.manifest().unwrap().overlays[0].conflict,
        Some(OverlayConflict::DiskDeleted)
    );
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:2", b"new")],
        vec![edit],
    ));
    let recreated = service
        .reconcile(
            &deleted,
            vec![entry("chapter.tex", NEW, 3)],
            vec![dirty_request("chapter.tex", 7)],
        )
        .unwrap();
    assert_eq!(recreated.entries, deleted.entries);
    assert_eq!(recreated.epoch, deleted.epoch + 1);
    let manifest = service.manifest().unwrap();
    assert_eq!(manifest.overlays[0].revision, 7);
    assert_eq!(
        manifest.overlays[0].conflict,
        Some(OverlayConflict::DiskReplaced)
    );
    assert_eq!(manifest.disk[0].identity, "chapter:2");
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:2", b"new")],
        vec![],
    ));
    let clean = service
        .reconcile(&recreated, vec![entry("chapter.tex", NEW, 3)], vec![])
        .unwrap();
    assert_eq!(clean.entries, vec![entry("chapter.tex", NEW, 3)]);
    assert!(service.manifest().unwrap().overlays.is_empty());
}

// Catches comparing names/hashes alone when replacement has identical bytes.
#[test]
fn review_focus_r5() {
    let (producer, service, previous) = start();
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:2", b"old")],
        vec![],
    ));
    let replaced = service
        .reconcile(&previous, vec![entry("chapter.tex", OLD, 3)], vec![])
        .unwrap();
    assert_eq!(replaced.entries, previous.entries);
    assert_eq!(replaced.epoch, previous.epoch + 1);
    assert_eq!(service.manifest().unwrap().disk[0].identity, "chapter:2");
}

// Catches unnecessary epochs from enumeration/mtime changes and duplicate exact names.
#[test]
fn unchanged_observation_is_idempotent_and_sorted() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    let a = disk("a.tex", "a:1", b"old");
    let mut b = disk("b.tex", "b:1", b"new");
    producer.set(observe(vec![b.clone(), a.clone()], vec![]));
    let before = service
        .reconcile(
            &empty(),
            vec![entry("b.tex", NEW, 3), entry("a.tex", OLD, 3)],
            vec![],
        )
        .unwrap();
    assert_eq!(
        before.entries,
        vec![entry("a.tex", OLD, 3), entry("b.tex", NEW, 3)]
    );
    b.mtime_ns = 100;
    producer.set(observe(vec![a.clone(), b], vec![]));
    assert_eq!(
        service
            .reconcile(&before, before.entries.clone(), vec![])
            .unwrap(),
        before
    );
    producer.set(observe(vec![a.clone(), a], vec![]));
    assert_eq!(
        service.reconcile(
            &before,
            vec![entry("a.tex", OLD, 3), entry("a.tex", OLD, 3)],
            vec![]
        ),
        Err(VfsError::IdentityConflict)
    );
}

// Catches treating format-valid client hashes, sizes, or revisions as producer evidence.
#[test]
fn request_claims_must_bind_to_observed_bytes_and_runtime_revision() {
    let (producer, service, previous) = start();
    let before = service.manifest().unwrap();
    for request in [
        vec![],
        vec![entry("chapter.tex", NEW, 3)],
        vec![entry("chapter.tex", OLD, 4)],
        vec![entry("chapter.tex", &"A".repeat(64), 3)],
        vec![entry("chapter.tex", "bad", 3)],
    ] {
        assert_eq!(
            service.reconcile(&previous, request, vec![]),
            Err(VfsError::IdentityConflict)
        );
        assert_eq!(service.manifest().unwrap(), before);
    }
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:1", b"old")],
        vec![overlay("chapter.tex", 8, b"dirty", "chapter:1", OLD)],
    ));
    assert_eq!(
        service.reconcile(
            &previous,
            vec![entry("chapter.tex", OLD, 3)],
            vec![dirty_request("chapter.tex", 7)]
        ),
        Err(VfsError::IdentityConflict)
    );
    assert_eq!(service.manifest().unwrap(), before);
}

// Catches partial scans erasing files and stale callers replacing current state.
#[test]
fn incomplete_failed_or_stale_observation_does_not_publish() {
    let (producer, service, previous) = start();
    let before = service.manifest().unwrap();
    producer.set(Observation {
        complete: false,
        disk: vec![],
        overlays: vec![],
    });
    assert_eq!(
        service.reconcile(&previous, vec![], vec![]),
        Err(VfsError::IdentityConflict)
    );
    for error in [
        VfsError::OutsideRoot,
        VfsError::IdentityConflict,
        VfsError::Quota,
    ] {
        *producer.0.borrow_mut() = Err(error.clone());
        assert_eq!(service.reconcile(&previous, vec![], vec![]), Err(error));
        assert_eq!(service.manifest().unwrap(), before);
    }
    producer.set(observe(vec![], vec![]));
    assert_eq!(
        service.reconcile(&empty(), vec![], vec![]),
        Err(VfsError::IdentityConflict)
    );
    let mut tampered = previous.clone();
    tampered.entries[0].content_sha256 = NEW.into();
    assert_eq!(
        service.reconcile(&tampered, vec![], vec![]),
        Err(VfsError::IdentityConflict)
    );
    assert_eq!(service.manifest().unwrap(), before);
}

// Catches stale/zero overlay revisions and revision-only changes hidden by same bytes.
#[test]
fn overlay_revisions_are_monotonic_and_part_of_the_epoch() {
    let (producer, service, previous) = start();
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:1", b"old")],
        vec![overlay("chapter.tex", 8, b"dirty", "chapter:1", OLD)],
    ));
    let edited = service
        .reconcile(
            &previous,
            vec![entry("chapter.tex", OLD, 3)],
            vec![dirty_request("chapter.tex", 8)],
        )
        .unwrap();
    let before = service.manifest().unwrap();
    for revision in [0, 7] {
        producer.set(observe(
            vec![disk("chapter.tex", "chapter:1", b"old")],
            vec![overlay("chapter.tex", revision, b"dirty", "chapter:1", OLD)],
        ));
        assert_eq!(
            service.reconcile(
                &edited,
                vec![entry("chapter.tex", OLD, 3)],
                vec![dirty_request("chapter.tex", revision)]
            ),
            Err(VfsError::IdentityConflict)
        );
        assert_eq!(service.manifest().unwrap(), before);
    }
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:1", b"old")],
        vec![overlay("chapter.tex", 9, b"dirty", "chapter:1", OLD)],
    ));
    let revised = service
        .reconcile(
            &edited,
            vec![entry("chapter.tex", OLD, 3)],
            vec![dirty_request("chapter.tex", 9)],
        )
        .unwrap();
    assert_eq!(revised.entries, edited.entries);
    assert_eq!(revised.epoch, edited.epoch + 1);
    assert_eq!(service.manifest().unwrap().overlays[0].revision, 9);
}

// Catches conflict loss for edits to the same disk identity and unsaved new files.
#[test]
fn disk_changes_and_new_file_collisions_are_explicit_conflicts() {
    let (producer, service, previous) = start();
    producer.set(observe(
        vec![disk("chapter.tex", "chapter:1", b"new")],
        vec![overlay("chapter.tex", 7, b"dirty", "chapter:1", OLD)],
    ));
    let changed = service
        .reconcile(
            &previous,
            vec![entry("chapter.tex", NEW, 3)],
            vec![dirty_request("chapter.tex", 7)],
        )
        .unwrap();
    assert_eq!(changed.entries, vec![entry("chapter.tex", DIRTY, 5)]);
    assert_eq!(
        service.manifest().unwrap().overlays[0].conflict,
        Some(OverlayConflict::DiskChanged)
    );
    let new_file = OverlayObservation {
        path: "unsaved.tex".into(),
        revision: 1,
        utf8: b"dirty".to_vec(),
        base: None,
    };
    producer.set(observe(vec![], vec![new_file.clone()]));
    let unsaved = service
        .reconcile(&changed, vec![], vec![dirty_request("unsaved.tex", 1)])
        .unwrap();
    assert_eq!(service.manifest().unwrap().overlays[0].conflict, None);
    producer.set(observe(
        vec![disk("unsaved.tex", "new:1", b"old")],
        vec![new_file],
    ));
    service
        .reconcile(
            &unsaved,
            vec![entry("unsaved.tex", OLD, 3)],
            vec![dirty_request("unsaved.tex", 1)],
        )
        .unwrap();
    assert_eq!(
        service.manifest().unwrap().overlays[0].conflict,
        Some(OverlayConflict::DiskCreated)
    );
}

// Catches malformed producer data being published despite a well-shaped request.
#[test]
fn invalid_producer_text_identity_or_overlay_does_not_publish() {
    let (producer, service, previous) = start();
    let before = service.manifest().unwrap();
    for utf8 in [b"a\r\nb".to_vec(), b"a\0b".to_vec(), vec![0xff]] {
        producer.set(observe(
            vec![disk("chapter.tex", "chapter:1", &utf8)],
            vec![],
        ));
        assert_eq!(
            service.reconcile(&previous, vec![entry("chapter.tex", OLD, 3)], vec![]),
            Err(VfsError::IdentityConflict)
        );
    }
    for identity in ["", "identity\0"] {
        producer.set(observe(vec![disk("chapter.tex", identity, b"old")], vec![]));
        assert_eq!(
            service.reconcile(&previous, vec![entry("chapter.tex", OLD, 3)], vec![]),
            Err(VfsError::IdentityConflict)
        );
    }
    let edit = overlay("chapter.tex", 1, b"dirty", "chapter:1", OLD);
    producer.set(observe(vec![], vec![edit.clone(), edit]));
    assert_eq!(
        service.reconcile(
            &previous,
            vec![],
            vec![
                dirty_request("chapter.tex", 1),
                dirty_request("chapter.tex", 1)
            ]
        ),
        Err(VfsError::IdentityConflict)
    );
    assert_eq!(service.manifest().unwrap(), before);
}

// Catches accepting pre-normalized hashes instead of hashing the exact logical bytes.
#[test]
fn logical_feff_unicode_and_empty_content_are_hashed_exactly() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    let text = "\u{feff}A😀e\u{301}\n".as_bytes();
    let hash = "2ad249c27f300bb9538f6431a29e1214a7316b61d21662a09c492c7fdd4932c8";
    let empty_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    producer.set(observe(
        vec![
            disk("feff.tex", "feff:1", text),
            disk("empty.tex", "empty:1", b""),
        ],
        vec![],
    ));
    let result = service
        .reconcile(
            &empty(),
            vec![
                entry("feff.tex", hash, 12),
                entry("empty.tex", empty_hash, 0),
            ],
            vec![],
        )
        .unwrap();
    assert_eq!(
        result.entries,
        vec![
            entry("empty.tex", empty_hash, 0),
            entry("feff.tex", hash, 12)
        ]
    );
}

// Catches SHA padding/block errors and failure to enforce the 1 MiB file ceiling.
#[test]
fn hash_padding_boundaries_and_file_quota() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(producer.clone(), limits());
    let vectors = [
        (
            55,
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            56,
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
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
            1024 * 1024,
            "9bc1b2a288b26af7257a36277ae3816a7d4f16e89c1e7e77d0a5c48bad62b360",
        ),
    ];
    let mut previous = empty();
    for (len, hash) in vectors {
        producer.set(observe(
            vec![disk("a.tex", "a:1", &vec![b'a'; len])],
            vec![],
        ));
        previous = service
            .reconcile(&previous, vec![entry("a.tex", hash, len as u64)], vec![])
            .unwrap();
        assert_eq!(previous.entries[0], entry("a.tex", hash, len as u64));
    }
    let before = service.manifest().unwrap();
    producer.set(observe(
        vec![disk("a.tex", "a:1", &vec![b'a'; 1024 * 1024 + 1])],
        vec![],
    ));
    assert_eq!(
        service.reconcile(&previous, vec![entry("a.tex", OLD, 3)], vec![]),
        Err(VfsError::Quota)
    );
    assert_eq!(service.manifest().unwrap(), before);
}

// Catches quotas applied only to the disk list, silently truncating the visible union.
#[test]
fn count_total_and_overlay_quotas_reject_atomically() {
    let producer = SyntheticProducer::new();
    let service = VfsService::new(
        producer.clone(),
        SnapshotLimits {
            max_files: 2,
            max_file_bytes: 5,
            max_total_bytes: 6,
        },
    );
    producer.set(observe(
        vec![disk("a.tex", "a:1", b"old"), disk("b.tex", "b:1", b"new")],
        vec![],
    ));
    let previous = service
        .reconcile(
            &empty(),
            vec![entry("a.tex", OLD, 3), entry("b.tex", NEW, 3)],
            vec![],
        )
        .unwrap();
    let before = service.manifest().unwrap();
    producer.set(observe(
        vec![
            disk("a.tex", "a:1", b"old"),
            disk("b.tex", "b:1", b"new"),
            disk("c.tex", "c:1", b"old"),
        ],
        vec![],
    ));
    assert_eq!(
        service.reconcile(&previous, vec![], vec![]),
        Err(VfsError::Quota)
    );
    let edit = OverlayObservation {
        path: "c.tex".into(),
        revision: 1,
        utf8: b"dirty".to_vec(),
        base: None,
    };
    producer.set(observe(
        vec![disk("a.tex", "a:1", b"old"), disk("b.tex", "b:1", b"new")],
        vec![edit.clone()],
    ));
    assert_eq!(
        service.reconcile(
            &previous,
            previous.entries.clone(),
            vec![dirty_request("c.tex", 1)]
        ),
        Err(VfsError::Quota)
    );
    producer.set(observe(
        vec![disk("a.tex", "a:1", b"old"), disk("b.tex", "b:1", b"new")],
        vec![OverlayObservation {
            path: "a.tex".into(),
            ..edit
        }],
    ));
    assert_eq!(
        service.reconcile(
            &previous,
            previous.entries.clone(),
            vec![dirty_request("a.tex", 1)]
        ),
        Err(VfsError::Quota)
    );
    assert_eq!(service.manifest().unwrap(), before);
}
