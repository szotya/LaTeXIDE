//! Cached project symbols; authoritative open buffers always shadow filesystem text.
use crate::{completion::{CompletionIndexStatus, CompletionKind}, document::Document, tex_scan::{self, Token, Tokens}};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::Read, path::{Path, PathBuf}, sync::Arc, time::{Duration, Instant, SystemTime}};

const FILES: usize = 128;
const BYTES: usize = 8 * 1024 * 1024;
const SYMBOLS: usize = 8192;
const VISITED: usize = 10_000;
#[derive(Clone)]
pub(crate) struct Symbol { pub name: String, pub kind: CompletionKind, pub arguments: Option<u8>, pub path: Arc<str>, pub line: u32, pub description: String, pub start_utf16: u32, pub length_utf16: u32 }
#[derive(PartialEq)]
enum Fingerprint { Open(u64), Disk(u64, Option<SystemTime>) }
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
        let disk_refresh = self.force || self.refreshed.is_none_or(|time| time.elapsed() >= Duration::from_secs(1));
        if disk_refresh {
            (self.manifest, self.discovery_partial) = discover(root);
            self.refreshed = Some(Instant::now());
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
                    self.entries.get(&path).map(|entry| match &entry.fingerprint { Fingerprint::Disk(len, time) => (Fingerprint::Disk(*len, *time), entry.bytes), Fingerprint::Open(_) => (Fingerprint::Disk(0, None), entry.bytes) })
                } else {
                    confined_path(root, &path).and_then(|p| fs::metadata(p).ok()).map(|meta| (Fingerprint::Disk(meta.len(), meta.modified().ok()), meta.len() as usize))
                };
            let Some((fingerprint, size)) = info else { self.status.skipped_files += 1; self.status.truncated = true; self.entries.remove(&path); continue; };
            if size > crate::MAX_FILE_BYTES || bytes + size > BYTES { self.status.truncated = true; self.status.skipped_files += 1; continue; }
            let budget = SYMBOLS - symbols;
            let cached = self.entries.get(&path).is_some_and(|entry| entry.fingerprint == fingerprint && entry.budget == budget && !(self.force && opened.is_none()));
            if !cached {
                let disk;
                let text = if let Some(document) = opened { document.text.as_str() }
                    else { match read_source(root, &path) { Some(text) => { disk = text; disk.as_str() }, None => { self.status.skipped_files += 1; self.status.truncated = true; self.entries.remove(&path); continue; } } };
                if bytes + text.len() > BYTES { self.status.truncated = true; self.status.skipped_files += 1; continue; }
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
    let mut symbols = Vec::new(); let mut p = 0; let origin: Arc<str> = path.into(); let mut coordinates = Coordinates::default();
    while p < text.len() {
        if text.as_bytes()[p] != b'@' { p += text[p..].chars().next().unwrap().len_utf8(); continue; }
        p += 1; let kind_start = p;
        while text.as_bytes().get(p).is_some_and(u8::is_ascii_alphabetic) { p += 1; }
        let kind = text[kind_start..p].to_ascii_lowercase(); p = tex_scan::skip_space(text, p);
        let Some(open @ (b'{' | b'(')) = text.as_bytes().get(p).copied() else { continue; };
        let close = if open == b'{' { b'}' } else { b')' }; p = tex_scan::skip_space(text, p + 1);
        let key_start = p;
        while p < text.len() && !matches!(text.as_bytes()[p], b',' | b'}' | b')' | b'\n') { p += text[p..].chars().next().unwrap().len_utf8(); }
        let key = text[key_start..p].trim();
        if !matches!(kind.as_str(), "comment" | "string" | "preamble" | "") && text.as_bytes().get(p) == Some(&b',') && tex_scan::valid_key(key) {
            if symbols.len() == limit { return (symbols, true); }
            let name_start = key_start + text[key_start..p].len() - text[key_start..p].trim_start().len();
            let (start_utf16, length_utf16, line) = coordinates.range(text, name_start, name_start + key.len());
            symbols.push(Symbol { name: key.into(), kind: CompletionKind::Citation, arguments: None, path: Arc::clone(&origin), line, description: format!("BibTeX · {kind}"), start_utf16, length_utf16 });
        }
        // Skip the entire entry, respecting nested braces, quotes and escaped delimiters.
        p = key_start; let mut depth: usize = 0; let mut quoted = false;
        while p < text.len() {
            let ch = text.as_bytes()[p];
            if ch == b'\\' { p += 1; if p < text.len() { p += text[p..].chars().next().unwrap().len_utf8(); } continue; }
            if ch == b'"' && depth == 0 { quoted = !quoted; }
            if ch == b'{' { depth += 1; }
            else if ch == b'}' && depth > 0 { depth -= 1; }
            else if ch == close && depth == 0 && !quoted { p += 1; break; }
            p += text[p..].chars().next().unwrap().len_utf8();
        }
    }
    (symbols, false)
}
