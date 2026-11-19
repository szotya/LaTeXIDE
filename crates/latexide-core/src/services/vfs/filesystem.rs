//! First synchronous filesystem producer for controlled local project roots.
//! Raw fingerprints remain separate from canonical logical UTF-8. This is not
//! an OS watcher, handle-rooted hostile-race confinement, or a save authority.
//! A complete receipt means bounded enumeration and individually checked reads.
//! Reads are sequential per-file observations, not one atomic workspace state;
//! an earlier file may change after its read without changing the root identity.

use super::{DiskObservation, EntryAccess, Observation, ObservationProducer, OverlayObservation};
use latexide_contracts::{SnapshotLimits, common::MAX_FILE_BYTES, vfs::VfsError};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const MAX_ENTRIES: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_RAW_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct TraversalLimits {
    pub max_entries: usize,
    pub max_depth: usize,
    pub max_raw_bytes: usize,
}
impl Default for TraversalLimits {
    fn default() -> Self {
        Self {
            max_entries: MAX_ENTRIES,
            max_depth: MAX_DEPTH,
            max_raw_bytes: MAX_RAW_BYTES,
        }
    }
}

/// Must return the authoritative host's bounded immutable dirty snapshot.
/// This producer neither synthesizes revisions nor mutates or saves overlays.
pub trait HostOverlays {
    fn snapshot(&self, limits: &SnapshotLimits) -> Result<Vec<OverlayObservation>, VfsError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFileRecord {
    pub path: String,
    pub identity: String,
    pub raw_size: u64,
    pub raw_sha256: String,
    pub logical_size: u64,
    pub logical_sha256: String,
    pub physical_bom: bool,
    pub crlf: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureFailure {
    pub error: VfsError,
    pub operation: &'static str,
    pub path: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureReceipt {
    pub attempt: u64,
    /// All entries were observed within budget; this is not cross-file atomicity.
    pub complete: bool,
    pub entries_seen: usize,
    pub files_read: usize,
    pub raw_bytes_read: usize,
    /// Always empty on failure: incomplete raw records are not a snapshot.
    pub raw_files: Vec<RawFileRecord>,
    pub failure: Option<CaptureFailure>,
}
impl CaptureReceipt {
    fn empty(attempt: u64) -> Self {
        Self {
            attempt,
            complete: false,
            entries_seen: 0,
            files_read: 0,
            raw_bytes_read: 0,
            raw_files: vec![],
            failure: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    uid: u32,
}

#[derive(Clone)]
pub struct BoundedFilesystemProducer<H> {
    root: PathBuf,
    root_identity: RootIdentity,
    overlays: H,
    traversal: TraversalLimits,
    last: Arc<Mutex<CaptureReceipt>>,
}
impl<H: HostOverlays> BoundedFilesystemProducer<H> {
    /// A canonical, non-symlink directory is supplied by the host. No live user
    /// project is discovered, and no authority token is inferred from this path.
    pub fn new(root: PathBuf, overlays: H, traversal: TraversalLimits) -> Result<Self, VfsError> {
        let root_identity = platform::root_identity(&root)?;
        Ok(Self {
            root,
            root_identity,
            overlays,
            traversal: TraversalLimits {
                max_entries: traversal.max_entries.min(MAX_ENTRIES),
                max_depth: traversal.max_depth.min(MAX_DEPTH),
                max_raw_bytes: traversal.max_raw_bytes.min(MAX_RAW_BYTES),
            },
            last: Arc::new(Mutex::new(CaptureReceipt::empty(0))),
        })
    }
    pub fn last_capture(&self) -> Result<CaptureReceipt, VfsError> {
        self.last
            .lock()
            .map(|last| last.clone())
            .map_err(|_| VfsError::IdentityConflict)
    }
}
impl<H: HostOverlays> ObservationProducer for BoundedFilesystemProducer<H> {
    fn observe(&self, limits: &SnapshotLimits) -> Result<Observation, VfsError> {
        let mut receipt = self.last.lock().map_err(|_| VfsError::IdentityConflict)?;
        let attempt = receipt.attempt.checked_add(1).ok_or(VfsError::Quota)?;
        *receipt = CaptureReceipt::empty(attempt);
        let result = (|| {
            let overlays = self
                .overlays
                .snapshot(limits)
                .map_err(|error| failure(error, "host-overlays", None))?;
            if overlays.len() > limits.max_files.min(latexide_core::MAX_OPEN_DOCUMENTS) {
                return Err(failure(VfsError::Quota, "overlay-count", None));
            }
            let mut bytes = 0usize;
            for overlay in &overlays {
                super::path(&overlay.path)
                    .map_err(|error| failure(error, "overlay-path", Some(&overlay.path)))?;
                let size = super::content_size(&overlay.utf8, limits)
                    .map_err(|error| failure(error, "overlay-bytes", Some(&overlay.path)))?;
                bytes = bytes
                    .checked_add(size)
                    .ok_or_else(|| failure(VfsError::Quota, "overlay-total", None))?;
                if bytes > limits.max_total_bytes {
                    return Err(failure(VfsError::Quota, "overlay-total", None));
                }
            }
            let (disk, raw_files) = platform::capture(
                &self.root,
                self.root_identity,
                limits,
                self.traversal,
                &mut receipt,
            )?;
            receipt.raw_files = raw_files;
            receipt.complete = true;
            Ok(Observation {
                complete: true,
                disk,
                overlays,
            })
        })();
        match result {
            Ok(observation) => Ok(observation),
            Err(error) => {
                receipt.complete = false;
                receipt.raw_files.clear();
                receipt.failure = Some(error.clone());
                Err(error.error)
            }
        }
    }
}

fn failure(error: VfsError, operation: &'static str, path: Option<&str>) -> CaptureFailure {
    CaptureFailure {
        error,
        operation,
        path: path.map(str::to_owned),
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::{
        fs::{self, Metadata, OpenOptions},
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };

    // Named installed SDK sys/fcntl.h: O_NONBLOCK, O_NOFOLLOW, O_NOFOLLOW_ANY.
    // Unsupported OS flag behavior fails capture; there is no weaker retry.
    const OPEN_FLAGS: i32 = 0x00000004 | 0x20000000;

    fn read_raw(reader: impl Read, raw: &mut Vec<u8>, cap: usize, remaining: usize) -> std::io::Result<usize> {
        // One probe byte detects growth without reading a whole per-file cap
        // when the aggregate budget is almost exhausted.
        reader.take(cap.min(remaining) as u64 + 1).read_to_end(raw)
    }

    #[test]
    fn raw_reader_stops_at_remaining_budget_plus_one_probe_byte() {
        use std::io::Cursor;
        for (available, cap, remaining, expected) in [
            (8192, MAX_FILE_BYTES, 4096, 4097), // file grew after metadata admission
            (4096, MAX_FILE_BYTES, 4096, 4096), // exact aggregate fit
            (8192, 2, 4096, 3),                 // per-file cap still applies
            (8192, MAX_FILE_BYTES, 0, 1),      // only the overflow probe remains
        ] {
            let mut reader = Cursor::new(vec![b'x'; available]);
            let mut raw = Vec::new();
            assert_eq!(read_raw(&mut reader, &mut raw, cap, remaining).unwrap(), expected);
            assert_eq!(raw.len(), expected);
            assert_eq!(reader.position(), expected as u64);
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Stamp {
        device: u64,
        inode: u64,
        uid: u32,
        mode: u32,
        links: u64,
        size: u64,
        mtime: i64,
        mtime_nsec: i64,
        ctime: i64,
        ctime_nsec: i64,
    }
    fn stamp(meta: &Metadata) -> Stamp {
        Stamp {
            device: meta.dev(),
            inode: meta.ino(),
            uid: meta.uid(),
            mode: meta.mode(),
            links: meta.nlink(),
            size: meta.len(),
            mtime: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
            ctime: meta.ctime(),
            ctime_nsec: meta.ctime_nsec(),
        }
    }
    fn identity(meta: &Metadata) -> RootIdentity {
        RootIdentity {
            device: meta.dev(),
            inode: meta.ino(),
            uid: meta.uid(),
        }
    }
    pub(super) fn root_identity(root: &Path) -> Result<RootIdentity, VfsError> {
        if !root.is_absolute() {
            return Err(VfsError::OutsideRoot);
        }
        let meta = fs::symlink_metadata(root).map_err(|_| VfsError::IdentityConflict)?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || fs::canonicalize(root).map_err(|_| VfsError::OutsideRoot)? != root
        {
            return Err(VfsError::OutsideRoot);
        }
        Ok(identity(&meta))
    }
    pub(super) fn capture(
        root: &Path,
        anchor: RootIdentity,
        limits: &SnapshotLimits,
        traversal: TraversalLimits,
        receipt: &mut CaptureReceipt,
    ) -> Result<(Vec<DiskObservation>, Vec<RawFileRecord>), CaptureFailure> {
        if root_identity(root).map_err(|error| failure(error, "project-root", None))? != anchor {
            return Err(failure(
                VfsError::IdentityConflict,
                "project-root-changed",
                None,
            ));
        }
        let mut scan = Scan {
            root,
            anchor,
            limits,
            traversal,
            receipt,
            disk: vec![],
            raw: vec![],
        };
        scan.directory(root, "", 0)?;
        if root_identity(root).map_err(|error| failure(error, "project-root", None))? != anchor {
            return Err(failure(
                VfsError::IdentityConflict,
                "project-root-changed",
                None,
            ));
        }
        scan.disk.sort_by(|a, b| a.path.cmp(&b.path));
        scan.raw.sort_by(|a, b| a.path.cmp(&b.path));
        Ok((scan.disk, scan.raw))
    }
    struct Scan<'a> {
        root: &'a Path,
        anchor: RootIdentity,
        limits: &'a SnapshotLimits,
        traversal: TraversalLimits,
        receipt: &'a mut CaptureReceipt,
        disk: Vec<DiskObservation>,
        raw: Vec<RawFileRecord>,
    }
    impl Scan<'_> {
        fn directory(
            &mut self,
            directory: &Path,
            relative: &str,
            depth: usize,
        ) -> Result<(), CaptureFailure> {
            let before = fs::symlink_metadata(directory).map_err(|_| {
                failure(VfsError::IdentityConflict, "directory-stat", Some(relative))
            })?;
            if !before.is_dir()
                || before.file_type().is_symlink()
                || before.dev() != self.anchor.device
            {
                return Err(failure(
                    VfsError::OutsideRoot,
                    "directory-kind",
                    Some(relative),
                ));
            }
            let canonical = fs::canonicalize(directory).map_err(|_| {
                failure(VfsError::IdentityConflict, "directory-path", Some(relative))
            })?;
            if canonical != directory || !canonical.starts_with(self.root) {
                return Err(failure(
                    VfsError::OutsideRoot,
                    "directory-path",
                    Some(relative),
                ));
            }
            let entries = fs::read_dir(directory).map_err(|_| {
                failure(VfsError::IdentityConflict, "directory-read", Some(relative))
            })?;
            let mut names = Vec::new();
            for item in entries {
                let item = item.map_err(|_| {
                    failure(
                        VfsError::IdentityConflict,
                        "directory-entry",
                        Some(relative),
                    )
                })?;
                self.receipt.entries_seen = self
                    .receipt
                    .entries_seen
                    .checked_add(1)
                    .ok_or_else(|| failure(VfsError::Quota, "entry-count", None))?;
                if self.receipt.entries_seen > self.traversal.max_entries {
                    return Err(failure(VfsError::Quota, "entry-count", Some(relative)));
                }
                let name = item
                    .file_name()
                    .into_string()
                    .map_err(|_| failure(VfsError::OutsideRoot, "filename-utf8", Some(relative)))?;
                let rel = if relative.is_empty() {
                    name
                } else {
                    format!("{relative}/{name}")
                };
                super::super::path(&rel)
                    .map_err(|error| failure(error, "relative-path", Some(&rel)))?;
                names.push((rel, item.path()));
            }
            names.sort_by(|a, b| a.0.cmp(&b.0));
            for (rel, path) in names {
                let meta = fs::symlink_metadata(&path)
                    .map_err(|_| failure(VfsError::IdentityConflict, "entry-stat", Some(&rel)))?;
                if meta.file_type().is_symlink() {
                    return Err(failure(VfsError::OutsideRoot, "symlink", Some(&rel)));
                }
                if meta.is_dir() {
                    if depth >= self.traversal.max_depth {
                        return Err(failure(VfsError::Quota, "directory-depth", Some(&rel)));
                    }
                    self.directory(&path, &rel, depth + 1)?;
                } else if meta.is_file() {
                    self.file(&path, &rel, &meta)?;
                } else {
                    return Err(failure(VfsError::OutsideRoot, "entry-kind", Some(&rel)));
                }
            }
            let after = fs::symlink_metadata(directory).map_err(|_| {
                failure(
                    VfsError::IdentityConflict,
                    "directory-readback",
                    Some(relative),
                )
            })?;
            if stamp(&before) != stamp(&after) {
                return Err(failure(
                    VfsError::IdentityConflict,
                    "directory-changed",
                    Some(relative),
                ));
            }
            Ok(())
        }
        fn file(
            &mut self,
            path: &Path,
            rel: &str,
            before: &Metadata,
        ) -> Result<(), CaptureFailure> {
            if self.disk.len() >= self.limits.max_files {
                return Err(failure(VfsError::Quota, "file-count", Some(rel)));
            }
            if before.dev() != self.anchor.device {
                return Err(failure(VfsError::OutsideRoot, "file-device", Some(rel)));
            }
            let cap = self.limits.max_file_bytes.min(MAX_FILE_BYTES);
            if before.len() > cap as u64 {
                return Err(failure(VfsError::Quota, "file-bytes", Some(rel)));
            }
            let total_cap = self
                .limits
                .max_total_bytes
                .min(self.traversal.max_raw_bytes);
            if self
                .receipt
                .raw_bytes_read
                .checked_add(before.len() as usize)
                .is_none_or(|n| n > total_cap)
            {
                return Err(failure(VfsError::Quota, "total-bytes", Some(rel)));
            }
            let mut file = OpenOptions::new()
                .read(true)
                .custom_flags(OPEN_FLAGS)
                .open(path)
                .map_err(|_| failure(VfsError::IdentityConflict, "file-open", Some(rel)))?;
            let opened = file
                .metadata()
                .map_err(|_| failure(VfsError::IdentityConflict, "file-fstat", Some(rel)))?;
            if !opened.is_file() || stamp(before) != stamp(&opened) {
                return Err(failure(
                    VfsError::IdentityConflict,
                    "file-replaced",
                    Some(rel),
                ));
            }
            let canonical = fs::canonicalize(path)
                .map_err(|_| failure(VfsError::IdentityConflict, "file-path", Some(rel)))?;
            if canonical != path || !canonical.starts_with(self.root) {
                return Err(failure(VfsError::OutsideRoot, "file-path", Some(rel)));
            }
            let mut raw = Vec::with_capacity(before.len() as usize);
            let read = read_raw(&mut file, &mut raw, cap, total_cap - self.receipt.raw_bytes_read);
            self.receipt.raw_bytes_read = self
                .receipt
                .raw_bytes_read
                .checked_add(raw.len())
                .ok_or_else(|| failure(VfsError::Quota, "total-bytes", Some(rel)))?;
            read.map_err(|_| failure(VfsError::IdentityConflict, "file-read", Some(rel)))?;
            if raw.len() > cap || self.receipt.raw_bytes_read > total_cap {
                return Err(failure(VfsError::Quota, "read-bytes", Some(rel)));
            }
            let after = file
                .metadata()
                .map_err(|_| failure(VfsError::IdentityConflict, "file-readback", Some(rel)))?;
            let named = fs::symlink_metadata(path).map_err(|_| {
                failure(VfsError::IdentityConflict, "file-name-readback", Some(rel))
            })?;
            if stamp(before) != stamp(&after)
                || stamp(&after) != stamp(&named)
                || raw.len() as u64 != after.len()
            {
                return Err(failure(
                    VfsError::IdentityConflict,
                    "file-changed",
                    Some(rel),
                ));
            }
            self.receipt.files_read += 1;
            let bom = raw.starts_with(&[0xef, 0xbb, 0xbf]);
            let text = std::str::from_utf8(if bom { &raw[3..] } else { &raw })
                .map_err(|_| failure(VfsError::IdentityConflict, "logical-utf8", Some(rel)))?;
            let crlf = text.contains("\r\n");
            let logical = text.replace("\r\n", "\n");
            if logical.contains(['\r', '\0']) || (crlf && text.replace("\r\n", "").contains('\n')) {
                return Err(failure(
                    VfsError::IdentityConflict,
                    "logical-text",
                    Some(rel),
                ));
            }
            let identity = format!("unix:{}:{}", after.dev(), after.ino());
            self.raw.push(RawFileRecord {
                path: rel.into(),
                identity: identity.clone(),
                raw_size: raw.len() as u64,
                raw_sha256: super::super::digest::sha256(&raw),
                logical_size: logical.len() as u64,
                logical_sha256: super::super::digest::sha256(logical.as_bytes()),
                physical_bom: bom,
                crlf,
            });
            self.disk.push(DiskObservation {
                path: rel.into(),
                identity,
                utf8: logical.into_bytes(),
                mtime_ns: 0,
                access: EntryAccess::RegularWithinRoot,
            });
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;
    pub(super) fn root_identity(_: &Path) -> Result<RootIdentity, VfsError> {
        Err(VfsError::OutsideRoot)
    }
    pub(super) fn capture(
        _: &Path,
        _: RootIdentity,
        _: &SnapshotLimits,
        _: TraversalLimits,
        _: &mut CaptureReceipt,
    ) -> Result<(Vec<DiskObservation>, Vec<RawFileRecord>), CaptureFailure> {
        Err(failure(VfsError::OutsideRoot, "unsupported-platform", None))
    }
}
