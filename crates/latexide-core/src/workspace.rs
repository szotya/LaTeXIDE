use crate::{CoreError, DocumentInfo, DocumentSnapshot, MAX_OPEN_DOCUMENTS, Result, TextEdit, document::{Document, Patch}};
use serde::Serialize;
use std::{collections::{BTreeMap, VecDeque}, fs::{self, OpenOptions}, io::Write,
    path::{Component, Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};

static TEMP_ID: AtomicU64 = AtomicU64::new(1);
const HISTORY_BYTES: usize = 16 * 1024 * 1024;
const HISTORY_ENTRIES: usize = 256;

struct HistoryEntry { document_id: String, patches: Vec<Patch>, origin: String, transaction_id: u64 }
impl HistoryEntry {
    fn bytes(&self) -> usize { self.patches.iter().map(|patch| patch.before.len() + patch.after.len()).sum() }
}

#[derive(Debug, Serialize)]
pub struct FileEntry { pub path: String, pub size: u64 }

pub struct Workspace {
    root: PathBuf,
    documents: BTreeMap<String, Document>,
    undo: VecDeque<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    next_transaction: u64,
    completion_index: crate::project_index::ProjectIndex,
}

impl Workspace {
    pub fn open(root: PathBuf) -> Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() { return Err(CoreError::new("invalid_project", "A projektgyökér nem mappa.")); }
        Ok(Self { root, documents: BTreeMap::new(), undo: VecDeque::new(), redo: Vec::new(), next_transaction: 1, completion_index: Default::default() })
    }
    pub fn root(&self) -> &Path { &self.root }
    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }
    pub fn snapshots(&self) -> Vec<DocumentSnapshot> { self.documents.values().map(Document::snapshot).collect() }
    pub fn document_infos(&self) -> Vec<DocumentInfo> { self.documents.values().map(Document::info).collect() }
    pub fn syntax_window(&self, id: &str, expected_revision: u64, offset_utf16: u32, length_utf16: u32) -> Result<Vec<[u32; 3]>> {
        let document = self.documents.get(id).ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))?;
        if document.revision != expected_revision { return Err(CoreError::new("revision_conflict", "A dokumentum közben megváltozott.")); }
        Ok(crate::syntax::syntax_spans(&document.text, offset_utf16, length_utf16))
    }

    pub fn completions(&mut self, id: &str, expected_revision: u64, caret_utf16: u32) -> Result<crate::completion::CompletionSet> {
        let document = self.documents.get(id).ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))?;
        if document.revision != expected_revision { return Err(CoreError::new("revision_conflict", "A dokumentum közben megváltozott.")); }
        let context = crate::completion::context(&document.text, caret_utf16)?;
        if !context.needs_index() { return Ok(context.set); }
        self.completion_index.refresh(&self.root, &self.documents, id);
        Ok(crate::completion::with_index(context, &self.completion_index))
    }
    pub fn refresh_completion_index(&mut self) { self.completion_index.force_refresh(); }

    pub fn definition(&mut self, id: &str, expected_revision: u64, offset_utf16: u32) -> Result<crate::definition::DefinitionSet> {
        let document = self.documents.get(id).ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))?;
        if document.revision != expected_revision { return Err(CoreError::new("revision_conflict", "A dokumentum közben megváltozott.")); }
        let context = crate::definition::context(&document.text, offset_utf16)?;
        if context.symbol.is_none() { return Ok(context); }
        self.completion_index.force_refresh();
        self.completion_index.refresh(&self.root, &self.documents, id);
        Ok(crate::definition::with_index(context, &self.completion_index, &self.documents))
    }

    fn resolve(&self, relative: &str) -> Result<PathBuf> {
        let relative_path = Path::new(relative);
        if relative_path.is_absolute() || relative_path.components().any(|component|
            matches!(component, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
            return Err(CoreError::new("path_escape", "Csak a projektgyökéren belüli relatív útvonal használható."));
        }
        let canonical = self.root.join(relative_path).canonicalize()?;
        if !canonical.starts_with(&self.root) {
            return Err(CoreError::new("path_escape", "A fájl a projektgyökéren kívülre mutat."));
        }
        if !canonical.is_file() { return Err(CoreError::new("not_file", "A kijelölt útvonal nem fájl.")); }
        Ok(canonical)
    }

    pub fn open_document(&mut self, relative: &str) -> Result<DocumentSnapshot> {
        let path = self.resolve(relative)?;
        let id = path.strip_prefix(&self.root).expect("validated root").to_str()
            .ok_or_else(|| CoreError::new("invalid_path", "A fájlnév nem UTF-8."))?.replace('\\', "/");
        if let Some(document) = self.documents.get(&id) { return Ok(document.snapshot()); }
        if self.documents.len() >= MAX_OPEN_DOCUMENTS { return Err(CoreError::new("document_limit", "Legfeljebb 10 szövegfájl lehet nyitva.")); }
        if fs::metadata(&path)?.len() > crate::MAX_FILE_BYTES as u64 { return Err(CoreError::new("file_too_large", "A fájl meghaladja az 1 MiB határt.")); }
        let bytes = fs::read(&path)?;
        let document = Document::load(id.clone(), path, bytes)?;
        let snapshot = document.snapshot();
        self.documents.insert(id, document);
        Ok(snapshot)
    }

    pub fn snapshot(&self, id: &str) -> Result<DocumentSnapshot> {
        self.documents.get(id).map(Document::snapshot)
            .ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))
    }

    pub fn apply_edits(&mut self, id: &str, expected_revision: u64, edits: Vec<TextEdit>, origin: &str) -> Result<DocumentInfo> {
        let document = self.documents.get_mut(id).ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))?;
        if document.revision != expected_revision { return Err(CoreError::new("revision_conflict", "A dokumentum közben megváltozott; olvasd újra az aktuális revíziót.")); }
        let (text, patches) = document.prepare_edits(edits)?;
        if patches.is_empty() { return Ok(document.info()); }
        let transaction_id = self.next_transaction;
        self.next_transaction += 1;
        document.text = text;
        document.revision += 1;
        document.last_origin = origin.into();
        document.transaction_id = transaction_id;
        let snapshot = document.info();
        self.redo.clear();
        self.undo.push_back(HistoryEntry { document_id: id.into(), patches, origin: origin.into(), transaction_id });
        let mut bytes: usize = self.undo.iter().map(HistoryEntry::bytes).sum();
        while self.undo.len() > 1 && (bytes > HISTORY_BYTES || self.undo.len() > HISTORY_ENTRIES) {
            bytes -= self.undo.pop_front().expect("nonempty").bytes();
        }
        Ok(snapshot)
    }

    pub fn undo(&mut self) -> Result<DocumentSnapshot> {
        let entry = self.undo.pop_back().ok_or_else(|| CoreError::new("nothing_to_undo", "Nincs visszavonható módosítás."))?;
        let document = self.documents.get_mut(&entry.document_id).expect("history document stays open");
        for patch in entry.patches.iter().rev() {
            document.text.replace_range(patch.start..patch.start + patch.after.len(), &patch.before);
        }
        document.revision += 1;
        document.last_origin = format!("undo:{}", entry.origin);
        document.transaction_id = entry.transaction_id;
        let snapshot = document.snapshot();
        self.redo.push(entry);
        Ok(snapshot)
    }

    pub fn redo(&mut self) -> Result<DocumentSnapshot> {
        let entry = self.redo.pop().ok_or_else(|| CoreError::new("nothing_to_redo", "Nincs megismételhető módosítás."))?;
        let document = self.documents.get_mut(&entry.document_id).expect("history document stays open");
        for patch in &entry.patches {
            document.text.replace_range(patch.start..patch.start + patch.before.len(), &patch.after);
        }
        document.revision += 1;
        document.last_origin = format!("redo:{}", entry.origin);
        document.transaction_id = entry.transaction_id;
        let snapshot = document.snapshot();
        self.undo.push_back(entry);
        Ok(snapshot)
    }

    pub fn save_document(&mut self, id: &str, expected_revision: u64) -> Result<DocumentSnapshot> {
        let path = self.resolve(id)?;
        let document = self.documents.get_mut(id).ok_or_else(|| CoreError::new("document_not_open", "A dokumentum nincs megnyitva."))?;
        if path != document.path {
            return Err(CoreError::new("external_change", "A fájl útvonala másik célra mutat; nem írtuk felül."));
        }
        if expected_revision != document.revision { return Err(CoreError::new("revision_conflict", "A mentendő revízió közben megváltozott.")); }
        let baseline = document.bytes_for(&document.saved_text);
        if fs::read(&path)? != baseline { return Err(CoreError::new("external_change", "A fájlt másik program módosította; nem írtuk felül.")); }
        if document.text == document.saved_text { return Ok(document.snapshot()); }
        let bytes = document.bytes_for(&document.text);
        let permissions = fs::metadata(&path)?.permissions();
        if permissions.readonly() { return Err(CoreError::new("read_only", "A fájl írásvédett.")); }
        let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or("document");
        let temporary = path.with_file_name(format!(".{filename}.latexide-{}-{}.tmp", std::process::id(), TEMP_ID.fetch_add(1, Ordering::Relaxed)));
        let result = (|| -> Result<()> {
            let mut output = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
            output.write_all(&bytes)?;
            output.set_permissions(permissions)?;
            output.sync_all()?;
            if fs::read(&path)? != baseline { return Err(CoreError::new("external_change", "A fájl mentés közben megváltozott; nem írtuk felül.")); }
            fs::rename(&temporary, &path)?;
            Ok(())
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result?;
        document.saved_text = document.text.clone();
        Ok(document.snapshot())
    }

    pub fn list_files(&self) -> Result<Vec<FileEntry>> {
        let mut files = Vec::new();
        let mut pending = vec![self.root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if entry.file_name().to_string_lossy().starts_with('.') { continue; }
                let kind = entry.file_type()?;
                if kind.is_symlink() { continue; }
                if kind.is_dir() { pending.push(entry.path()); }
                else if kind.is_file() {
                    let path = entry.path().strip_prefix(&self.root).expect("project entry").to_string_lossy().replace('\\', "/");
                    files.push(FileEntry { path, size: entry.metadata()?.len() });
                }
                if files.len() + pending.len() > 10_000 { return Err(CoreError::new("project_too_large", "A projekt fájllistája meghaladja a kezdeti korlátot.")); }
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }
}
