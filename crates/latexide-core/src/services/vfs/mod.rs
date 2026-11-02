//! Bounded reconciliation of explicit, complete observations. No filesystem walk,
//! watcher, document mutation, serialization, or save authority lives here.
//!
//! The host supplies an ObservationProducer, never one decoded from a client.
//! SHA-256 is computed from its canonical logical UTF-8 bytes on every call.
//! An identity/confinement claim is only as trustworthy as that producer: these
//! types do not capture OS handles or prove filesystem identity/confinement.
//! Frozen v1 omits identity, overlay revision/size, and conflict metadata in its
//! response. Consumers needing those must obtain the atomic Manifest sidecar.

mod digest;

use latexide_contracts::{
    SnapshotLimits,
    common::{MAX_FILE_BYTES, is_sha256},
    vfs::{DirtyOverlay, VfsEntry, VfsError, VfsPort, VfsSnapshot},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryAccess {
    RegularWithinRoot,
    Symlink,
    OutsideRoot,
    Unknown,
}
/// A trusted producer's canonical bytes and opaque, root-scoped file identity.
/// Physical BOM removal and CRLF decoding belong to that producer/runtime.
#[derive(Debug, Clone)]
pub struct DiskObservation {
    pub path: String,
    pub identity: String,
    pub utf8: Vec<u8>,
    /// Advisory only; never used as a content or identity cache key.
    pub mtime_ns: u64,
    pub access: EntryAccess,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskBaseline {
    pub identity: String,
    pub content_sha256: String,
}
#[derive(Debug, Clone)]
pub struct OverlayObservation {
    pub path: String,
    pub revision: u64,
    pub utf8: Vec<u8>,
    /// Last disk identity/digest acknowledged by the authoritative runtime.
    /// None means an unsaved new file, not an unknown baseline for an old file.
    pub base: Option<DiskBaseline>,
}
#[derive(Debug, Clone)]
pub struct Observation {
    pub complete: bool,
    pub disk: Vec<DiskObservation>,
    pub overlays: Vec<OverlayObservation>,
}
pub trait ObservationProducer {
    /// Return a complete immutable observation or an error. The producer must
    /// enforce limits during capture, before allocating/reading large inputs.
    /// This adapter checks them again, but cannot bound an external producer's
    /// allocations, latency, completeness claim, or OS access decisions.
    fn observe(&self, limits: &SnapshotLimits) -> Result<Observation, VfsError>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayConflict {
    DiskDeleted,
    DiskCreated,
    DiskChanged,
    DiskReplaced,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskRecord {
    pub path: String,
    pub identity: String,
    pub content_sha256: String,
    pub size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRecord {
    pub path: String,
    pub revision: u64,
    pub content_sha256: String,
    pub size: u64,
    pub base: Option<DiskBaseline>,
    pub conflict: Option<OverlayConflict>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub snapshot: VfsSnapshot,
    pub disk: Vec<DiskRecord>,
    pub overlays: Vec<OverlayRecord>,
}
pub struct VfsService<P> {
    producer: P,
    limits: SnapshotLimits,
    state: Mutex<Manifest>,
}
impl<P: ObservationProducer> VfsService<P> {
    /// Starts an empty, session-local manifest. This is a derived read model,
    /// not another editable document store. There is no unauthenticated restore.
    pub fn new(producer: P, limits: SnapshotLimits) -> Self {
        Self {
            producer,
            limits,
            state: Mutex::new(Manifest {
                snapshot: VfsSnapshot {
                    epoch: 0,
                    entries: vec![],
                },
                disk: vec![],
                overlays: vec![],
            }),
        }
    }
    pub fn manifest(&self) -> Result<Manifest, VfsError> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| VfsError::IdentityConflict)
    }
}
impl<P: ObservationProducer> VfsPort for VfsService<P> {
    fn reconcile(
        &self,
        previous: &VfsSnapshot,
        mut disk: Vec<VfsEntry>,
        mut overlays: Vec<DirtyOverlay>,
    ) -> Result<VfsSnapshot, VfsError> {
        let mut state = self.state.lock().map_err(|_| VfsError::IdentityConflict)?;
        if previous != &state.snapshot {
            return Err(VfsError::IdentityConflict);
        }
        count(disk.len(), self.limits.max_files)?;
        count(
            overlays.len(),
            self.limits.max_files.min(latexide_core::MAX_OPEN_DOCUMENTS),
        )?;
        for entry in &disk {
            path(&entry.path)?;
            if !is_sha256(&entry.content_sha256) {
                return Err(VfsError::IdentityConflict);
            }
            if entry.size > self.limits.max_file_bytes.min(MAX_FILE_BYTES) as u64 {
                return Err(VfsError::Quota);
            }
        }
        for overlay in &overlays {
            path(&overlay.path)?;
            if overlay.revision == 0 || !is_sha256(&overlay.content_sha256) {
                return Err(VfsError::IdentityConflict);
            }
        }
        // The supplied DTOs bind the call to a fresh producer observation. A
        // plausible digest alone never authenticates bytes or file identity.
        let observed = self.producer.observe(&self.limits)?;
        let mut next = prepare(observed, &state, &self.limits)?;
        disk.sort_by(|a, b| a.path.cmp(&b.path));
        overlays.sort_by(|a, b| a.path.cmp(&b.path));
        let expected_disk: Vec<_> = next
            .disk
            .iter()
            .map(|file| VfsEntry {
                path: file.path.clone(),
                content_sha256: file.content_sha256.clone(),
                size: file.size,
            })
            .collect();
        let expected_overlays: Vec<_> = next
            .overlays
            .iter()
            .map(|file| DirtyOverlay {
                path: file.path.clone(),
                revision: file.revision,
                content_sha256: file.content_sha256.clone(),
            })
            .collect();
        if disk != expected_disk || overlays != expected_overlays {
            return Err(VfsError::IdentityConflict);
        }
        let changed = next.disk != state.disk
            || next.overlays != state.overlays
            || next.snapshot.entries != state.snapshot.entries;
        next.snapshot.epoch = if changed {
            state.snapshot.epoch.checked_add(1).ok_or(VfsError::Quota)?
        } else {
            state.snapshot.epoch
        };
        let result = next.snapshot.clone();
        *state = next; // The only publication point; every error leaves state intact.
        Ok(result)
    }
}

fn prepare(
    observation: Observation,
    previous: &Manifest,
    limits: &SnapshotLimits,
) -> Result<Manifest, VfsError> {
    if !observation.complete {
        return Err(VfsError::IdentityConflict);
    }
    count(observation.disk.len(), limits.max_files)?;
    count(
        observation.overlays.len(),
        limits.max_files.min(latexide_core::MAX_OPEN_DOCUMENTS),
    )?;
    let mut disk = BTreeMap::new();
    let mut identities = BTreeSet::new();
    let mut disk_bytes = 0;
    for file in observation.disk {
        path(&file.path)?;
        if file.access != EntryAccess::RegularWithinRoot {
            return Err(VfsError::OutsideRoot);
        }
        identity(&file.identity)?;
        let size = content_size(&file.utf8, limits)?;
        add_bytes(&mut disk_bytes, size, limits)?;
        if !identities.insert(file.identity.clone()) {
            return Err(VfsError::IdentityConflict);
        }
        let record = DiskRecord {
            path: file.path.clone(),
            identity: file.identity,
            content_sha256: digest::sha256(&file.utf8),
            size: size as u64,
        };
        if disk.insert(file.path, record).is_some() {
            return Err(VfsError::IdentityConflict);
        }
    }
    let old_overlays: BTreeMap<_, _> = previous
        .overlays
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut overlays = BTreeMap::new();
    let mut overlay_bytes = 0;
    for file in observation.overlays {
        path(&file.path)?;
        if file.revision == 0 {
            return Err(VfsError::IdentityConflict);
        }
        if let Some(base) = &file.base {
            identity(&base.identity)?;
            if !is_sha256(&base.content_sha256) {
                return Err(VfsError::IdentityConflict);
            }
        }
        let size = content_size(&file.utf8, limits)?;
        add_bytes(&mut overlay_bytes, size, limits)?;
        let hash = digest::sha256(&file.utf8);
        if let Some(old) = old_overlays.get(file.path.as_str()) {
            if file.revision < old.revision
                || (file.revision == old.revision
                    && (hash != old.content_sha256
                        || size as u64 != old.size
                        || file.base != old.base))
            {
                return Err(VfsError::IdentityConflict);
            }
        }
        let conflict = match (&file.base, disk.get(&file.path)) {
            (None, None) => None,
            (None, Some(_)) => Some(OverlayConflict::DiskCreated),
            (Some(_), None) => Some(OverlayConflict::DiskDeleted),
            (Some(base), Some(current)) if base.identity != current.identity => {
                Some(OverlayConflict::DiskReplaced)
            }
            (Some(base), Some(current)) if base.content_sha256 != current.content_sha256 => {
                Some(OverlayConflict::DiskChanged)
            }
            (Some(_), Some(_)) => None,
        };
        let record = OverlayRecord {
            path: file.path.clone(),
            revision: file.revision,
            content_sha256: hash,
            size: size as u64,
            base: file.base,
            conflict,
        };
        if overlays.insert(file.path, record).is_some() {
            return Err(VfsError::IdentityConflict);
        }
    }
    let mut entries: BTreeMap<_, _> = disk
        .values()
        .map(|file| {
            (
                file.path.clone(),
                VfsEntry {
                    path: file.path.clone(),
                    content_sha256: file.content_sha256.clone(),
                    size: file.size,
                },
            )
        })
        .collect();
    for file in overlays.values() {
        entries.insert(
            file.path.clone(),
            VfsEntry {
                path: file.path.clone(),
                content_sha256: file.content_sha256.clone(),
                size: file.size,
            },
        );
    }
    count(entries.len(), limits.max_files)?;
    let mut visible_bytes = 0;
    for file in entries.values() {
        add_bytes(&mut visible_bytes, file.size as usize, limits)?;
    }
    Ok(Manifest {
        snapshot: VfsSnapshot {
            epoch: 0,
            entries: entries.into_values().collect(),
        },
        disk: disk.into_values().collect(),
        overlays: overlays.into_values().collect(),
    })
}

fn count(actual: usize, maximum: usize) -> Result<(), VfsError> {
    if actual > maximum {
        Err(VfsError::Quota)
    } else {
        Ok(())
    }
}
fn add_bytes(total: &mut usize, size: usize, limits: &SnapshotLimits) -> Result<(), VfsError> {
    *total = total.checked_add(size).ok_or(VfsError::Quota)?;
    count(*total, limits.max_total_bytes)
}
fn content_size(bytes: &[u8], limits: &SnapshotLimits) -> Result<usize, VfsError> {
    count(bytes.len(), limits.max_file_bytes.min(MAX_FILE_BYTES))?;
    let text = std::str::from_utf8(bytes).map_err(|_| VfsError::IdentityConflict)?;
    if text.contains(['\r', '\0']) {
        return Err(VfsError::IdentityConflict);
    }
    Ok(bytes.len())
}
fn identity(value: &str) -> Result<(), VfsError> {
    count(value.len(), 256)?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(VfsError::IdentityConflict);
    }
    Ok(())
}
fn path(value: &str) -> Result<(), VfsError> {
    count(value.len(), 4096)?;
    if value.is_empty()
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(VfsError::OutsideRoot);
    }
    // Exact UTF-8 identity: no lowercase, NFC, percent decoding or OS aliasing.
    Ok(())
}
