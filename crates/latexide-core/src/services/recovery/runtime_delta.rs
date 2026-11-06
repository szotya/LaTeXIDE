//! Incremental runtime commits. A checksummed high-water header selects an
//! exact, hash-chained prefix; raw appended tails are never recovery authority.
use super::*;
#[cfg(unix)]
use std::os::unix::fs::FileExt;

pub(super) const EDIT_FILE: &str = "runtime-edits.rec";
pub(super) const HEAD_FILE: &str = "runtime-head.rec";
const HEADER: usize = 248;
const MAX_LOG: usize = 32 * 1024 * 1024;
const MAX_RECORDS: usize = 1024;

pub(super) struct EditLog {
    path: PathBuf,
    head_path: PathBuf,
    base: String,
    pub sequence: u64,
    bytes: usize,
    last: String,
    records: usize,
    header: Vec<u8>,
    tail: bool,
}
fn header(base: &str, sequence: u64, bytes: usize, last: &str) -> Result<Vec<u8>> {
    let mut w = Writer(Vec::new());
    w.u64(sequence);
    w.u64(bytes as u64);
    w.string(last)?;
    w.string(base)?;
    let h = seal(b"LXLOG001", w.0)?;
    if h.len() != HEADER {
        return Err(bad("edit header size"));
    }
    Ok(h)
}
fn base_binding(root: &Path, scope: &StoreScope, hash: &str, sequence: u64) -> String {
    raw_digest(
        format!(
            "{}:{}:{}:{}:{hash}:{sequence}",
            scope.project_id,
            scope.session_id,
            scope.nonce,
            root.file_name().unwrap().to_string_lossy()
        )
        .as_bytes(),
    )
}
impl EditLog {
    pub fn create(root: &Path, scope: &StoreScope, hash: &str, sequence: u64) -> Result<Self> {
        let base = base_binding(root, scope, hash, sequence);
        let h = header(&base, sequence, 0, &base)?;
        let path = root.join(EDIT_FILE);
        let head_path = root.join(HEAD_FILE);
        new_file(&path, &[])?;
        new_file(&head_path, &h)?;
        Ok(Self {
            path,
            head_path,
            base: base.clone(),
            sequence,
            bytes: 0,
            last: base,
            records: 0,
            header: h,
            tail: false,
        })
    }
    pub fn open(
        root: &Path,
        scope: &StoreScope,
        hash: &str,
        sequence: u64,
        state: &mut State,
        project: &Path,
    ) -> Result<Option<Self>> {
        let path = root.join(EDIT_FILE);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None), // legacy checkpoint
            Err(e) => return Err(io(e)),
            Ok(_) => (),
        }
        let raw = durable::read_private(&path, MAX_LOG).map_err(durable)?;
        let head_path = root.join(HEAD_FILE);
        let header_bytes = durable::read_private(&head_path, HEADER).map_err(durable)?;
        let h = header_bytes.as_slice();
        let mut r = Reader::new(unseal(h, b"LXLOG001")?);
        let selected_sequence = r.u64()?;
        let bytes = r.count(MAX_LOG)?;
        let selected_hash = r.string(64)?;
        let base = r.string(64)?;
        r.end()?;
        if base != base_binding(root, scope, hash, sequence)
            || !latexide_contracts::is_sha256(&selected_hash)
        {
            return Err(bad("edit base binding"));
        }
        let selected = raw
            .get(..bytes)
            .ok_or_else(|| bad("selected edit prefix missing"))?;
        let (mut at, mut seq, mut last, mut records) = (0usize, sequence, base.clone(), 0usize);
        while at < selected.len() {
            let remaining = &selected[at..];
            let n = remaining
                .get(16..24)
                .ok_or_else(|| bad("short selected edit"))?;
            let len = usize::try_from(u64::from_le_bytes(n.try_into().unwrap()))
                .map_err(|_| bad("edit length"))?
                .checked_add(88)
                .ok_or_else(|| bad("edit frame overflow"))?;
            let frame = remaining
                .get(..len)
                .ok_or_else(|| bad("torn selected edit"))?;
            let mut r = Reader::new(unseal(frame, b"LXEDIT01")?);
            seq = seq
                .checked_add(1)
                .ok_or_else(|| bad("edit sequence overflow"))?;
            if r.u64()? != seq || r.string(64)? != last {
                return Err(bad("edit chain"));
            }
            apply_delta(state, &mut r, project)?;
            r.end()?;
            last = raw_digest(frame);
            at += len;
            records += 1;
            if records > MAX_RECORDS {
                return Err(bad("edit record quota"));
            }
        }
        if seq != selected_sequence || last != selected_hash {
            return Err(bad("edit high water"));
        }
        state.validate()?;
        Ok(Some(Self {
            path,
            head_path,
            base,
            sequence: seq,
            bytes,
            last,
            records,
            header: h.to_vec(),
            tail: raw.len() != bytes,
        }))
    }
}

fn text_delta(w: &mut Writer, before: &str, after: &str) -> Result<()> {
    let mut start = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(start) || !after.is_char_boundary(start) {
        start -= 1;
    }
    let mut suffix = before.as_bytes()[start..]
        .iter()
        .rev()
        .zip(after.as_bytes()[start..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(before.len() - suffix)
        || !after.is_char_boundary(after.len() - suffix)
    {
        suffix -= 1;
    }
    w.u64(start as u64);
    w.u64((before.len() - start - suffix) as u64);
    w.string(&after[start..after.len() - suffix])
}
fn apply_text(text: &mut String, r: &mut Reader<'_>) -> Result<()> {
    let start = r.count(crate::MAX_FILE_BYTES)?;
    let removed = r.count(crate::MAX_FILE_BYTES)?;
    let inserted = r.string(crate::MAX_FILE_BYTES)?;
    let end = start
        .checked_add(removed)
        .ok_or_else(|| bad("edit text overflow"))?;
    if text.get(start..end).is_none()
        || text.len() - removed + inserted.len() > crate::MAX_FILE_BYTES
    {
        return Err(bad("edit text range/quota"));
    }
    text.replace_range(start..end, &inserted);
    Ok(())
}
fn write_history(w: &mut Writer, before: &[&HistoryEntry], after: &[&HistoryEntry]) -> Result<()> {
    let start = after
        .first()
        .and_then(|h| before.iter().position(|old| old == h))
        .unwrap_or(0);
    let keep = before[start..]
        .iter()
        .zip(after)
        .take_while(|(a, b)| a == b)
        .count();
    w.u64(start as u64);
    w.u64(keep as u64);
    w.u64((after.len() - keep) as u64);
    for h in &after[keep..] {
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
    Ok(())
}
fn read_history(before: &[HistoryEntry], r: &mut Reader<'_>) -> Result<Vec<HistoryEntry>> {
    let start = r.count(HISTORY_ENTRIES)?;
    let keep = r.count(HISTORY_ENTRIES)?;
    let end = start
        .checked_add(keep)
        .ok_or_else(|| bad("edit history overflow"))?;
    let mut after = before
        .get(start..end)
        .ok_or_else(|| bad("edit history prefix"))?
        .to_vec();
    let n = r.count(HISTORY_ENTRIES)?;
    if keep + n > HISTORY_ENTRIES {
        return Err(bad("edit history quota"));
    }
    for _ in 0..n {
        let document_id = r.string(4096)?;
        let origin = r.string(16384)?;
        let transaction_id = r.u64()?;
        let count = r.count(1000)?;
        let mut patches = Vec::new();
        for _ in 0..count {
            patches.push(Patch {
                start: r.count(crate::MAX_FILE_BYTES)?,
                before: r.string(crate::MAX_FILE_BYTES)?,
                after: r.string(crate::MAX_FILE_BYTES)?,
            });
        }
        after.push(HistoryEntry {
            document_id,
            origin,
            transaction_id,
            patches,
        });
    }
    Ok(after)
}
fn encode_delta(before: &State, after: &State, sequence: u64, last: &str) -> Result<Vec<u8>> {
    let mut w = Writer(Vec::new());
    w.u64(sequence);
    w.string(last)?;
    w.u64(after.next_transaction);
    w.string(after.pending_save.as_deref().unwrap_or(""))?;
    let removed: Vec<_> = before
        .documents
        .keys()
        .filter(|id| !after.documents.contains_key(*id))
        .collect();
    w.u64(removed.len() as u64);
    for id in removed {
        w.string(id)?;
    }
    let changed: Vec<_> = after
        .documents
        .iter()
        .filter(|(id, d)| before.documents.get(*id) != Some(*d))
        .collect();
    w.u64(changed.len() as u64);
    for (id, d) in changed {
        let old = before.documents.get(id);
        w.string(id)?;
        text_delta(&mut w, old.map_or("", |v| &v.text), &d.text)?;
        text_delta(&mut w, old.map_or("", |v| &v.saved_text), &d.saved_text)?;
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
    write_history(
        &mut w,
        &before.undo.iter().collect::<Vec<_>>(),
        &after.undo.iter().collect::<Vec<_>>(),
    )?;
    write_history(
        &mut w,
        &before.redo.iter().collect::<Vec<_>>(),
        &after.redo.iter().collect::<Vec<_>>(),
    )?;
    seal(b"LXEDIT01", w.0)
}
fn apply_delta(state: &mut State, r: &mut Reader<'_>, root: &Path) -> Result<()> {
    state.next_transaction = r.u64()?;
    let pending = r.string(4096)?;
    state.pending_save = if pending.is_empty() {
        None
    } else {
        Some(pending)
    };
    let removed = r.count(crate::MAX_OPEN_DOCUMENTS)?;
    for _ in 0..removed {
        if state.documents.remove(&r.string(4096)?).is_none() {
            return Err(bad("edit missing document"));
        }
    }
    let changed = r.count(crate::MAX_OPEN_DOCUMENTS)?;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..changed {
        let id = r.string(4096)?;
        if !latexide_contracts::is_relative_path(&id, false) || !seen.insert(id.clone()) {
            return Err(bad("edit document ID"));
        }
        let old = state.documents.remove(&id);
        let (mut text, mut saved_text) = old.map(|d| (d.text, d.saved_text)).unwrap_or_default();
        apply_text(&mut text, r)?;
        apply_text(&mut saved_text, r)?;
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
        state.documents.insert(
            id.clone(),
            Document {
                path: root.join(&id),
                id,
                text,
                saved_text,
                revision,
                bom,
                line_ending,
                last_origin,
                transaction_id,
                disk_identity,
            },
        );
    }
    let undo = state.undo.iter().cloned().collect::<Vec<_>>();
    state.undo = read_history(&undo, r)?.into();
    state.redo = read_history(&state.redo, r)?;
    Ok(())
}

impl RuntimeRecovery {
    pub fn commit_change(&mut self, before: &State, after: &State) -> Result<()> {
        self.writable()?;
        self.verify_project_scope(&self.project_root)?;
        let seq = self
            .status
            .sequence
            .checked_add(1)
            .ok_or_else(|| bad("sequence exhausted"))?;
        if let Some(mut log) = self.edits.take() {
            let frame = match encode_delta(before, after, seq, &log.last) {
                Ok(frame) => frame,
                Err(e) => {
                    self.edits = Some(log);
                    return Err(e);
                }
            };
            if !log.tail && log.records < MAX_RECORDS && log.bytes + frame.len() <= MAX_LOG {
                let result = self.append_edit(&mut log, &frame, seq);
                self.edits = Some(log);
                if let Err(e) = &result {
                    self.status.last_error = Some(e.code.clone());
                }
                return result;
            }
            self.edits = Some(log);
        }
        self.commit(after)
    }
    #[cfg(unix)]
    fn append_edit(&mut self, log: &mut EditLog, frame: &[u8], sequence: u64) -> Result<()> {
        if let Err(e) = self.root_budget((frame.len() + HEADER) as u64, None) {
            if e.code != "recovery_quota" {
                return Err(e);
            }
            self.verify_project_scope(&self.project_root)?;
            self.cleanup()?;
            self.root_budget((frame.len() + HEADER) as u64, None)?;
        }
        self.verify_project_scope(&self.project_root)?;
        let selector = durable::read_private(&self.root.join("current"), 8192).map_err(durable)?;
        if self.selector_sha.as_deref() != Some(raw_digest(&selector).as_str()) {
            return Err(bad("selector changed externally"));
        }
        let checked = durable::open_private(&log.path, false).map_err(durable)?;
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(0x20000000)
            .open(&log.path)
            .map_err(io)?;
        let a = checked.metadata().map_err(io)?;
        let b = f.metadata().map_err(io)?;
        if (a.dev(), a.ino(), a.mode(), a.uid(), a.nlink())
            != (b.dev(), b.ino(), b.mode(), b.uid(), b.nlink())
            || b.len() != log.bytes as u64
        {
            return Err(bad("edit file replaced or externally extended"));
        }
        let old = durable::read_private(&log.head_path, HEADER).map_err(durable)?;
        if old != log.header {
            return Err(bad("edit high water changed externally"));
        }
        let last = raw_digest(frame);
        let next = header(&log.base, sequence, log.bytes + frame.len(), &last)?;
        let parent = log.path.parent().ok_or_else(|| bad("edit parent"))?;
        let pending = parent.join(format!(".runtime-head-{}.pending", random_hex(16)?));
        // Never overwrite a selecting receipt. Submit both files, then order
        // them at the drive before the selecting rename. A final full media
        // flush makes the newly selected prefix durable before acknowledgment.
        log.tail = true;
        f.write_all_at(frame, log.bytes as u64).map_err(io)?;
        plain_sync(&f)?;
        let mut receipt = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(0x20000000)
            .open(&pending)
            .map_err(io)?;
        receipt.write_all(&next).map_err(io)?;
        plain_sync(&receipt)?;
        publication_barrier(&f)?;
        self.phase(RecoveryPhase::Prepared)?;
        self.phase(RecoveryPhase::BeforePublish)?;
        self.verify_project_scope(&self.project_root)?;
        if durable::read_private(&log.head_path, HEADER).map_err(durable)? != log.header {
            return Err(bad("edit high water changed before publication"));
        }
        let selector = durable::read_private(&self.root.join("current"), 8192).map_err(durable)?;
        if self.selector_sha.as_deref() != Some(raw_digest(&selector).as_str()) {
            return Err(bad("selector changed before publication"));
        }
        fs::rename(&pending, &log.head_path).map_err(io)?;
        log.bytes += frame.len();
        log.sequence = sequence;
        log.last = last;
        log.records += 1;
        log.header = next;
        log.tail = false;
        self.status.sequence = sequence;
        if let Err(e) = self
            .phase(RecoveryPhase::SelectorRenamed)
            .and_then(|_| {
                OpenOptions::new()
                    .read(true)
                    .custom_flags(0x20000000)
                    .open(parent)
                    .map_err(io)
            })
            .and_then(|dir| plain_sync(&dir))
            .and_then(|_| full_media_sync(&f))
            .and_then(|_| self.phase(RecoveryPhase::Published))
        {
            self.status.blocked = true;
            return Err(CoreError::new(
                "recovery_commit_uncertain",
                format!("Edit committed but observer unconfirmed: {e}"),
            ));
        }
        self.status.last_error = if self.phase(RecoveryPhase::BeforeCleanup).is_err() {
            Some("recovery_cleanup_pending".into())
        } else {
            None
        };
        Ok(())
    }
    #[cfg(not(unix))]
    fn append_edit(&mut self, _: &mut EditLog, _: &[u8], _: u64) -> Result<()> {
        Err(CoreError::new(
            "recovery_unconfirmed_platform",
            "Incremental recovery is unconfirmed on this platform",
        ))
    }
}

pub(super) fn head_pending_name(name: &str) -> bool {
    name.len() == 54
        && name.starts_with(".runtime-head-")
        && name.ends_with(".pending")
        && name.as_bytes()[14..46]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}
#[cfg(unix)]
fn plain_sync(file: &File) -> Result<()> {
    unsafe extern "C" {
        fn fsync(fd: i32) -> i32;
    }
    loop {
        if unsafe { fsync(file.as_raw_fd()) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(io(error));
        }
    }
}

#[cfg(target_os = "macos")]
fn sync_command(file: &File, command: i32) -> std::io::Result<()> {
    unsafe extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
    }
    loop {
        // Darwin's libc wrapper consumes a variadic int even for sync commands.
        if unsafe { fcntl(file.as_raw_fd(), command, 0_i32) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
#[cfg(target_os = "macos")]
fn full_media_sync(file: &File) -> Result<()> {
    // Darwin F_FULLFSYNC. Do not silently downgrade an unsupported full flush.
    sync_command(file, 51).map_err(io)
}
#[cfg(target_os = "macos")]
fn finish_barrier(file: &File, result: std::io::Result<()>) -> Result<()> {
    match result {
        Ok(()) => Ok(()),
        // Darwin EINVAL, ENOTTY, ENOTSUP, ENOSYS: the cheaper barrier is
        // unavailable. A successful full media flush provides stronger order.
        Err(e) if matches!(e.raw_os_error(), Some(22 | 25 | 45 | 78)) => full_media_sync(file),
        Err(e) => Err(io(e)),
    }
}
#[cfg(target_os = "macos")]
fn publication_barrier(file: &File) -> Result<()> {
    // Darwin F_BARRIERFSYNC, after both log and receipt were submitted by fsync.
    finish_barrier(file, sync_command(file, 85))
}
#[cfg(all(unix, not(target_os = "macos")))]
fn publication_barrier(file: &File) -> Result<()> {
    plain_sync(file)
}
#[cfg(all(unix, not(target_os = "macos")))]
fn full_media_sync(file: &File) -> Result<()> {
    file.sync_all().map_err(io)
}

#[cfg(all(test, target_os = "macos"))]
mod barrier_tests {
    use super::*;
    #[test]
    fn unsupported_barrier_requires_a_real_full_flush_and_io_errors_propagate() {
        // Inject only the syscall result; the fallback performs a real full
        // flush on an owned regular file, and cannot hide a failing flush.
        let root = std::env::var_os("P01_RUNTIME_SCRATCH")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = root.join(format!("pr68-barrier-{}", random_hex(16).unwrap()));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(b"ordered data").unwrap();
        publication_barrier(&file).unwrap();
        for errno in [22, 25, 45, 78] {
            finish_barrier(&file, Err(std::io::Error::from_raw_os_error(errno))).unwrap();
        }
        assert!(finish_barrier(&file, Err(std::io::Error::from_raw_os_error(5))).is_err());
        let device = File::open("/dev/null").unwrap();
        assert!(finish_barrier(&device, Err(std::io::Error::from_raw_os_error(45))).is_err());
    }
}
