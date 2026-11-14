use std::collections::BTreeSet;
use std::ops::Range;
use latexide_contracts::{DiagnosticSeverity, SourceAnchor, SourceDiagnostic, Utf16Range};
use latexide_contracts::bibliography::{BibEntry, BibField, BibliographyReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentKind { Trivia, Entry, Directive, Malformed }

/// A contiguous source partition. Entry raw bytes include their field trivia.
#[derive(Debug, Clone)]
pub struct Fragment<'a> {
    pub kind: FragmentKind,
    pub bytes: Range<usize>,
    pub raw: &'a str,
}

#[derive(Debug, Clone)]
pub struct LosslessBibliography<'a> {
    source: &'a str,
    fragments: Vec<Fragment<'a>>,
    report: BibliographyReport,
}
impl<'a> LosslessBibliography<'a> {
    pub fn source(&self) -> &'a str { self.source }
    /// Reassemble the partition, rather than returning the stored source.
    pub fn render(&self) -> String { self.fragments.iter().map(|fragment| fragment.raw).collect() }
    pub fn fragments(&self) -> &[Fragment<'a>] { &self.fragments }
    pub fn report(&self) -> &BibliographyReport { &self.report }
}

struct Coordinates {
    utf16: Vec<u32>,
    lines: Vec<usize>,
}
impl Coordinates {
    fn new(source: &str) -> Self {
        let mut utf16 = vec![0; source.len() + 1];
        let mut count = 0u32;
        let mut lines = vec![0];
        for (byte, ch) in source.char_indices() {
            utf16[byte] = count;
            count = count.checked_add(ch.len_utf16() as u32).expect("source exceeds contract-v1 UTF16 range capacity");
            if ch == '\n' { lines.push(byte + 1); }
        }
        utf16[source.len()] = count;
        Self { utf16, lines }
    }
    fn range(&self, start: usize, end: usize) -> Utf16Range {
        Utf16Range { start: self.utf16[start], length: self.utf16[end] - self.utf16[start] }
    }
    fn line_column(&self, byte: usize) -> (u32, u32) {
        let line = self.lines.partition_point(|start| *start <= byte) - 1;
        (line as u32, self.utf16[byte] - self.utf16[self.lines[line]])
    }
}

struct Parser<'a> {
    source: &'a str,
    coordinates: Coordinates,
    digest: String,
    report: BibliographyReport,
    fragments: Vec<Fragment<'a>>,
    seen: BTreeSet<String>,
    repeated: BTreeSet<String>,
}

fn next(source: &str, byte: usize) -> usize {
    byte + source[byte..].chars().next().expect("nonempty source suffix").len_utf8()
}
fn comment_end(source: &str, mut byte: usize, end: usize) -> usize {
    while byte < end && source.as_bytes()[byte] != b'\n' { byte += 1; }
    byte
}
fn trivia_end(source: &str, mut byte: usize, end: usize) -> usize {
    loop {
        while byte < end && source.as_bytes()[byte].is_ascii_whitespace() { byte += 1; }
        if byte < end && source.as_bytes()[byte] == b'%' { byte = comment_end(source, byte, end); }
        else { return byte; }
    }
}
fn identifier(byte: u8) -> bool { byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':') }
fn trim_bounds(source: &str, mut start: usize, mut end: usize) -> (usize, usize) {
    while start < end && source.as_bytes()[start].is_ascii_whitespace() { start += 1; }
    while end > start && source.as_bytes()[end - 1].is_ascii_whitespace() { end -= 1; }
    (start, end)
}

/// Balanced braces and quoted strings share a scanner; percent comments apply
/// only outside brace/quote values. Delimiters in escaped scalar pairs are raw.
fn scan(source: &str, start: usize, end: usize, outer_close: Option<u8>) -> (usize, bool) {
    let mut p = start;
    let mut braces = 0usize;
    let mut quoted = false;
    let mut quoted_braces = 0usize;
    while p < end {
        let byte = source.as_bytes()[p];
        if byte == b'\\' {
            p += 1;
            if p < end { p = next(source, p); }
            continue;
        }
        if quoted {
            match byte {
                b'{' => quoted_braces += 1,
                b'}' if quoted_braces > 0 => quoted_braces -= 1,
                b'"' if quoted_braces == 0 => quoted = false,
                _ => {}
            }
        } else if braces > 0 {
            match byte { b'{' => braces += 1, b'}' => braces -= 1, _ => {} }
        } else {
            if outer_close == Some(byte) || (outer_close.is_none() && byte == b',') { return (p, true); }
            match byte {
                b'{' => braces = 1,
                b'"' => quoted = true,
                b'%' => { p = comment_end(source, p, end); continue; }
                _ => {}
            }
        }
        p = next(source, p);
    }
    (end, false)
}

impl<'a> Parser<'a> {
    fn fragment(&mut self, kind: FragmentKind, start: usize, end: usize) {
        if start < end { self.fragments.push(Fragment { kind, bytes: start..end, raw: &self.source[start..end] }); }
    }
    fn diagnostic(&mut self, code: &str, message: String, start: usize, end: usize, severity: DiagnosticSeverity) {
        let (line, character) = self.coordinates.line_column(start);
        self.report.diagnostics.push(SourceDiagnostic {
            anchor: SourceAnchor { document_id: "bibliography.bib".into(), revision: None, content_sha256: self.digest.clone(), line, character, range: self.coordinates.range(start, end) },
            severity, code: code.into(), message,
        });
    }
    fn fields(&mut self, mut p: usize, end: usize) -> Vec<BibField> {
        let mut fields = Vec::new();
        while p < end {
            p = trivia_end(self.source, p, end);
            if p == end { break; }
            if self.source.as_bytes()[p] == b',' { p += 1; continue; }
            let name_start = p;
            while p < end && identifier(self.source.as_bytes()[p]) { p += 1; }
            if p == name_start {
                let bad_end = next(self.source, p);
                self.diagnostic("expected_field", "expected a field name".into(), p, bad_end, DiagnosticSeverity::Error);
                p = bad_end;
                continue;
            }
            let name = self.source[name_start..p].to_owned();
            p = trivia_end(self.source, p, end);
            if p == end || self.source.as_bytes()[p] != b'=' {
                self.diagnostic("expected_equals", "field is missing '='".into(), p, p, DiagnosticSeverity::Error);
                fields.push(BibField { name, raw_value: String::new() });
                while p < end && self.source.as_bytes()[p] != b',' { p = next(self.source, p); }
                continue;
            }
            p = trivia_end(self.source, p + 1, end);
            let start = p;
            let (stop, comma) = scan(self.source, p, end, None);
            let (_, raw_end) = trim_bounds(self.source, start, stop);
            let raw_value = self.source[start..raw_end].to_owned();
            if raw_value.is_empty() {
                self.diagnostic("missing_field_value", "field is missing its value".into(), start, start, DiagnosticSeverity::Error);
            }
            fields.push(BibField { name, raw_value });
            p = if comma { stop + 1 } else { stop };
        }
        fields
    }
    fn entry(&mut self, start: usize, kind: &str, body: usize, body_end: usize, end: usize, closed: bool) {
        let key_start = trivia_end(self.source, body, body_end);
        let mut key_end = key_start;
        while key_end < body_end && self.source.as_bytes()[key_end] != b',' { key_end = next(self.source, key_end); }
        let (first, last) = trim_bounds(self.source, key_start, key_end);
        let key = self.source[first..last].to_owned();
        if key.is_empty() { self.diagnostic("missing_key", "entry is missing its key".into(), first, last, DiagnosticSeverity::Error); }
        let fields = if key_end < body_end { self.fields(key_end + 1, body_end) } else { Vec::new() };
        // Identity is exact and case-sensitive, like the existing citation index.
        // Do not choose a winner or change completion's existing last-declaration rule.
        if !key.is_empty() && !self.seen.insert(key.clone()) {
            if self.repeated.insert(key.clone()) { self.report.duplicate_keys.push(key.clone()); }
            self.diagnostic("duplicate_key", format!("duplicate bibliography key: {key}"), first, last, DiagnosticSeverity::Warning);
        }
        self.report.entries.push(BibEntry { key, entry_type: kind.to_owned(), fields, range: self.coordinates.range(start, end) });
        if !closed { self.diagnostic("unterminated_entry", "entry is missing its closing delimiter".into(), end, end, DiagnosticSeverity::Error); }
    }
    fn parse(mut self) -> LosslessBibliography<'a> {
        let mut p = 0usize;
        let mut partition = 0usize;
        while p < self.source.len() {
            if self.source.as_bytes()[p] == b'%' { p = comment_end(self.source, p, self.source.len()); continue; }
            if self.source.as_bytes()[p] != b'@' { p = next(self.source, p); continue; }
            let start = p;
            let kind_start = p + 1;
            p = kind_start;
            while p < self.source.len() && identifier(self.source.as_bytes()[p]) { p += 1; }
            if p == kind_start {
                self.fragment(FragmentKind::Trivia, partition, start);
                self.fragment(FragmentKind::Malformed, start, p);
                self.diagnostic("missing_entry_type", "entry is missing its type".into(), start, p, DiagnosticSeverity::Error);
                partition = p;
                continue;
            }
            let kind = self.source[kind_start..p].to_owned();
            let open = trivia_end(self.source, p, self.source.len());
            if open == self.source.len() || !matches!(self.source.as_bytes()[open], b'{' | b'(') {
                // Bound recovery to this malformed header line, not trivia lookahead.
                let end = comment_end(self.source, p, self.source.len());
                let anchor = open.min(end);
                self.fragment(FragmentKind::Trivia, partition, start);
                self.fragment(FragmentKind::Malformed, start, end);
                self.diagnostic("missing_entry_delimiter", "entry is missing its opening delimiter".into(), anchor, anchor, DiagnosticSeverity::Error);
                partition = end; p = end;
                continue;
            }
            let close = if self.source.as_bytes()[open] == b'{' { b'}' } else { b')' };
            let (body_end, closed) = scan(self.source, open + 1, self.source.len(), Some(close));
            let end = if closed { body_end + 1 } else { body_end };
            self.fragment(FragmentKind::Trivia, partition, start);
            if matches!(kind.to_ascii_lowercase().as_str(), "comment" | "string" | "preamble") {
                self.fragment(FragmentKind::Directive, start, end);
                if !closed { self.diagnostic("unterminated_directive", "directive is missing its closing delimiter".into(), end, end, DiagnosticSeverity::Error); }
            } else {
                self.entry(start, &kind, open + 1, body_end, end, closed);
                self.fragment(FragmentKind::Entry, start, end);
            }
            partition = end; p = end;
        }
        self.fragment(FragmentKind::Trivia, partition, self.source.len());
        self.report.diagnostics.sort_by_key(|diagnostic| diagnostic.anchor.range.start);
        LosslessBibliography { source: self.source, fragments: self.fragments, report: self.report }
    }
}

pub fn parse_bibliography(source: &str) -> LosslessBibliography<'_> {
    Parser {
        source,
        coordinates: Coordinates::new(source),
        digest: super::fingerprint::sha256_hex(source.as_bytes()),
        report: BibliographyReport { entries: Vec::new(), diagnostics: Vec::new(), duplicate_keys: Vec::new() },
        fragments: Vec::new(), seen: BTreeSet::new(), repeated: BTreeSet::new(),
    }.parse()
}
