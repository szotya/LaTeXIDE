//! Private storage format v1. Caller-observed receipts select recovery; file
//! presence never acknowledges work. Single trusted writer required. No OS lock,
//! runtime authority, Save hook or power-loss qualification is provided here.
#[path = "durable_codec.rs"]
mod codec;
#[cfg(target_os = "macos")]
#[path = "private_directory.rs"]
pub(crate) mod private_directory;
use super::{RecoveryService, sha256};
use latexide_contracts::{
    is_sha256,
    recovery::{Checkpoint, JournalEntry, RecoveryError, RecoveryPort, RecoverySet},
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const PRIVATE_VERSION: u32 = 1;
pub const MAX_STORAGE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreScope {
    pub project_id: String,
    pub session_id: String,
    pub nonce: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding {
    pub bom: bool,
    pub crlf: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    CheckpointWrite,
    CheckpointPartiallyWritten,
    CheckpointWritten,
    CheckpointFlushed,
    CheckpointSync,
    CheckpointSynced,
    CheckpointRename,
    CheckpointRenamed,
    CheckpointParentSync,
    CheckpointParentSynced,
    JournalWrite,
    JournalPartiallyWritten,
    JournalWritten,
    JournalFlushed,
    JournalSync,
    JournalSynced,
    JournalRename,
    JournalRenamed,
    JournalParentSync,
    JournalParentSynced,
    BeforeAck,
}
#[derive(Debug, Clone, PartialEq)]
pub enum DurableError {
    Io {
        phase: Phase,
        kind: std::io::ErrorKind,
    },
    Corrupt(&'static str),
    UnsupportedVersion(u32),
    Quota,
    WrongScope,
    UnsafePath,
    UnconfirmedPlatform,
    Replay(RecoveryError),
}
impl DurableError {
    pub fn v1_error(&self) -> RecoveryError {
        match self {
            Self::Quota => RecoveryError::Quota,
            Self::UnsupportedVersion(_) => RecoveryError::UnsupportedSchema,
            Self::Replay(error) => error.clone(),
            _ => RecoveryError::Corrupt,
        }
    }
}
pub type Result<T> = std::result::Result<T, DurableError>;
/// Trusted caller receipt, retained outside the producer until ACK is observed.
/// decode() verifies integrity, not authenticity or whether a caller observed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckReceipt {
    pub scope: StoreScope,
    pub generation: u64,
    pub sequence: u64,
    pub checkpoint_sha256: String,
    pub journal_bytes: u64,
    pub journal_sha256: String,
    pub last_frame_sha256: String,
}
impl AckReceipt {
    pub fn encode(&self) -> Result<Vec<u8>> {
        codec::receipt(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        codec::decode_receipt(bytes)
    }
}
#[derive(Debug, Clone)]
pub struct Recovered {
    pub recovery: RecoverySet,
    pub encodings: Vec<Encoding>,
}
#[derive(Debug)]
pub struct PrefixRecovery {
    pub recovered: Recovered,
    pub original_error: DurableError,
    pub v1_error: RecoveryError,
    pub acknowledged_receipt_sha256: String,
    pub rejected_tail_bytes: u64,
    pub rejected_tail_sha256: String,
}
/// Optional observability/fault boundary. Injected failures are labelled by phase.
/// A successful hook is not the IO operation; real IO always follows it.
pub trait Hooks {
    fn reached(&mut self, _phase: Phase) -> std::io::Result<()> {
        Ok(())
    }
}
pub struct NoHooks;
impl Hooks for NoHooks {}
fn reached(h: &mut impl Hooks, phase: Phase) -> Result<()> {
    h.reached(phase).map_err(|e| DurableError::Io {
        phase,
        kind: e.kind(),
    })
}
fn io<T>(r: std::io::Result<T>, phase: Phase) -> Result<T> {
    r.map_err(|e| DurableError::Io {
        phase,
        kind: e.kind(),
    })
}
pub fn raw_digest(bytes: &[u8]) -> String {
    sha256::hex(bytes)
}

#[derive(Debug)]
pub struct JournalStore {
    root: PathBuf,
    scope: StoreScope,
}
impl JournalStore {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn create(
        parent: &Path,
        leaf: &str,
        scope: StoreScope,
        cp: Checkpoint,
        enc: Vec<Encoding>,
    ) -> Result<(Self, AckReceipt)> {
        Self::create_with(parent, leaf, scope, cp, enc, &mut NoHooks)
    }
    pub fn create_with(
        parent: &Path,
        leaf: &str,
        scope: StoreScope,
        cp: Checkpoint,
        enc: Vec<Encoding>,
        h: &mut impl Hooks,
    ) -> Result<(Self, AckReceipt)> {
        supported()?;
        validate_scope(&scope)?;
        if leaf.is_empty() || leaf == "." || leaf == ".." || leaf.contains(['/', '\\', '\0', ':']) {
            return Err(DurableError::UnsafePath);
        }
        private_dir(parent)?;
        validate_checkpoint(&cp, &enc)?;
        let root = parent.join(leaf);
        create_dir(&root)?;
        // Persist the new store directory entry before its first acknowledged data.
        sync_dir(parent, Phase::CheckpointParentSync)?;
        let store = Self { root, scope };
        let ack = store.write_generation(1, &cp, &enc, h)?;
        Ok((store, ack))
    }
    pub fn open(root: &Path, scope: StoreScope) -> Result<Self> {
        supported()?;
        validate_scope(&scope)?;
        private_dir(root)?;
        Ok(Self {
            root: root.to_path_buf(),
            scope,
        })
    }
    pub fn strict(&self, ack: &AckReceipt) -> Result<RecoverySet> {
        Ok(self.read_with_metadata(ack)?.recovery)
    }
    pub fn read_with_metadata(&self, ack: &AckReceipt) -> Result<Recovered> {
        self.load(ack, false)
    }
    /// Explicit opt-in only. The original strict error and rejected tail digest
    /// remain visible. No byte is truncated, deleted or rewritten.
    pub fn recover_explicit_prefix(&self, ack: &AckReceipt) -> Result<PrefixRecovery> {
        let error = match self.read_with_metadata(ack) {
            Err(e) => e,
            Ok(_) => return Err(DurableError::Corrupt("strict input has no rejected tail")),
        };
        let recovered = self.load(ack, true)?;
        let raw = read_private(&self.journal_path(ack.generation), MAX_STORAGE_BYTES)?;
        let at = usize::try_from(ack.journal_bytes).map_err(|_| DurableError::Quota)?;
        let tail = raw
            .get(at..)
            .ok_or(DurableError::Corrupt("ack prefix missing"))?;
        Ok(PrefixRecovery {
            recovered,
            v1_error: error.v1_error(),
            original_error: error,
            acknowledged_receipt_sha256: raw_digest(&ack.encode()?),
            rejected_tail_bytes: tail.len() as u64,
            rejected_tail_sha256: raw_digest(tail),
        })
    }
    pub fn append(&self, ack: &AckReceipt, entry: JournalEntry) -> Result<AckReceipt> {
        self.append_with(ack, entry, &mut NoHooks)
    }
    pub fn append_with(
        &self,
        ack: &AckReceipt,
        entry: JournalEntry,
        h: &mut impl Hooks,
    ) -> Result<AckReceipt> {
        let state = self.load(ack, false)?;
        let next = ack
            .sequence
            .checked_add(1)
            .ok_or(DurableError::Corrupt("sequence overflow"))?;
        // Validate all semantics before any write; same v1 single authority rules.
        let cp = Checkpoint {
            schema: 1,
            sequence: ack.sequence,
            documents: state.recovery.documents,
        };
        RecoveryService::new(next)
            .recover(cp, &[entry.clone()], &[])
            .map_err(DurableError::Replay)?;
        let index = self.document_index(ack, &entry.document_id)?;
        let encoding = state.encodings[index];
        validate_encoded_size(&entry.utf8, encoding)?;
        let frame = codec::entry(
            &self.scope,
            ack.generation,
            &ack.last_frame_sha256,
            &entry,
            encoding,
        )?;
        let mut bytes = read_private(&self.journal_path(ack.generation), MAX_STORAGE_BYTES)?;
        let cp_size = read_private(&self.checkpoint_path(ack.generation), MAX_STORAGE_BYTES)?.len();
        if bytes
            .len()
            .checked_add(frame.len())
            .and_then(|n| n.checked_add(cp_size))
            .is_none_or(|n| n > MAX_STORAGE_BYTES)
        {
            return Err(DurableError::Quota);
        }
        if codec::entry_count(&bytes)? >= 1024 {
            return Err(DurableError::Quota);
        }
        self.reserve_bytes(frame.len(), 0)?;
        bytes.extend_from_slice(&frame);
        reached(h, Phase::JournalWrite)?;
        let path = self.journal_path(ack.generation);
        let mut file = open_private(&path, true)?;
        if io(file.metadata(), Phase::JournalWrite)?.len() != ack.journal_bytes {
            return Err(DurableError::Corrupt("journal moved before append"));
        }
        let split = frame.len() / 2;
        io(file.write_all(&frame[..split]), Phase::JournalWrite)?;
        reached(h, Phase::JournalPartiallyWritten)?;
        io(file.write_all(&frame[split..]), Phase::JournalWrite)?;
        reached(h, Phase::JournalWritten)?;
        io(file.flush(), Phase::JournalFlushed)?;
        reached(h, Phase::JournalFlushed)?;
        reached(h, Phase::JournalSync)?;
        io(file.sync_all(), Phase::JournalSync)?;
        reached(h, Phase::JournalSynced)?;
        reached(h, Phase::JournalParentSync)?;
        sync_dir(&self.root, Phase::JournalParentSync)?;
        reached(h, Phase::JournalParentSynced)?;
        if read_private(&path, MAX_STORAGE_BYTES)? != bytes {
            return Err(DurableError::Corrupt("append readback changed"));
        }
        let new = AckReceipt {
            scope: self.scope.clone(),
            generation: ack.generation,
            sequence: next,
            checkpoint_sha256: ack.checkpoint_sha256.clone(),
            journal_bytes: bytes.len() as u64,
            journal_sha256: raw_digest(&bytes),
            last_frame_sha256: codec::frame_digest(&frame)?,
        };
        // Also enforces the 1024-record replay limit before ACK, never promoting
        // merely present bytes when validation fails.
        self.load(&new, false)?;
        reached(h, Phase::BeforeAck)?;
        Ok(new)
    }
    pub fn checkpoint(&self, ack: &AckReceipt) -> Result<AckReceipt> {
        self.checkpoint_with(ack, &mut NoHooks)
    }
    /// Rotate from exactly the caller-selected state; prior generations stay.
    pub fn checkpoint_with(&self, ack: &AckReceipt, h: &mut impl Hooks) -> Result<AckReceipt> {
        let state = self.load(ack, false)?;
        let generation = ack
            .generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt("generation overflow"))?;
        self.write_generation(
            generation,
            &Checkpoint {
                schema: 1,
                sequence: ack.sequence,
                documents: state.recovery.documents,
            },
            &state.encodings,
            h,
        )
    }
    fn write_generation(
        &self,
        generation: u64,
        cp: &Checkpoint,
        enc: &[Encoding],
        h: &mut impl Hooks,
    ) -> Result<AckReceipt> {
        let cpbytes = codec::checkpoint(&self.scope, generation, cp, enc)?;
        let cphash = raw_digest(&cpbytes);
        let journal = codec::head(&self.scope, generation, &cphash, cp.sequence)?;
        if cpbytes.len() + journal.len() > MAX_STORAGE_BYTES {
            return Err(DurableError::Quota);
        }
        self.reserve_bytes(cpbytes.len() + journal.len(), 2)?;
        atomic_new(
            &self.root,
            &self.checkpoint_path(generation),
            &cpbytes,
            true,
            h,
        )?;
        atomic_new(
            &self.root,
            &self.journal_path(generation),
            &journal,
            false,
            h,
        )?;
        let ack = AckReceipt {
            scope: self.scope.clone(),
            generation,
            sequence: cp.sequence,
            checkpoint_sha256: cphash,
            journal_bytes: journal.len() as u64,
            journal_sha256: raw_digest(&journal),
            last_frame_sha256: codec::frame_digest(&journal)?,
        };
        self.load(&ack, false)?;
        reached(h, Phase::BeforeAck)?;
        Ok(ack)
    }
    // Bound all retained generations and failed pending files without deleting
    // evidence or invalidating earlier caller receipts. No implicit compaction.
    fn reserve_bytes(&self, additional: usize, new_files: usize) -> Result<()> {
        private_dir(&self.root)?;
        let mut total = additional;
        let mut count = new_files;
        for entry in io(fs::read_dir(&self.root), Phase::JournalWrite)? {
            count += 1;
            if count > 64 {
                return Err(DurableError::Quota);
            }
            let path = io(entry, Phase::JournalWrite)?.path();
            let file = open_private(&path, false)?;
            let size = usize::try_from(io(file.metadata(), Phase::JournalWrite)?.len())
                .map_err(|_| DurableError::Quota)?;
            total = total.checked_add(size).ok_or(DurableError::Quota)?;
            if total > MAX_STORAGE_BYTES {
                return Err(DurableError::Quota);
            }
        }
        if total > MAX_STORAGE_BYTES {
            return Err(DurableError::Quota);
        }
        Ok(())
    }
    fn checkpoint_path(&self, g: u64) -> PathBuf {
        self.root.join(format!("checkpoint-{g}.rec"))
    }
    fn journal_path(&self, g: u64) -> PathBuf {
        self.root.join(format!("journal-{g}.rec"))
    }
    fn document_index(&self, ack: &AckReceipt, id: &str) -> Result<usize> {
        let cp = read_private(&self.checkpoint_path(ack.generation), MAX_STORAGE_BYTES)?;
        let (_, _, cp, _) = codec::decode_checkpoint(&cp)?;
        cp.documents
            .iter()
            .position(|d| d.document_id == id)
            .ok_or(DurableError::Corrupt("unknown document"))
    }
    fn load(&self, ack: &AckReceipt, prefix: bool) -> Result<Recovered> {
        supported()?;
        private_dir(&self.root)?;
        validate_scope(&self.scope)?;
        codec::validate_ack(ack)?;
        if ack.scope != self.scope {
            return Err(DurableError::WrongScope);
        }
        let cpraw = read_private(&self.checkpoint_path(ack.generation), MAX_STORAGE_BYTES)?;
        let raw = read_private(&self.journal_path(ack.generation), MAX_STORAGE_BYTES)?;
        if cpraw.len() + raw.len() > MAX_STORAGE_BYTES {
            return Err(DurableError::Quota);
        }
        let end = usize::try_from(ack.journal_bytes).map_err(|_| DurableError::Quota)?;
        if !prefix && raw.len() != end {
            return Err(DurableError::Corrupt("unacknowledged or truncated journal"));
        }
        let bytes = raw
            .get(..end)
            .ok_or(DurableError::Corrupt("acknowledged bytes missing"))?;
        if raw_digest(&cpraw) != ack.checkpoint_sha256 || raw_digest(bytes) != ack.journal_sha256 {
            return Err(DurableError::Corrupt("receipt byte binding"));
        }
        let (scope, generation, cp, enc) = codec::decode_checkpoint(&cpraw)?;
        if scope != self.scope {
            return Err(DurableError::WrongScope);
        }
        if generation != ack.generation {
            return Err(DurableError::Corrupt("checkpoint generation"));
        }
        validate_checkpoint(&cp, &enc)?;
        let (scope, generation, cp_hash, sequence, mut at, mut previous) =
            codec::decode_head(bytes)?;
        if scope != self.scope {
            return Err(DurableError::WrongScope);
        }
        if generation != ack.generation
            || cp_hash != ack.checkpoint_sha256
            || sequence != cp.sequence
        {
            return Err(DurableError::Corrupt("journal checkpoint binding"));
        }
        let mut entries = Vec::new();
        while at < bytes.len() {
            if entries.len() >= 1024 {
                return Err(DurableError::Quota);
            }
            let (scope, generation, prev, entry, encoding, next, digest) =
                codec::decode_entry(bytes, at)?;
            if scope != self.scope {
                return Err(DurableError::WrongScope);
            }
            if generation != ack.generation || prev != previous {
                return Err(DurableError::Corrupt("journal chain binding"));
            }
            let i = cp
                .documents
                .iter()
                .position(|d| d.document_id == entry.document_id)
                .ok_or(DurableError::Corrupt("unknown document"))?;
            if enc[i] != encoding {
                return Err(DurableError::Corrupt("encoding metadata changed"));
            }
            validate_encoded_size(&entry.utf8, encoding)?;
            entries.push(entry);
            at = next;
            previous = digest;
        }
        if previous != ack.last_frame_sha256 {
            return Err(DurableError::Corrupt("last frame binding"));
        }
        let recovery = RecoveryService::new(ack.sequence)
            .recover(cp, &entries, &[])
            .map_err(DurableError::Replay)?;
        Ok(Recovered {
            recovery,
            encodings: enc,
        })
    }
}
fn validate_checkpoint(cp: &Checkpoint, enc: &[Encoding]) -> Result<()> {
    if enc.len() != cp.documents.len() {
        return Err(DurableError::Corrupt("encoding count"));
    }
    RecoveryService::new(cp.sequence)
        .recover(cp.clone(), &[], &[])
        .map_err(DurableError::Replay)?;
    for (doc, encoding) in cp.documents.iter().zip(enc) {
        validate_encoded_size(&doc.utf8, *encoding)?;
    }
    Ok(())
}
fn validate_encoded_size(bytes: &[u8], enc: Encoding) -> Result<()> {
    let extra = if enc.crlf {
        bytes.iter().filter(|b| **b == b'\n').count()
    } else {
        0
    };
    if bytes.len() + extra + if enc.bom { 3 } else { 0 } > latexide_core::MAX_FILE_BYTES {
        return Err(DurableError::Quota);
    }
    Ok(())
}
fn validate_scope(s: &StoreScope) -> Result<()> {
    if s.project_id.is_empty()
        || s.session_id.is_empty()
        || s.project_id.len() > 128
        || s.session_id.len() > 128
        || s.project_id.contains('\0')
        || s.session_id.contains('\0')
        || !is_sha256(&s.nonce)
    {
        return Err(DurableError::WrongScope);
    }
    Ok(())
}
fn supported() -> Result<()> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(DurableError::UnconfirmedPlatform)
    }
}
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn geteuid() -> u32;
    fn renamex_np(from: *const std::ffi::c_char, to: *const std::ffi::c_char, flags: u32) -> i32;
}
#[cfg(target_os = "macos")]
fn uid() -> u32 {
    unsafe { geteuid() }
}
#[cfg(not(target_os = "macos"))]
fn uid() -> u32 {
    0
}
#[cfg(unix)]
pub(crate) fn private_dir(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(DurableError::UnsafePath);
    }
    let mut part = PathBuf::new();
    for c in path.components() {
        part.push(c);
        let m = fs::symlink_metadata(&part).map_err(|_| DurableError::UnsafePath)?;
        if m.file_type().is_symlink() {
            return Err(DurableError::UnsafePath);
        }
    }
    let m = fs::symlink_metadata(path).map_err(|_| DurableError::UnsafePath)?;
    if !m.is_dir() || m.mode() & 0o777 != 0o700 || m.uid() != uid() {
        return Err(DurableError::UnsafePath);
    }
    Ok(())
}
#[cfg(not(unix))]
pub(crate) fn private_dir(_: &Path) -> Result<()> {
    Err(DurableError::UnconfirmedPlatform)
}
#[cfg(target_os = "macos")]
pub(crate) fn create_dir(path: &Path) -> Result<()> {
    private_directory::create(path)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn create_dir(_: &Path) -> Result<()> {
    Err(DurableError::UnconfirmedPlatform)
}
pub(crate) fn ensure_private_path(path: &Path) -> Result<()> {
    supported()?;
    #[cfg(target_os = "macos")]
    {
        private_directory::ensure(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err(DurableError::UnconfirmedPlatform)
    }
}
#[cfg(unix)]
pub(crate) fn open_private(path: &Path, append: bool) -> Result<File> {
    supported()?;
    private_dir(path.parent().ok_or(DurableError::UnsafePath)?)?;
    let before = fs::symlink_metadata(path).map_err(|e| DurableError::Io {
        phase: Phase::JournalWrite,
        kind: e.kind(),
    })?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.mode() & 0o777 != 0o600
        || before.uid() != uid()
        || before.nlink() != 1
    {
        return Err(DurableError::UnsafePath);
    }
    // SDK sys/fcntl.h: O_NOFOLLOW_ANY=0x20000000 (macOS 14+).
    // ANY already covers the final component; combining it with O_NOFOLLOW is EINVAL.
    let file = io(
        OpenOptions::new()
            .read(true)
            .append(append)
            .custom_flags(0x20000000)
            .open(path),
        Phase::JournalWrite,
    )?;
    let after = io(file.metadata(), Phase::JournalWrite)?;
    if after.dev() != before.dev()
        || after.ino() != before.ino()
        || after.mode() != before.mode()
        || after.uid() != uid()
        || after.nlink() != 1
    {
        return Err(DurableError::UnsafePath);
    }
    Ok(file)
}
#[cfg(not(unix))]
pub(crate) fn open_private(_: &Path, _: bool) -> Result<File> {
    Err(DurableError::UnconfirmedPlatform)
}
pub(crate) fn read_private(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = open_private(path, false)?;
    if io(file.metadata(), Phase::JournalWrite)?.len() > max as u64 {
        return Err(DurableError::Quota);
    }
    let mut bytes = Vec::new();
    io(
        file.take(max as u64 + 1).read_to_end(&mut bytes),
        Phase::JournalWrite,
    )?;
    if bytes.len() > max {
        return Err(DurableError::Quota);
    }
    Ok(bytes)
}
#[cfg(unix)]
pub(crate) fn sync_dir(path: &Path, phase: Phase) -> Result<()> {
    private_dir(path)?;
    let file = io(
        OpenOptions::new()
            .read(true)
            .custom_flags(0x20000000)
            .open(path),
        phase,
    )?;
    io(file.sync_all(), phase)
}
#[cfg(not(unix))]
pub(crate) fn sync_dir(_: &Path, _: Phase) -> Result<()> {
    Err(DurableError::UnconfirmedPlatform)
}
#[cfg(target_os = "macos")]
fn rename_exclusive(from: &Path, to: &Path, phase: Phase) -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let from = CString::new(from.as_os_str().as_bytes()).map_err(|_| DurableError::UnsafePath)?;
    let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| DurableError::UnsafePath)?;
    // SDK sys/stdio.h: RENAME_EXCL=0x4. Never overwrite a prepared generation.
    if unsafe { renamex_np(from.as_ptr(), to.as_ptr(), 0x4) } != 0 {
        return Err(DurableError::Io {
            phase,
            kind: std::io::Error::last_os_error().kind(),
        });
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn rename_exclusive(_: &Path, _: &Path, _: Phase) -> Result<()> {
    Err(DurableError::UnconfirmedPlatform)
}
#[cfg(unix)]
fn atomic_new(
    root: &Path,
    path: &Path,
    bytes: &[u8],
    checkpoint: bool,
    h: &mut impl Hooks,
) -> Result<()> {
    use Phase::*;
    private_dir(root)?;
    let (write, partial, written, flushed, sync, synced, rename, renamed, parent, parentsynced) =
        if checkpoint {
            (
                CheckpointWrite,
                CheckpointPartiallyWritten,
                CheckpointWritten,
                CheckpointFlushed,
                CheckpointSync,
                CheckpointSynced,
                CheckpointRename,
                CheckpointRenamed,
                CheckpointParentSync,
                CheckpointParentSynced,
            )
        } else {
            (
                JournalWrite,
                JournalPartiallyWritten,
                JournalWritten,
                JournalFlushed,
                JournalSync,
                JournalSynced,
                JournalRename,
                JournalRenamed,
                JournalParentSync,
                JournalParentSynced,
            )
        };
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(DurableError::UnsafePath)?;
    let temp = root.join(format!(".{filename}.pending"));
    reached(h, write)?;
    let mut file = io(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(0x20000000)
            .open(&temp),
        write,
    )?;
    let split = bytes.len() / 2;
    io(file.write_all(&bytes[..split]), write)?;
    reached(h, partial)?;
    io(file.write_all(&bytes[split..]), write)?;
    reached(h, written)?;
    io(file.flush(), flushed)?;
    reached(h, flushed)?;
    reached(h, sync)?;
    io(file.sync_all(), sync)?;
    reached(h, synced)?;
    reached(h, rename)?;
    rename_exclusive(&temp, path, rename)?;
    reached(h, renamed)?;
    reached(h, parent)?;
    sync_dir(root, parent)?;
    reached(h, parentsynced)?;
    Ok(())
}
#[cfg(not(unix))]
fn atomic_new(_: &Path, _: &Path, _: &[u8], _: bool, _: &mut impl Hooks) -> Result<()> {
    Err(DurableError::UnconfirmedPlatform)
}
