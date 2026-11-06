//! Versioned single-Workspace binding. Serialized states are recovery/rollback
//! values, never a second mutable runtime. No document is saved by restoration.
use crate::recovery::durable::{self, AckReceipt, Encoding, JournalStore, StoreScope, raw_digest};
use crate::{
    CoreError, Result,
    document::{Document, LineEnding, Patch},
    workspace::{HISTORY_BYTES, HISTORY_ENTRIES, HistoryEntry},
};
use latexide_contracts::recovery::{Checkpoint, RecoveredDocument};
#[cfg(unix)]
use std::os::unix::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawFd,
};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const MAX_STATE: usize = 64 * 1024 * 1024;
const MAX_ROOT: u64 = 256 * 1024 * 1024;
#[path = "runtime_delta.rs"]
mod incremental;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryPhase {
    Prepared,
    BeforePublish,
    SelectorRenamed,
    Published,
    BeforeCleanup,
}
pub trait RecoveryObserver: Send {
    fn phase(&mut self, _phase: RecoveryPhase) -> std::io::Result<()> {
        Ok(())
    }
}
pub(crate) struct NoObserver;
impl RecoveryObserver for NoObserver {}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RecoveryStatus {
    pub enabled: bool,
    pub restored: bool,
    pub sequence: u64,
    pub blocked: bool,
    pub last_error: Option<String>,
}
#[derive(Clone, PartialEq)]
pub(crate) struct State {
    pub documents: BTreeMap<String, Document>,
    pub undo: VecDeque<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    pub next_transaction: u64,
    pub pending_save: Option<String>,
}
impl State {
    fn checkpoint(&self, sequence: u64) -> (Checkpoint, Vec<Encoding>) {
        let docs = self
            .documents
            .values()
            .map(|d| RecoveredDocument {
                document_id: d.id.clone(),
                revision: d.revision,
                utf8: d.text.as_bytes().to_vec(),
                dirty: d.text != d.saved_text,
            })
            .collect();
        let enc = self
            .documents
            .values()
            .map(|d| Encoding {
                bom: d.bom,
                crlf: matches!(d.line_ending, LineEnding::Crlf),
            })
            .collect();
        (
            Checkpoint {
                schema: 1,
                sequence,
                documents: docs,
            },
            enc,
        )
    }
    fn validate(&self) -> Result<()> {
        if self.documents.len() > crate::MAX_OPEN_DOCUMENTS
            || self.undo.len() + self.redo.len() > HISTORY_ENTRIES
            || self.next_transaction == 0
        {
            return Err(bad("state bounds"));
        }
        if self
            .pending_save
            .as_ref()
            .is_some_and(|id| !self.documents.contains_key(id))
        {
            return Err(bad("pending Save target"));
        }
        let mut bytes = 0usize;
        for (id, d) in &self.documents {
            if !latexide_contracts::is_relative_path(id, false)
                || id != &d.id
                || d.revision == 0
                || d.transaction_id >= self.next_transaction
            {
                return Err(bad("document metadata"));
            }
            for s in [&d.text, &d.saved_text] {
                if s.len() > crate::MAX_FILE_BYTES
                    || s.contains(['\r', '\0'])
                    || d.bytes_for(s).len() > crate::MAX_FILE_BYTES
                {
                    return Err(bad("logical text/encoding"));
                }
            }
            if d.last_origin.len() > 16384 {
                return Err(bad("origin bound"));
            }
        }
        for h in self.undo.iter().chain(self.redo.iter()) {
            if !self.documents.contains_key(&h.document_id)
                || h.transaction_id == 0
                || h.transaction_id >= self.next_transaction
                || h.origin.len() > 16384
                || h.patches.len() > 1000
            {
                return Err(bad("history binding"));
            }
            for p in &h.patches {
                bytes = bytes
                    .checked_add(p.before.len() + p.after.len())
                    .ok_or_else(|| bad("history overflow"))?;
                if p.start > crate::MAX_FILE_BYTES
                    || p.before.len() > crate::MAX_FILE_BYTES
                    || p.after.len() > crate::MAX_FILE_BYTES
                {
                    return Err(bad("patch bounds"));
                }
            }
        }
        if bytes > HISTORY_BYTES {
            return Err(bad("history budget"));
        }
        for (history, undo) in [
            (self.undo.iter().rev().collect::<Vec<_>>(), true),
            (self.redo.iter().rev().collect::<Vec<_>>(), false),
        ] {
            let mut texts: BTreeMap<_, _> = self
                .documents
                .iter()
                .map(|(id, d)| (id.clone(), d.text.clone()))
                .collect();
            for h in history {
                let text = texts
                    .get_mut(&h.document_id)
                    .ok_or_else(|| bad("history target"))?;
                let patches: Vec<_> = if undo {
                    h.patches.iter().rev().collect()
                } else {
                    h.patches.iter().collect()
                };
                for p in patches {
                    let (from, to) = if undo {
                        (&p.after, &p.before)
                    } else {
                        (&p.before, &p.after)
                    };
                    let end = p
                        .start
                        .checked_add(from.len())
                        .ok_or_else(|| bad("patch overflow"))?;
                    if text.get(p.start..end) != Some(from.as_str()) {
                        return Err(bad("history text/scalar binding"));
                    }
                    text.replace_range(p.start..end, to);
                    if text.len() > 2 * crate::MAX_FILE_BYTES {
                        return Err(bad("history intermediate budget"));
                    }
                }
                if text.len() > crate::MAX_FILE_BYTES {
                    return Err(bad("history file budget"));
                }
            }
        }
        Ok(())
    }
    fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut w = Writer(Vec::new());
        w.u64(self.next_transaction);
        w.string(self.pending_save.as_deref().unwrap_or(""))?;
        w.u64(self.documents.len() as u64);
        for d in self.documents.values() {
            w.string(&d.id)?;
            w.string(&d.text)?;
            w.string(&d.saved_text)?;
            w.u64(d.revision);
            w.byte(u8::from(d.bom));
            w.byte(u8::from(matches!(d.line_ending, LineEnding::Crlf)));
            w.string(&d.last_origin)?;
            w.u64(d.transaction_id);
            w.byte(u8::from(d.disk_identity.is_some()));
            if let Some((dev, ino)) = d.disk_identity {
                w.u64(dev);
                w.u64(ino);
            }
        }
        for hs in [
            self.undo.iter().collect::<Vec<_>>(),
            self.redo.iter().collect::<Vec<_>>(),
        ] {
            w.u64(hs.len() as u64);
            for h in hs {
                w.string(&h.document_id)?;
                w.string(&h.origin)?;
                w.u64(h.transaction_id);
                w.u64(h.patches.len() as u64);
                for p in &h.patches {
                    w.u64(p.start as u64);
                    w.string(&p.before)?;
                    w.string(&p.after)?;
                }
            }
        }
        seal(b"LXSTATE1", w.0)
    }
    fn decode(bytes: &[u8], root: &Path) -> Result<Self> {
        let mut r = Reader::new(unseal(bytes, b"LXSTATE1")?);
        let next_transaction = r.u64()?;
        let p = r.string(4096)?;
        let pending_save = if p.is_empty() { None } else { Some(p) };
        let n = r.count(crate::MAX_OPEN_DOCUMENTS)?;
        let mut documents = BTreeMap::new();
        for _ in 0..n {
            let id = r.string(4096)?;
            if !latexide_contracts::is_relative_path(&id, false) {
                return Err(bad("relative document path"));
            }
            let text = r.string(crate::MAX_FILE_BYTES)?;
            let saved_text = r.string(crate::MAX_FILE_BYTES)?;
            let revision = r.u64()?;
            let bom = r.boolean()?;
            let line_ending = if r.boolean()? {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            };
            let last_origin = r.string(16384)?;
            let transaction_id = r.u64()?;
            let disk_identity = if r.boolean()? {
                Some((r.u64()?, r.u64()?))
            } else {
                None
            };
            let d = Document {
                id: id.clone(),
                path: root.join(&id),
                text,
                saved_text,
                revision,
                bom,
                line_ending,
                last_origin,
                transaction_id,
                disk_identity,
            };
            if documents.insert(id, d).is_some() {
                return Err(bad("duplicate document"));
            }
        }
        let mut stacks = Vec::new();
        for _ in 0..2 {
            let n = r.count(HISTORY_ENTRIES)?;
            let mut hs = Vec::new();
            for _ in 0..n {
                let document_id = r.string(4096)?;
                let origin = r.string(16384)?;
                let transaction_id = r.u64()?;
                let n = r.count(1000)?;
                let mut patches = Vec::new();
                for _ in 0..n {
                    let start = usize::try_from(r.u64()?).map_err(|_| bad("patch index"))?;
                    let before = r.string(crate::MAX_FILE_BYTES)?;
                    let after = r.string(crate::MAX_FILE_BYTES)?;
                    patches.push(Patch {
                        start,
                        before,
                        after,
                    });
                }
                hs.push(HistoryEntry {
                    document_id,
                    origin,
                    transaction_id,
                    patches,
                });
            }
            stacks.push(hs);
        }
        r.end()?;
        let redo = stacks.pop().unwrap();
        let undo = stacks.pop().unwrap().into();
        let s = Self {
            documents,
            undo,
            redo,
            next_transaction,
            pending_save,
        };
        s.validate()?;
        Ok(s)
    }
}
fn bad(message: &str) -> CoreError {
    CoreError::new("recovery_corrupt", message)
}
fn io(e: std::io::Error) -> CoreError {
    CoreError::new("recovery_io", format!("{}: {e}", e.kind()))
}
fn durable(e: durable::DurableError) -> CoreError {
    CoreError::new("recovery_storage", format!("{e:?}"))
}
struct Writer(Vec<u8>);
impl Writer {
    fn byte(&mut self, b: u8) {
        self.0.push(b)
    }
    fn u64(&mut self, n: u64) {
        self.0.extend(n.to_le_bytes())
    }
    fn bytes(&mut self, b: &[u8]) -> Result<()> {
        if b.len() > MAX_STATE
            || self
                .0
                .len()
                .checked_add(b.len() + 8)
                .is_none_or(|n| n > MAX_STATE)
        {
            return Err(CoreError::new(
                "recovery_quota",
                "Runtime snapshot exceeds 64 MiB",
            ));
        }
        self.u64(b.len() as u64);
        self.0.extend(b);
        Ok(())
    }
    fn string(&mut self, s: &str) -> Result<()> {
        self.bytes(s.as_bytes())
    }
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| bad("field overflow"))?;
        let b = self.b.get(self.at..end).ok_or_else(|| bad("short field"))?;
        self.at = end;
        Ok(b)
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.take(1)?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(bad("boolean")),
        }
    }
    fn count(&mut self, max: usize) -> Result<usize> {
        let n = usize::try_from(self.u64()?).map_err(|_| bad("count overflow"))?;
        if n > max {
            return Err(bad("count quota"));
        }
        Ok(n)
    }
    fn bytes(&mut self, max: usize) -> Result<Vec<u8>> {
        let n = self.count(max)?;
        Ok(self.take(n)?.to_vec())
    }
    fn string(&mut self, max: usize) -> Result<String> {
        String::from_utf8(self.bytes(max)?).map_err(|_| bad("UTF8"))
    }
    fn end(&self) -> Result<()> {
        if self.at != self.b.len() {
            Err(bad("trailing fields"))
        } else {
            Ok(())
        }
    }
}
fn seal(magic: &[u8; 8], body: Vec<u8>) -> Result<Vec<u8>> {
    if body.len() > MAX_STATE {
        return Err(bad("frame quota"));
    }
    let mut bytes = magic.to_vec();
    bytes.extend(1u64.to_le_bytes());
    bytes.extend((body.len() as u64).to_le_bytes());
    bytes.extend(body);
    let hash = raw_digest(&bytes);
    bytes.extend(hash.as_bytes());
    Ok(bytes)
}
fn unseal<'a>(bytes: &'a [u8], magic: &[u8; 8]) -> Result<&'a [u8]> {
    if bytes.len() < 88 || bytes.len() > MAX_STATE + 88 || &bytes[..8] != magic {
        return Err(bad("frame header"));
    }
    if u64::from_le_bytes(bytes[8..16].try_into().unwrap()) != 1 {
        return Err(CoreError::new(
            "recovery_version",
            "Unsupported runtime recovery format",
        ));
    }
    let n = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into().unwrap()))
        .map_err(|_| bad("frame length"))?;
    if n.checked_add(88) != Some(bytes.len())
        || raw_digest(&bytes[..bytes.len() - 64]).as_bytes() != &bytes[bytes.len() - 64..]
    {
        return Err(bad("frame length/checksum"));
    }
    Ok(&bytes[24..bytes.len() - 64])
}
struct Selector {
    incremental: bool,
    sequence: u64,
    current: String,
    previous: String,
    state_sha: String,
    ack: AckReceipt,
}
impl Selector {
    fn encode(&self) -> Result<Vec<u8>> {
        let mut w = Writer(Vec::new());
        w.u64(self.sequence);
        w.string(&self.current)?;
        w.string(&self.previous)?;
        w.string(&self.state_sha)?;
        w.bytes(&self.ack.encode().map_err(durable)?)?;
        seal(
            if self.incremental {
                b"LXPTR002"
            } else {
                b"LXPTR001"
            },
            w.0,
        )
    }
    fn decode(bytes: &[u8]) -> Result<Self> {
        let incremental = bytes.starts_with(b"LXPTR002");
        let mut r = Reader::new(unseal(
            bytes,
            if incremental {
                b"LXPTR002"
            } else {
                b"LXPTR001"
            },
        )?);
        let sequence = r.u64()?;
        let current = r.string(64)?;
        let previous = r.string(64)?;
        let state_sha = r.string(64)?;
        let ack = AckReceipt::decode(&r.bytes(8192)?).map_err(durable)?;
        r.end()?;
        if !cohort_name(&current)
            || (!previous.is_empty() && !cohort_name(&previous))
            || !latexide_contracts::is_sha256(&state_sha)
            || sequence != ack.sequence
        {
            return Err(bad("selector binding"));
        }
        Ok(Self {
            incremental,
            sequence,
            current,
            previous,
            state_sha,
            ack,
        })
    }
}
pub(crate) struct RuntimeRecovery {
    root: PathBuf,
    project_root: PathBuf,
    project_identity: (u64, u64),
    storage_identity: (u64, u64),
    scope: StoreScope,
    current: Option<String>,
    previous: Option<String>,
    selector_sha: Option<String>,
    edits: Option<incremental::EditLog>,
    status: RecoveryStatus,
    _lock: File,
    observer: Box<dyn RecoveryObserver>,
}
pub(crate) enum OpenRecovery {
    Enabled(RuntimeRecovery, Option<State>),
    Unavailable(CoreError),
}
fn legacy_project_id(project_root: &Path) -> Result<String> {
    #[cfg(unix)]
    let identity = {
        let m = fs::metadata(project_root).map_err(io)?;
        format!("{}:{}:{}", project_root.display(), m.dev(), m.ino())
    };
    #[cfg(not(unix))]
    let identity = project_root.to_string_lossy().into_owned();
    Ok(raw_digest(identity.as_bytes()))
}
fn project_marker(project_root: &Path) -> PathBuf {
    project_root.join(".latexide/recovery-project-id")
}
fn read_project_id(project_root: &Path) -> Result<Option<String>> {
    let p = project_marker(project_root);
    match fs::symlink_metadata(&p) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(e)),
        Ok(_) => {
            let bytes = durable::read_private(&p, 64).map_err(durable)?;
            let id = String::from_utf8(bytes).map_err(|_| bad("project identity UTF8"))?;
            if !latexide_contracts::is_sha256(&id) {
                return Err(bad("project identity"));
            }
            Ok(Some(id))
        }
    }
}

fn directory_identity(path: &Path) -> Result<(u64, u64)> {
    #[cfg(unix)]
    {
        let f = OpenOptions::new()
            .read(true)
            .custom_flags(0x20000000)
            .open(path)
            .map_err(io)?;
        let m = f.metadata().map_err(io)?;
        if !m.is_dir() {
            return Err(bad("project/storage directory identity"));
        }
        Ok((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(bad("unconfirmed directory identity"))
    }
}

impl RuntimeRecovery {
    pub fn open(
        project_root: &Path,
        root: PathBuf,
        observer: Box<dyn RecoveryObserver>,
    ) -> Result<OpenRecovery> {
        if !cfg!(target_os = "macos") {
            return Err(CoreError::new(
                "recovery_unconfirmed_platform",
                "Persistent runtime recovery is currently confirmed only on macOS",
            ));
        }
        let project_identity = directory_identity(project_root)?;
        if let Err(e) = ensure_private(&root) {
            return Ok(OpenRecovery::Unavailable(e));
        }
        let storage_identity = directory_identity(&root)?;
        let lock = match lock(&root) {
            Ok(lock) => lock,
            Err(e) => return Ok(OpenRecovery::Unavailable(e)),
        };
        if let Err(e) = ensure_private(&project_root.join(".latexide")) {
            return Ok(OpenRecovery::Unavailable(e));
        }
        let marker = read_project_id(project_root)?;
        let selector_path = root.join("current");
        let bytes = match fs::symlink_metadata(&selector_path) {
            Ok(_) => Some(durable::read_private(&selector_path, 8192).map_err(durable)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(e)),
        };
        let mut state = None;
        let (mut current, mut previous, mut selector_sha) = (None, None, None);
        let mut sequence = 0;
        let mut edits = None;
        let scope = if let Some(bytes) = bytes {
            let p = Selector::decode(&bytes)?;
            let expected = match &marker {
                Some(id) => id.clone(),
                // Migrate an already verified path-bound store at its original
                // location. A recovery-only copy cannot mint its project ID.
                None => legacy_project_id(project_root)?,
            };
            if p.ack.scope.project_id != expected || p.ack.scope.session_id != "single-workspace-v1"
            {
                return Err(bad("project scope"));
            }
            let croot = root.join(&p.current);
            let store = JournalStore::open(&croot, p.ack.scope.clone()).map_err(durable)?;
            let recovered = store.read_with_metadata(&p.ack).map_err(durable)?;
            let data = durable::read_private(&croot.join("runtime-state.bin"), MAX_STATE + 88)
                .map_err(durable)?;
            if raw_digest(&data) != p.state_sha {
                return Err(bad("runtime metadata byte binding"));
            }
            let mut s = State::decode(&data, project_root)?;
            let (cp, enc) = s.checkpoint(p.sequence);
            if cp.documents != recovered.recovery.documents
                || enc != recovered.encodings
                || recovered.recovery.last_sequence != p.sequence
            {
                return Err(bad("runtime/provider state binding"));
            }
            if p.incremental {
                edits = Some(
                    incremental::EditLog::open(
                        &croot,
                        &p.ack.scope,
                        &p.state_sha,
                        p.sequence,
                        &mut s,
                        project_root,
                    )?
                    .ok_or_else(|| bad("selected edit log missing"))?,
                );
            }
            sequence = edits.as_ref().map_or(p.sequence, |log| log.sequence);
            let scope = p.ack.scope;
            current = Some(p.current);
            previous = if p.previous.is_empty() {
                None
            } else {
                Some(p.previous)
            };
            selector_sha = Some(raw_digest(&bytes));
            state = Some(s);
            scope
        } else {
            StoreScope {
                project_id: marker.clone().unwrap_or(random_hex(32)?),
                session_id: "single-workspace-v1".into(),
                nonce: random_hex(32)?,
            }
        };
        if marker.is_none() {
            new_file(&project_marker(project_root), scope.project_id.as_bytes())?;
            dir_sync(&project_root.join(".latexide"))?;
        }
        let r = Self {
            project_root: project_root.to_path_buf(),
            project_identity,
            storage_identity,
            root,
            scope,
            current,
            previous,
            selector_sha,
            edits,
            status: RecoveryStatus {
                enabled: true,
                restored: state.is_some(),
                sequence,
                blocked: false,
                last_error: None,
            },
            _lock: lock,
            observer,
        };
        r.verify_project_scope(project_root)?;
        Ok(OpenRecovery::Enabled(r, state))
    }
    pub(crate) fn verify_project_scope(&self, project_root: &Path) -> Result<()> {
        if read_project_id(project_root)?.as_ref() != Some(&self.scope.project_id)
            || directory_identity(project_root)? != self.project_identity
            || directory_identity(&self.root)? != self.storage_identity
        {
            return Err(bad("project scope"));
        }
        #[cfg(unix)]
        {
            let held = self._lock.metadata().map_err(io)?;
            let named = durable::open_private(&self.root.join("writer.lock"), false)
                .map_err(durable)?
                .metadata()
                .map_err(io)?;
            if (held.dev(), held.ino()) != (named.dev(), named.ino()) {
                return Err(bad("writer lock identity"));
            }
        }
        Ok(())
    }
    pub fn status(&self) -> RecoveryStatus {
        self.status.clone()
    }
    pub fn writable(&self) -> Result<()> {
        if self.status.blocked {
            Err(CoreError::new(
                "recovery_commit_uncertain",
                "Recovery publication is uncertain; reopen before further mutations",
            ))
        } else {
            Ok(())
        }
    }
    fn phase(&mut self, p: RecoveryPhase) -> Result<()> {
        self.observer.phase(p).map_err(io)
    }
    pub fn commit(&mut self, state: &State) -> Result<()> {
        self.writable()?;
        let r = self.commit_inner(state);
        if let Err(e) = &r {
            self.status.last_error = Some(e.code.clone());
        }
        r
    }
    fn commit_inner(&mut self, state: &State) -> Result<()> {
        self.verify_project_scope(&self.project_root)?;
        self.cleanup()?;
        let data = state.encode()?;
        let sequence = self
            .status
            .sequence
            .checked_add(1)
            .ok_or_else(|| bad("sequence exhausted"))?;
        let name = format!("cohort-{}", random_hex(16)?);
        let (cp, enc) = state.checkpoint(sequence);
        let (_store, ack) = JournalStore::create(&self.root, &name, self.scope.clone(), cp, enc)
            .map_err(durable)?;
        let croot = self.root.join(&name);
        new_file(&croot.join("runtime-state.bin"), &data)?;
        let edits =
            incremental::EditLog::create(&croot, &self.scope, &raw_digest(&data), sequence)?;
        dir_sync(&croot)?;
        self.phase(RecoveryPhase::Prepared)?;
        let selector = Selector {
            incremental: true,
            sequence,
            current: name.clone(),
            previous: self.current.clone().unwrap_or_default(),
            state_sha: raw_digest(&data),
            ack,
        };
        let bytes = selector.encode()?;
        self.root_budget(bytes.len() as u64)?;
        let temp = self
            .root
            .join(format!("current-{}.pending", random_hex(16)?));
        new_file(&temp, &bytes)?;
        self.phase(RecoveryPhase::BeforePublish)?;
        self.verify_project_scope(&self.project_root)?;
        let existing = match fs::symlink_metadata(self.root.join("current")) {
            Ok(_) => Some(raw_digest(
                &durable::read_private(&self.root.join("current"), 8192).map_err(durable)?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(e)),
        };
        if existing != self.selector_sha {
            return Err(bad("selector changed externally"));
        }
        fs::rename(&temp, self.root.join("current")).map_err(io)?;
        self.previous = self.current.replace(name);
        self.edits = Some(edits);
        self.status.sequence = sequence;
        self.selector_sha = Some(raw_digest(&bytes));
        if let Err(error) = self
            .phase(RecoveryPhase::SelectorRenamed)
            .and_then(|_| dir_sync(&self.root))
            .and_then(|_| self.phase(RecoveryPhase::Published))
        {
            self.status.blocked = true;
            return Err(CoreError::new(
                "recovery_commit_uncertain",
                format!(
                    "Complete state published but directory-sync/observer unconfirmed: {error}"
                ),
            ));
        }
        self.status.last_error = None;
        if self
            .phase(RecoveryPhase::BeforeCleanup)
            .and_then(|_| self.cleanup())
            .is_err()
        {
            self.status.last_error = Some("recovery_cleanup_pending".into());
        }
        Ok(())
    }
    fn cleanup(&self) -> Result<()> {
        durable::private_dir(&self.root).map_err(durable)?;
        'entries: for e in fs::read_dir(&self.root).map_err(io)? {
            let e = e.map_err(io)?;
            let Some(name) = e.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if cohort_name(&name) {
                if self.current.as_ref() == Some(&name) || self.previous.as_ref() == Some(&name) {
                    continue;
                }
                if durable::private_dir(&e.path()).is_err() {
                    continue;
                }
                let mut files = Vec::new();
                for f in fs::read_dir(e.path()).map_err(io)? {
                    let f = f.map_err(io)?;
                    let Some(n) = f.file_name().to_str().map(str::to_owned) else {
                        continue 'entries;
                    };
                    if ![
                        "checkpoint-1.rec",
                        "journal-1.rec",
                        "runtime-state.bin",
                        incremental::EDIT_FILE,
                        incremental::HEAD_FILE,
                        ".checkpoint-1.rec.pending",
                        ".journal-1.rec.pending",
                    ]
                    .contains(&n.as_str())
                        && !incremental::head_pending_name(&n)
                    {
                        continue 'entries;
                    }
                    if durable::open_private(&f.path(), false).is_err() {
                        continue 'entries;
                    }
                    files.push(f.path());
                }
                for f in files {
                    fs::remove_file(f).map_err(io)?;
                }
                fs::remove_dir(e.path()).map_err(io)?;
            } else if name.starts_with("current-")
                && name.ends_with(".pending")
                && name.len() == 48
                && name.as_bytes()[8..40]
                    .iter()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                if durable::open_private(&e.path(), false).is_err() {
                    continue;
                }
                fs::remove_file(e.path()).map_err(io)?;
            }
        }
        dir_sync(&self.root)
    }
    fn root_budget(&self, extra: u64) -> Result<()> {
        let mut n = extra;
        let mut count = 0;
        let mut dirs = vec![self.root.clone()];
        while let Some(dir) = dirs.pop() {
            directory_identity(&dir)?;
            for e in fs::read_dir(dir).map_err(io)? {
                let p = e.map_err(io)?.path();
                let metadata = fs::symlink_metadata(&p).map_err(io)?;
                count += 1;
                n = n
                    .checked_add(metadata.len())
                    .ok_or_else(|| bad("storage overflow"))?;
                if n > MAX_ROOT || count > 32 {
                    return Err(CoreError::new(
                        "recovery_quota",
                        "Runtime recovery retained storage exceeds budget",
                    ));
                }
                if metadata.is_dir() {
                    dirs.push(p);
                }
            }
        }
        if n > MAX_ROOT || count > 32 {
            return Err(CoreError::new(
                "recovery_quota",
                "Runtime recovery retained storage exceeds budget",
            ));
        }
        Ok(())
    }
}
fn cohort_name(s: &str) -> bool {
    s.len() == 39
        && s.starts_with("cohort-")
        && s[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn random_hex(n: usize) -> Result<String> {
    let mut b = vec![0; n];
    File::open("/dev/urandom")
        .map_err(io)?
        .read_exact(&mut b)
        .map_err(io)?;
    Ok(b.iter().map(|b| format!("{b:02x}")).collect())
}
fn ensure_private(p: &Path) -> Result<()> {
    durable::ensure_private_path(p).map_err(durable)
}

#[cfg(all(test, target_os = "macos"))]
#[path = "ancestor_tests.rs"]
mod ancestor_tests;
#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
#[cfg(target_os = "macos")]
fn lock(root: &Path) -> Result<File> {
    let path = root.join("writer.lock");
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(0x20000000)
        .open(&path)
        .map_err(io)?;
    durable::open_private(&path, false).map_err(durable)?;
    if unsafe { flock(f.as_raw_fd(), 2 | 4) } != 0 {
        return Err(CoreError::new(
            "recovery_in_use",
            "This project recovery store already has a writer",
        ));
    }
    Ok(f)
}
#[cfg(not(target_os = "macos"))]
fn lock(_: &Path) -> Result<File> {
    Err(CoreError::new(
        "recovery_unconfirmed_platform",
        "Private writer lock is unconfirmed on this platform",
    ))
}
#[cfg(unix)]
fn new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0x20000000)
        .open(path)
        .map_err(io)?;
    f.write_all(bytes).map_err(io)?;
    f.flush().map_err(io)?;
    f.sync_all().map_err(io)
}
#[cfg(not(unix))]
fn new_file(_: &Path, _: &[u8]) -> Result<()> {
    Err(CoreError::new(
        "recovery_unconfirmed_platform",
        "Private storage unconfirmed",
    ))
}
#[cfg(unix)]
fn dir_sync(path: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .custom_flags(0x20000000)
        .open(path)
        .map_err(io)?
        .sync_all()
        .map_err(io)
}
#[cfg(not(unix))]
fn dir_sync(_: &Path) -> Result<()> {
    Err(CoreError::new(
        "recovery_unconfirmed_platform",
        "Directory sync unconfirmed",
    ))
}
