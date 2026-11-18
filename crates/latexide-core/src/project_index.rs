//! Cached project symbols; authoritative open buffers always shadow filesystem text.
// Keep the canonical provider module and its frozen source pins unchanged.
#[path = "services/bibliography/index_adapter.rs"]
mod bibliography_index;
// Borrow the bounded text: this SHA-256 uses fixed-size blocks/tail rather than
// constructing a second source-sized padded buffer. The shared helper is unchanged.
#[path = "services/recovery/sha256.rs"]
mod source_fingerprint;

use crate::{completion::{CompletionIndexStatus, CompletionKind}, document::Document, tex_scan::{self, Token, Tokens}};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::Read, path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant, SystemTime}};

const FILES: usize = 128;
const BYTES: usize = 8 * 1024 * 1024;
const SYMBOLS: usize = 8192;
const VISITED: usize = 10_000;
#[derive(Clone)]
pub(crate) struct Symbol { pub name: String, pub kind: CompletionKind, pub arguments: Option<u8>, pub path: Arc<str>, pub line: u32, pub description: String, pub start_utf16: u32, pub length_utf16: u32 }
#[derive(PartialEq)]
enum Fingerprint { Open(u64), Disk(u64, Option<SystemTime>, String) }
impl Fingerprint {
    fn same_content(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Open(left), Self::Open(right)) => left == right,
            (Self::Disk(_, _, left), Self::Disk(_, _, right)) => !left.is_empty() && left == right,
            _ => false,
        }
    }
}
struct Entry { fingerprint: Fingerprint, symbols: Vec<Symbol>, bytes: usize, budget: usize, truncated: bool }
#[derive(Default)]
pub(crate) struct ProjectIndex {
    entries: BTreeMap<String, Entry>, order: Vec<String>, manifest: Vec<String>,
    refreshed: Option<Instant>, force: bool, discovery_partial: bool,
    pub status: CompletionIndexStatus,
}
impl ProjectIndex {
    pub fn force_refresh(&mut self) { self.force = true; }
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> { self.order.iter().filter_map(|id| self.entries.get(id)).flat_map(|entry| &entry.symbols) }
    pub fn refresh(&mut self, root: &Path, documents: &BTreeMap<String, Document>, active: &str) {
        self.refresh_with_sources(root, documents, active, Instant::now, read_source);
    }
    // Private sources keep cache timing and metadata/read transitions deterministic
    // in unit contracts; the normal Workspace path always supplies the real sources.
    fn refresh_with_sources(&mut self, root: &Path, documents: &BTreeMap<String, Document>, active: &str,
        clock: impl Fn() -> Instant, mut read_disk: impl FnMut(&Path, &str) -> Option<String>) {
        let disk_refresh = self.force || self.refreshed.is_none_or(|time| clock().saturating_duration_since(time) >= Duration::from_secs(1));
        if disk_refresh {
            (self.manifest, self.discovery_partial) = discover(root);
            self.refreshed = Some(clock());
        }
        let mut paths = vec![active.to_owned()];
        paths.extend(documents.keys().filter(|id| id.as_str() != active).cloned());
        paths.extend(self.manifest.iter().filter(|id| !documents.contains_key(*id)).cloned());
        paths.retain(|id| supported(Path::new(id)));
        self.order.clear(); self.status = CompletionIndexStatus { indexed_files: 0, truncated: self.discovery_partial, skipped_files: 0 };
        let mut bytes = 0; let mut symbols = 0;
        for path in paths {
            if self.order.len() >= FILES || symbols >= SYMBOLS { self.status.truncated = true; break; }
            let opened = documents.get(&path);
            let info = if let Some(document) = opened { Some((Fingerprint::Open(document.revision), document.text.len())) }
                else if !disk_refresh {
                    self.entries.get(&path).map(|entry| match &entry.fingerprint { Fingerprint::Disk(len, time, digest) => (Fingerprint::Disk(*len, *time, digest.clone()), entry.bytes), Fingerprint::Open(_) => (Fingerprint::Disk(0, None, String::new()), entry.bytes) })
                } else {
                    confined_path(root, &path).and_then(|p| fs::metadata(p).ok()).map(|meta| (Fingerprint::Disk(meta.len(), meta.modified().ok(), String::new()), meta.len() as usize))
                };
            let Some((mut fingerprint, size)) = info else { self.status.skipped_files += 1; self.status.truncated = true; self.entries.remove(&path); continue; };
            if size > crate::MAX_FILE_BYTES || bytes + size > BYTES { self.status.truncated = true; self.status.skipped_files += 1; continue; }
            let budget = SYMBOLS - symbols;
            let reusable = |entry: &Entry, fingerprint: &Fingerprint| entry.budget == budget && entry.fingerprint.same_content(fingerprint);
            let mut cached = self.entries.get(&path).is_some_and(|entry| reusable(entry, &fingerprint));
            let mut disk = None;
            if opened.is_none() && (disk_refresh || !cached) {
                let Some(text) = read_disk(root, &path) else { self.status.skipped_files += 1; self.status.truncated = true; self.entries.remove(&path); continue; };
                if let Fingerprint::Disk(_, _, digest) = &mut fingerprint {
                    *digest = source_fingerprint::hex(text.as_bytes());
                }
                // Every disk parse, including a warm budget miss, binds this text.
                cached = self.entries.get(&path).is_some_and(|entry| reusable(entry, &fingerprint));
                disk = Some(text);
            }
            let text = opened.map(|document| document.text.as_str()).or(disk.as_deref());
            let decoded_size = text.map_or(size, str::len);
            if bytes + decoded_size > BYTES { self.status.truncated = true; self.status.skipped_files += 1; continue; }
            if cached {
                let entry = self.entries.get_mut(&path).expect("Cached entry");
                entry.fingerprint = fingerprint;
                entry.bytes = decoded_size;
            } else {
                let text = text.expect("A cache miss has authoritative text");
                let (parsed, truncated) = parse(&path, text, budget);
                self.entries.insert(path.clone(), Entry { fingerprint, symbols: parsed, bytes: text.len(), budget, truncated });
            }
            let entry = &self.entries[&path]; bytes += entry.bytes; symbols += entry.symbols.len();
            self.status.truncated |= entry.truncated;
            self.order.push(path);
        }
        let retained: BTreeSet<_> = self.order.iter().cloned().collect(); self.entries.retain(|path, _| retained.contains(path));
        self.status.indexed_files = self.order.len() as u32; self.force = false;
    }
}
fn supported(path: &Path) -> bool { path.extension().and_then(|s| s.to_str()).is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "tex" | "ltx" | "sty" | "cls" | "bib")) }
fn discover(root: &Path) -> (Vec<String>, bool) {
    let mut pending = vec![root.to_owned()]; let mut found = BTreeSet::new(); let mut visited = 0; let mut partial = false;
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else { partial = true; continue; };
        let mut children = Vec::new();
        for entry in entries {
            visited += 1; if visited > VISITED { partial = true; break; }
            if let Ok(entry) = entry { children.push(entry); } else { partial = true; }
        }
        children.sort_by_key(|entry| entry.file_name());
        for entry in children {
            if entry.file_name().to_string_lossy().starts_with('.') { continue; }
            let Ok(kind) = entry.file_type() else { partial = true; continue; };
            if kind.is_symlink() { continue; }
            if kind.is_dir() {
                if !matches!(entry.file_name().to_str(), Some("target" | "node_modules" | "dist" | "build")) { pending.push(entry.path()); }
            } else if kind.is_file() && supported(&entry.path()) {
                if let Some(path) = entry.path().strip_prefix(root).ok().and_then(Path::to_str) { found.insert(path.replace('\\', "/")); }
            }
        }
        if visited > VISITED { break; }
    }
    (found.into_iter().collect(), partial)
}
fn confined_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = root.join(relative).canonicalize().ok()?;
    (path.starts_with(root) && path.is_file()).then_some(path)
}
fn read_source(root: &Path, relative: &str) -> Option<String> {
    let path = confined_path(root, relative)?;
    let file = fs::File::open(&path).ok()?;
    if file.metadata().ok()?.len() > crate::MAX_FILE_BYTES as u64 { return None; }
    let mut bytes = Vec::new(); file.take(crate::MAX_FILE_BYTES as u64 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > crate::MAX_FILE_BYTES { return None; }
    // Offsets must describe the same BOM-free/LF text that open_document presents.
    Document::load(relative.into(), path, bytes).ok().map(|document| document.text)
}

#[derive(Default)]
struct Coordinates { byte: usize, utf16: u32, line: u32 }
impl Coordinates {
    fn range(&mut self, text: &str, start: usize, end: usize) -> (u32, u32, u32) {
        debug_assert!(start >= self.byte);
        let prefix = &text[self.byte..start];
        self.utf16 += prefix.encode_utf16().count() as u32;
        self.line += prefix.bytes().filter(|b| *b == b'\n').count() as u32;
        let length = text[start..end].encode_utf16().count() as u32;
        let result = (self.utf16, length, self.line + 1);
        self.utf16 += length; self.byte = end;
        self.line += text[start..end].bytes().filter(|b| *b == b'\n').count() as u32;
        result
    }
}
fn parse(path: &str, text: &str, limit: usize) -> (Vec<Symbol>, bool) {
    let (mut symbols, truncated) = if Path::new(path).extension().is_some_and(|s| s.eq_ignore_ascii_case("bib")) {
        bib_symbols(path, text, limit)
    } else { tex_symbols(path, text, limit) };
    // One winner per file/kind/name for both TeX and BibTeX, including partial parses.
    let mut seen = BTreeSet::new(); symbols.reverse(); symbols.retain(|s| seen.insert((s.kind, s.name.clone()))); symbols.reverse();
    (symbols, truncated)
}
fn tex_symbols(path: &str, text: &str, limit: usize) -> (Vec<Symbol>, bool) {
    let mut symbols = Vec::new(); let mut work = 0; let origin: Arc<str> = path.into(); let mut coordinates = Coordinates::default();
    let mut signature_budget = text.len().saturating_mul(2);
    for token in Tokens::new(text) {
        let Token::Command(command) = token else { continue; };
        work += 1; if work > 32_768 || signature_budget == 0 { return (symbols, true); }
        let mut position = tex_scan::skip_space(text, command.end);
        if text.as_bytes().get(position) == Some(&b'*') { position = tex_scan::skip_space(text, position + 1); }
        let (name, kind, arguments, description, name_range) = if command.name == "label" {
            let Some(g) = tex_scan::group_limited(text, position, b'{', b'}', 512).filter(|g| g.closed) else { continue; };
            let raw = &text[g.content.clone()]; let key = raw.trim(); if !tex_scan::valid_key(key) { continue; }
            let start = g.content.start + raw.len() - raw.trim_start().len();
            (key.to_owned(), CompletionKind::Reference, None, "Címke".to_owned(), start..start + key.len())
        } else if matches!(command.name, "newcommand" | "renewcommand" | "providecommand" | "DeclareRobustCommand" | "DeclareMathOperator" | "def" | "gdef" | "edef" | "xdef" | "NewDocumentCommand" | "RenewDocumentCommand" | "ProvideDocumentCommand" | "DeclareDocumentCommand") {
            let Some((name, after, range)) = macro_name(text, position) else { continue; }; position = after;
            let arguments = if matches!(command.name, "def" | "gdef" | "edef" | "xdef") {
                let start = tex_scan::skip_space(text, position);
                text.as_bytes()[start..].iter().take(257).position(|b| *b == b'{').and_then(|n| plain_parameters(&text[start..start + n]))
            } else if command.name.ends_with("DocumentCommand") {
                let group = tex_scan::group_limited(text, position, b'{', b'}', signature_budget.min(4096));
                if let Some(g) = group {
                    signature_budget = signature_budget.saturating_sub(g.end.saturating_sub(position));
                    if g.closed { document_arguments(&text[g.content]) } else { None }
                } else { None }
            } else if command.name == "DeclareMathOperator" { Some(0) }
            else {
                classic_arguments(text, position)
            };
            (name, CompletionKind::Macro, arguments, "Saját makró".to_owned(), range)
        } else { continue; };
        if symbols.len() == limit { return (symbols, true); }
        let (start_utf16, length_utf16, line) = coordinates.range(text, name_range.start, name_range.end);
        symbols.push(Symbol { name, kind, arguments, path: Arc::clone(&origin), line, description, start_utf16, length_utf16 });
    }
    (symbols, false)
}
fn macro_name(text: &str, position: usize) -> Option<(String, usize, std::ops::Range<usize>)> {
    let mut p = tex_scan::skip_space(text, position);
    let braced = text.as_bytes().get(p) == Some(&b'{');
    if braced { p = tex_scan::skip_space(text, p + 1); }
    if text.as_bytes().get(p) != Some(&b'\\') { return None; }
    let start = p + 1; p = start;
    while text.as_bytes().get(p).copied().is_some_and(tex_scan::is_letter) {
        p += 1; if p - start > 64 { return None; }
    }
    if p == start { return None; }
    let name = text[start..p].to_owned(); let range = start - 1..p;
    if braced {
        p = tex_scan::skip_space(text, p);
        if text.as_bytes().get(p) != Some(&b'}') { return None; }
        p += 1;
    }
    Some((name, p, range))
}
fn classic_arguments(text: &str, position: usize) -> Option<u8> {
    let p = tex_scan::skip_space(text, position);
    if text.as_bytes().get(p) != Some(&b'[') { return Some(0); }
    let digit = tex_scan::skip_space(text, p + 1);
    let count = text.as_bytes().get(digit).copied().filter(u8::is_ascii_digit)? - b'0';
    let end = tex_scan::skip_space(text, digit + 1);
    if text.as_bytes().get(end) != Some(&b']') { return None; }
    let next = tex_scan::skip_space(text, end + 1);
    // Presence of a default introduces an optional first parameter; its body
    // need not be expanded/scanned to construct the conservative signature.
    Some(count.saturating_sub(u8::from(text.as_bytes().get(next) == Some(&b'['))))
}
fn plain_parameters(text: &str) -> Option<u8> {
    let mut count = 0; let mut chars = text.chars();
    while let Some(ch) = chars.next() { count += 1; if count > 9 || ch != '#' || chars.next() != char::from_digit(count as u32, 10) { return None; } }
    Some(count)
}
fn document_arguments(text: &str) -> Option<u8> {
    let mut p = 0; let mut count = 0;
    while p < text.len() {
        p = tex_scan::skip_space(text, p); if p == text.len() { break; }
        match text.as_bytes()[p] {
            b'm' => { count += 1; p += 1; },
            b'o' | b's' | b'+' | b'!' => p += 1,
            b'O' => { p = tex_scan::group(text, p + 1, b'{', b'}').filter(|g| g.closed)?.end; },
            _ => return None,
        }
        if count > 9 { return None; }
    }
    Some(count)
}
fn bib_symbols(path: &str, text: &str, limit: usize) -> (Vec<Symbol>, bool) {
    let (entries, truncated) = bibliography_index::citations(text, limit);
    let origin: Arc<str> = path.into();
    let mut coordinates = Coordinates::default();
    let symbols = entries.into_iter().map(|entry| {
        let (start_utf16, length_utf16, line) = coordinates.range(text, entry.key_bytes.start, entry.key_bytes.end);
        Symbol { name: entry.key, kind: CompletionKind::Citation, arguments: None, path: Arc::clone(&origin), line,
            description: format!("BibTeX · {}", entry.entry_type), start_utf16, length_utf16 }
    }).collect();
    // Existing parse() keeps the last declaration per file; Workspace preserves
    // active/open/lexical file precedence and exposes cross-file alternatives.
    (symbols, truncated)
}

#[cfg(test)]
mod freshness_review_tests {
    use super::*;
    use std::{cell::Cell, fs::{File, FileTimes}, sync::atomic::{AtomicUsize, Ordering}};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            loop {
                let root = std::env::temp_dir().join(format!("p12-freshness-review-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
                match fs::create_dir(&root) {
                    Ok(()) => {
                        let fixture = Self(root.canonicalize().unwrap());
                        fixture.write("main.tex", "");
                        return fixture;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create own index fixture: {error}"),
                }
            }
        }
        fn write(&self, id: &str, text: &str) { fs::write(self.0.join(id), text).unwrap(); }
        fn document(&self, id: &str) -> Document {
            let path = self.0.join(id);
            Document::load(id.into(), path.clone(), fs::read(path).unwrap()).unwrap()
        }
        fn documents(&self) -> BTreeMap<String, Document> {
            BTreeMap::from([("main.tex".into(), self.document("main.tex"))])
        }
        fn rewrite_same_metadata(&self, id: &str, text: &str) {
            let path = self.0.join(id);
            let before = fs::metadata(&path).unwrap();
            assert_eq!(before.len(), text.len() as u64);
            assert_ne!(fs::read(&path).unwrap(), text.as_bytes());
            self.write(id, text);
            File::options().write(true).open(&path).unwrap()
                .set_times(FileTimes::new().set_modified(before.modified().unwrap())).unwrap();
            let after = fs::metadata(path).unwrap();
            assert_eq!(after.len(), before.len());
            assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        }
    }

    #[test]
    fn review_01_metadata_only_change_reuses_parsed_symbols() {
        let p = Fixture::new();
        let logical = "@book{oldKey,title={T}}\n";
        p.write("refs.bib", logical);
        let documents = p.documents();
        let now = Instant::now();
        let warm = now + Duration::from_millis(100);
        let mut index = ProjectIndex::default();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || now, read_source);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "oldKey");
        let parsed = index.entries["refs.bib"].symbols.as_ptr();
        let path = p.0.join("refs.bib");
        let before = fs::metadata(&path).unwrap();
        File::options().write(true).open(&path).unwrap()
            .set_times(FileTimes::new().set_modified(before.modified().unwrap() + Duration::from_secs(2))).unwrap();
        assert_ne!(fs::metadata(&path).unwrap().modified().unwrap(), before.modified().unwrap());
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, read_source);
        assert_eq!(index.entries["refs.bib"].symbols.as_ptr(), parsed, "MTIME_ONLY_REUSES_PARSE");

        // A physical BOM/CRLF rewrite has the same authoritative logical text.
        let physical = format!("\u{feff}{}", logical.replace('\n', "\r\n"));
        p.write("refs.bib", &physical);
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, read_source);
        let entry = &index.entries["refs.bib"];
        assert_eq!(entry.symbols.as_ptr(), parsed, "ENCODING_ONLY_REUSES_PARSE");
        assert_eq!(entry.bytes, logical.len());
        match &entry.fingerprint {
            Fingerprint::Disk(raw_bytes, time, digest) => {
                assert_eq!(*raw_bytes, physical.len() as u64);
                assert_eq!(*time, fs::metadata(&path).unwrap().modified().ok());
                assert_eq!(*digest, source_fingerprint::hex(logical.as_bytes()));
            }
            _ => panic!("Expected disk fingerprint"),
        }
        p.write("refs.bib", "@book{newKey,title={T}}\n");
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, read_source);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "newKey", "CHANGED_DIGEST_REPARSES");
    }

    #[test]
    fn review_02_cache_hit_enforces_decoded_byte_budget() {
        let p = Fixture::new();
        let short = "@book{target,title={T}}\n";
        let prefix = format!("{short}%");
        let large = prefix.clone() + &"x".repeat(crate::MAX_FILE_BYTES - prefix.len());
        p.write("z.bib", short);
        let short_time = fs::metadata(p.0.join("z.bib")).unwrap().modified().unwrap();
        let documents = p.documents();
        let now = Instant::now();
        let reads = Cell::new(0);
        // Model a writer finishing after metadata() and before the real bounded read.
        let mut reader = |root: &Path, id: &str| {
            if id == "z.bib" {
                reads.set(reads.get() + 1);
                fs::write(root.join(id), large.as_bytes()).unwrap();
            }
            read_source(root, id)
        };
        let mut index = ProjectIndex::default();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || now, &mut reader);
        assert_eq!(index.entries["z.bib"].bytes, crate::MAX_FILE_BYTES);
        assert_eq!(index.entries["z.bib"].symbols[0].name, "target");

        let mut remaining = BYTES - crate::MAX_FILE_BYTES / 2;
        let mut files = 0u32;
        while remaining > 0 {
            let size = remaining.min(crate::MAX_FILE_BYTES);
            p.write(&format!("a{files:02}.tex"), &"x".repeat(size));
            files += 1;
            remaining -= size;
        }
        p.write("z.bib", short);
        File::options().write(true).open(p.0.join("z.bib")).unwrap()
            .set_times(FileTimes::new().set_modified(short_time)).unwrap();
        match &index.entries["z.bib"].fingerprint {
            Fingerprint::Disk(raw_bytes, time, digest) => {
                let metadata = fs::metadata(p.0.join("z.bib")).unwrap();
                assert_eq!(*raw_bytes, metadata.len(), "CACHE_HIT_RAW_LENGTH_PRECONDITION");
                assert_eq!(*time, metadata.modified().ok(), "CACHE_HIT_MTIME_PRECONDITION");
                assert_eq!(*digest, source_fingerprint::hex(large.as_bytes()));
            }
            _ => panic!("Expected disk fingerprint"),
        }
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || now + Duration::from_millis(100), &mut reader);
        assert_eq!(reads.get(), 2);
        assert!(index.status.truncated);
        assert_eq!(index.status.skipped_files, 1, "DECODED_CACHE_HIT_OVER_BUDGET_IS_SKIPPED");
        assert_eq!(index.status.indexed_files, files + 1);
        assert!(!index.entries.contains_key("z.bib"));
        assert!(!index.order.iter().any(|id| id == "z.bib"));
    }

    #[test]
    fn review_03_warm_miss_binds_digest_to_parsed_text() {
        let p = Fixture::new();
        let old = "@book{oldKey,title={T}}\n";
        let new = "@book{newKey,title={T}}\n";
        p.write("refs.bib", old);
        let mut documents = p.documents();
        let now = Instant::now();
        let warm = now + Duration::from_millis(100);
        let mut index = ProjectIndex::default();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || now, read_source);
        assert_eq!(index.entries["refs.bib"].budget, SYMBOLS);
        p.rewrite_same_metadata("refs.bib", new);
        p.write("a.tex", "\\label{quota}");
        documents.insert("a.tex".into(), p.document("a.tex"));
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, read_source);
        assert_eq!(index.entries["refs.bib"].budget, SYMBOLS - 1);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "newKey");
        match &index.entries["refs.bib"].fingerprint {
            Fingerprint::Disk(_, _, digest) => assert_eq!(*digest, source_fingerprint::hex(new.as_bytes()), "WARM_PARSE_STORES_ACTUAL_DIGEST"),
            _ => panic!("Expected disk fingerprint"),
        }
        p.rewrite_same_metadata("refs.bib", old);
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, read_source);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "oldKey", "RESTORED_CONTENT_CANNOT_REUSE_NEW_SYMBOLS");
    }

    #[test]
    fn review_04_force_refresh_at_fixed_pre_expiry_time() {
        let p = Fixture::new();
        p.write("refs.bib", "@book{oldKey,title={T}}\n");
        let documents = p.documents();
        let now = Instant::now();
        let warm = now + Duration::from_millis(100);
        let reads = Cell::new(0);
        let mut reader = |root: &Path, id: &str| {
            if id == "refs.bib" { reads.set(reads.get() + 1); }
            read_source(root, id)
        };
        let mut index = ProjectIndex::default();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || now, &mut reader);
        assert_eq!(reads.get(), 1);
        p.rewrite_same_metadata("refs.bib", "@book{newKey,title={T}}\n");
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, &mut reader);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "oldKey", "FIXED_WARM_TIME_RETAINS_OLD_WITHOUT_FORCE");
        assert_eq!(reads.get(), 1, "WARM_CACHE_HIT_PERFORMS_NO_READ");
        assert_eq!(index.refreshed, Some(now));
        index.force_refresh();
        index.refresh_with_sources(&p.0, &documents, "main.tex", || warm, &mut reader);
        assert_eq!(index.entries["refs.bib"].symbols[0].name, "newKey", "FORCE_UPDATES_AT_SAME_PRE_EXPIRY_TIME");
        assert_eq!(reads.get(), 2);
        assert_eq!(index.refreshed, Some(warm));
    }
}
