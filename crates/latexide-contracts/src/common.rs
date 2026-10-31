use crate::ContractError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_EDIT_COUNT: usize = 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Utf16Range { pub start: u32, pub length: u32 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAnchor {
    pub document_id: String,
    pub revision: Option<u64>,
    pub content_sha256: String,
    pub line: u32,
    pub character: u32,
    pub range: Utf16Range,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    pub path: String,
    pub revision: Option<u64>,
    /// Producer-supplied digest of canonical logical UTF-8, not disk serialization.
    pub content_sha256: String,
    pub utf8: Vec<u8>,
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshot { pub id: String, pub epoch: u64, pub files: Vec<SnapshotFile> }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotLimits { pub max_files: usize, pub max_file_bytes: usize, pub max_total_bytes: usize }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity { Error, Warning, Information, Hint }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDiagnostic {
    pub anchor: SourceAnchor,
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionedEdit {
    pub document_id: String,
    pub expected_revision: u64,
    pub start_utf16: u32,
    pub length_utf16: u32,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformError {
    BuildFailed, LinkFailed, LaunchFailed, UnrunWorkflow, UnsupportedArchitecture,
    WrongOwner, UnsafePermissions, MissingRuntimeDirectory, UnsupportedDesktop,
}

impl WorkspaceSnapshot {
    pub fn validate(&self, limits: &SnapshotLimits) -> Result<(), ContractError> {
        if self.id.is_empty() || self.id.len() > 256 || self.id.chars().any(char::is_control) {
            return Err(ContractError::InvalidPayload);
        }
        if self.files.len() > limits.max_files { return Err(ContractError::Quota); }
        let mut paths = BTreeSet::new(); let mut total = 0usize;
        for file in &self.files {
            file.text()?;
            if !paths.insert(&file.path) { return Err(ContractError::InvalidPayload); }
            if file.utf8.len() > limits.max_file_bytes.min(MAX_FILE_BYTES) { return Err(ContractError::Quota); }
            total = total.checked_add(file.utf8.len()).ok_or(ContractError::Quota)?;
            if total > limits.max_total_bytes { return Err(ContractError::Quota); }
        }
        Ok(())
    }

    fn file(&self, id: &str) -> Result<&SnapshotFile, ContractError> {
        self.file_and_text(id).map(|(file, _)| file)
    }

    fn file_and_text(&self, id: &str) -> Result<(&SnapshotFile, &str), ContractError> {
        let mut matches = self.files.iter().filter(|file| file.path == id);
        let file = matches.next().ok_or(ContractError::InvalidPayload)?;
        if matches.next().is_some() { return Err(ContractError::InvalidPayload); }
        Ok((file, file.text()?))
    }
}

impl SnapshotFile {
    pub fn text(&self) -> Result<&str, ContractError> {
        if !crate::integration::is_relative_path(&self.path, false) || !is_sha256(&self.content_sha256)
            || self.revision == Some(0) || (self.dirty && self.revision.is_none())
        { return Err(ContractError::InvalidPayload); }
        if self.utf8.len() > MAX_FILE_BYTES { return Err(ContractError::Quota); }
        let text = std::str::from_utf8(&self.utf8).map_err(|_| ContractError::InvalidPayload)?;
        // A physical encoding BOM is removed by the producer. A logical FEFF
        // scalar can still be document content, including at offset zero.
        if text.contains(['\r', '\0']) { return Err(ContractError::InvalidPayload); }
        Ok(text)
    }
}

impl SourceAnchor {
    pub fn validate_in(&self, snapshot: &WorkspaceSnapshot) -> Result<(), ContractError> {
        let file = snapshot.file(&self.document_id)?;
        if file.revision != self.revision { return Err(ContractError::StaleRevision); }
        if !is_sha256(&self.content_sha256) { return Err(ContractError::InvalidPayload); }
        if file.content_sha256 != self.content_sha256 { return Err(ContractError::StaleSnapshot); }
        let text = file.text()?;
        let (start, _) = byte_range(text, self.range.start, self.range.length)?;
        let before = &text[..start];
        let line = before.bytes().filter(|b| *b == b'\n').count() as u32;
        let last_line = before.rsplit_once('\n').map_or(before, |(_, tail)| tail);
        if self.line != line || self.character != last_line.encode_utf16().count() as u32 {
            return Err(ContractError::InvalidRange);
        }
        Ok(())
    }
}

pub fn validate_edits(snapshot: &WorkspaceSnapshot, edits: &[RevisionedEdit]) -> Result<(), ContractError> {
    if edits.len() > MAX_EDIT_COUNT { return Err(ContractError::Quota); }
    let mut positions: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for edit in edits {
        let needed = positions.entry(&edit.document_id).or_default();
        needed.insert(edit.start_utf16);
        if let Some(end) = edit.start_utf16.checked_add(edit.length_utf16) { needed.insert(end); }
    }
    let mut texts: BTreeMap<&str, (&SnapshotFile, &str)> = BTreeMap::new();
    let mut offsets: BTreeMap<&str, BTreeMap<u32, usize>> = BTreeMap::new();
    let mut ranges: BTreeMap<&str, Vec<(u32, u32)>> = BTreeMap::new();
    let mut sizes: BTreeMap<&str, i64> = BTreeMap::new();
    for edit in edits {
        let id = edit.document_id.as_str();
        let (file, text) = match texts.get(id) {
            Some(entry) => *entry,
            None => {
                let entry = snapshot.file_and_text(id)?;
                texts.insert(id, entry);
                entry
            }
        };
        if file.revision != Some(edit.expected_revision) { return Err(ContractError::StaleRevision); }
        if edit.replacement.contains(['\r', '\0']) { return Err(ContractError::InvalidPayload); }
        if edit.replacement.len() > MAX_FILE_BYTES { return Err(ContractError::Quota); }
        let after = edit.start_utf16.checked_add(edit.length_utf16).ok_or(ContractError::InvalidRange)?;
        let mapped = offsets.entry(id).or_insert_with(|| scalar_offsets(text, &positions[id]));
        let start = *mapped.get(&edit.start_utf16).ok_or(ContractError::InvalidRange)?;
        let end = *mapped.get(&after).ok_or(ContractError::InvalidRange)?;
        ranges.entry(&edit.document_id).or_default().push((edit.start_utf16, after));
        let size = sizes.entry(&edit.document_id).or_insert(file.utf8.len() as i64);
        *size += edit.replacement.len() as i64 - (end - start) as i64;
    }
    for file_ranges in ranges.values_mut() {
        file_ranges.sort_unstable();
        if file_ranges.windows(2).any(|pair| pair[0].1 > pair[1].0 || pair[0].0 == pair[1].0) {
            return Err(ContractError::InvalidRange);
        }
    }
    if sizes.values().any(|size| *size < 0 || *size > MAX_FILE_BYTES as i64) { return Err(ContractError::Quota); }
    Ok(())
}

// Only requested endpoints are retained; each document is traversed once rather
// than once per edit. Missing entries preserve surrogate/out-of-range rejection.
fn scalar_offsets(text: &str, positions: &BTreeSet<u32>) -> BTreeMap<u32, usize> {
    let mut offsets = BTreeMap::new();
    let mut wanted = positions.iter().copied().peekable();
    let mut units = 0u32;
    for (byte, character) in text.char_indices() {
        while wanted.peek().is_some_and(|target| *target < units) { wanted.next(); }
        if wanted.peek() == Some(&units) {
            offsets.insert(units, byte);
            wanted.next();
        }
        if wanted.peek().is_none() { return offsets; }
        units += character.len_utf16() as u32;
    }
    while wanted.peek().is_some_and(|target| *target < units) { wanted.next(); }
    if wanted.peek() == Some(&units) { offsets.insert(units, text.len()); }
    offsets
}

/// Format check only. The trusted VFS/build producer must compute the digest;
/// matching digest strings are not authentication or proof of file identity.
pub fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn byte_range(text: &str, start: u32, length: u32) -> Result<(usize, usize), ContractError> {
    let end = start.checked_add(length).ok_or(ContractError::InvalidRange)?;
    let offset = |target: u32| {
        let mut units = 0u32;
        for (byte, character) in text.char_indices() {
            if units == target { return Some(byte); }
            units += character.len_utf16() as u32;
        }
        (units == target).then_some(text.len())
    };
    Ok((offset(start).ok_or(ContractError::InvalidRange)?, offset(end).ok_or(ContractError::InvalidRange)?))
}
