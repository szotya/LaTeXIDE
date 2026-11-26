//! Immutable lexical rename preview over one actual owner analysis capture.
//! No apply/Save/wire operation. Expansion, TeX include order and .bib value
//! semantics are outside this preview; unsupported cite arguments refuse it whole.
use super::workspace_adapter::WorkspaceBibliography;
use crate::{CoreError, Workspace, contracts::{SnapshotFile, SourceAnchor, Utf16Range}, tex_scan::{self, Token, Tokens}};
use std::path::Path;

const MAX_SITES: usize = 8192;
const MAX_SCAN_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameSiteKind { Declaration, Citation }

#[derive(Debug, Clone, PartialEq)]
pub struct KeyRenameEdit {
    pub kind: RenameSiteKind,
    pub source: SourceAnchor,
    pub expected_old_text: String,
    pub insertion: String,
}

#[derive(Debug)]
pub enum KeyRenameError {
    InvalidKey,
    MissingDeclaration,
    AmbiguousDeclaration { count: usize },
    NewKeyCollision,
    UnsupportedCitation { document_id: String, command_byte: usize },
    ArtifactSource { document_id: String },
    StaleCapture,
    OutsideRoot,
    Quota,
    IncompleteCapture { code: String },
}

#[derive(Debug, Clone)]
pub struct KeyRenameLimits { pub max_sites: usize, pub max_scan_bytes: usize }
impl Default for KeyRenameLimits {
    fn default() -> Self { Self { max_sites: MAX_SITES, max_scan_bytes: MAX_SCAN_BYTES } }
}

/// Borrows the immutable capture, never a second editable document store.
/// Review descriptions can be copied; they provide no later mutation authority.
#[derive(Debug)]
pub struct BibKeyRenamePreview<'a> {
    capture: &'a WorkspaceBibliography,
    old_key: String,
    new_key: String,
    edits: Vec<KeyRenameEdit>,
}
impl BibKeyRenamePreview<'_> {
    pub fn old_key(&self) -> &str { &self.old_key }
    pub fn new_key(&self) -> &str { &self.new_key }
    pub fn edits(&self) -> &[KeyRenameEdit] { &self.edits }
    pub fn capture_id(&self) -> &str { &self.capture.snapshot().id }
    pub fn capture_epoch(&self) -> u64 { self.capture.snapshot().epoch }
    pub fn revalidate(&self, workspace: &mut Workspace) -> Result<(), KeyRenameError> {
        self.capture.revalidate(workspace).map_err(capture_error)
    }
}
fn capture_error(error: CoreError) -> KeyRenameError {
    match error.code.as_str() {
        "stale_analysis_snapshot" | "revision_conflict" => KeyRenameError::StaleCapture,
        "analysis_outside_root" => KeyRenameError::OutsideRoot,
        "analysis_quota" | "snapshot_quota" => KeyRenameError::Quota,
        _ => KeyRenameError::IncompleteCapture { code: error.code },
    }
}
fn charge(work: &mut usize, bytes: usize) -> Result<(), KeyRenameError> {
    *work = work.checked_sub(bytes).ok_or(KeyRenameError::Quota)?; Ok(())
}
fn artifact(path: &str) -> bool {
    path.split('/').any(|part| part.starts_with('.') || part.starts_with("_minted-")
        || matches!(part, "target" | "build" | "dist" | "out" | "node_modules"))
}
fn citation(name: &str) -> bool {
    matches!(name, "cite" | "citep" | "citet" | "Citep" | "Citet" | "autocite" |
        "parencite" | "textcite" | "footcite" | "smartcite" | "supercite" | "nocite")
}
fn group(source: &str, p: usize, open: u8, close: u8, work: &mut usize)
    -> Result<Option<tex_scan::Group>, KeyRenameError>
{
    let cap = (*work).min(16_384);
    if cap == 0 { return Err(KeyRenameError::Quota); }
    let result = tex_scan::group_limited(source, p, open, close, cap);
    if let Some(group) = &result {
        charge(work, group.end.saturating_sub(p))?;
        if !group.closed && group.content.len() >= cap { return Err(KeyRenameError::Quota); }
    }
    Ok(result)
}
#[derive(Default)]
struct Coordinates { byte: usize, utf16: u32, line: u32, character: u32 }
impl Coordinates {
    fn anchor(&mut self, file: &SnapshotFile, source: &str, byte: usize, key: &str) -> SourceAnchor {
        for ch in source[self.byte..byte].chars() {
            self.utf16 += ch.len_utf16() as u32;
            if ch == '\n' { self.line += 1; self.character = 0; }
            else { self.character += ch.len_utf16() as u32; }
        }
        self.byte = byte;
        SourceAnchor { document_id: file.path.clone(), revision: file.revision,
            content_sha256: file.content_sha256.clone(), line: self.line, character: self.character,
            range: Utf16Range { start: self.utf16, length: key.encode_utf16().count() as u32 } }
    }
}
fn push(edits: &mut Vec<KeyRenameEdit>, edit: KeyRenameEdit, max: usize) -> Result<(), KeyRenameError> {
    if edits.len() >= max { return Err(KeyRenameError::Quota); } edits.push(edit); Ok(())
}
impl WorkspaceBibliography {
    pub fn preview_key_rename(&self, workspace: &mut Workspace, old_key: &str, new_key: &str)
        -> Result<BibKeyRenamePreview<'_>, KeyRenameError>
    {
        self.preview_key_rename_with_limits(workspace, old_key, new_key, &KeyRenameLimits::default())
    }

    /// Complete within captured canonical declarations and static cite commands
    /// in .tex/.ltx/.sty/.cls. Limits only lower ceilings. Unsupported syntax or
    /// incomplete capture yields no successful partial plan and no history entry.
    pub fn preview_key_rename_with_limits(&self, workspace: &mut Workspace, old_key: &str,
        new_key: &str, requested: &KeyRenameLimits) -> Result<BibKeyRenamePreview<'_>, KeyRenameError>
    {
        self.revalidate(workspace).map_err(capture_error)?;
        if !tex_scan::valid_key(old_key) || !tex_scan::valid_key(new_key) || old_key == new_key {
            return Err(KeyRenameError::InvalidKey);
        }
        let max_sites = requested.max_sites.min(MAX_SITES);
        let mut work = requested.max_scan_bytes.min(MAX_SCAN_BYTES);
        if max_sites == 0 || work == 0 { return Err(KeyRenameError::Quota); }
        for file in &self.snapshot().files {
            if artifact(&file.path) { return Err(KeyRenameError::ArtifactSource { document_id: file.path.clone() }); }
            charge(&mut work, file.utf8.len())?;
        }
        let declarations = self.rename_declarations().map_err(capture_error)?;
        let matching: Vec<_> = declarations.iter().filter(|declaration| declaration.key == old_key).collect();
        if matching.is_empty() { return Err(KeyRenameError::MissingDeclaration); }
        if matching.len() != 1 { return Err(KeyRenameError::AmbiguousDeclaration { count: matching.len() }); }
        if declarations.iter().any(|declaration| declaration.key == new_key) { return Err(KeyRenameError::NewKeyCollision); }
        let mut edits = Vec::new();
        push(&mut edits, KeyRenameEdit { kind: RenameSiteKind::Declaration, source: matching[0].anchor.clone(),
            expected_old_text: old_key.into(), insertion: new_key.into() }, max_sites)?;
        for file in &self.snapshot().files {
            if Path::new(&file.path).extension().is_some_and(|ext|ext.eq_ignore_ascii_case("bib")) { continue; }
            let source = file.text().map_err(|_| KeyRenameError::IncompleteCapture { code: "invalid_utf8".into() })?;
            let mut coordinates = Coordinates::default();
            for token in Tokens::new(source) {
                let Token::Command(command) = token else { continue; };
                if !citation(command.name) { continue; }
                let unsupported = || KeyRenameError::UnsupportedCitation { document_id: file.path.clone(), command_byte: command.start };
                let mut p = tex_scan::skip_space(source, command.end);
                if source.as_bytes().get(p) == Some(&b'*') { p = tex_scan::skip_space(source, p + 1); }
                while source.as_bytes().get(p) == Some(&b'[') {
                    let option = group(source, p, b'[', b']', &mut work)?.ok_or_else(&unsupported)?;
                    if !option.closed || source[option.content.clone()].chars().any(|c| matches!(c, '\\' | '{' | '}' | '$' | '#' | '%' | '&')) {
                        return Err(unsupported());
                    }
                    p = tex_scan::skip_space(source, option.end);
                }
                let argument = group(source, p, b'{', b'}', &mut work)?.ok_or_else(&unsupported)?;
                if !argument.closed { return Err(unsupported()); }
                let mut byte = argument.content.start;
                for item in source[argument.content].split(',') {
                    let key = item.trim();
                    if key.is_empty() || !tex_scan::valid_key(key) { return Err(unsupported()); }
                    if key == old_key {
                        let start = byte + item.len() - item.trim_start().len();
                        push(&mut edits, KeyRenameEdit { kind: RenameSiteKind::Citation,
                            source: coordinates.anchor(file, source, start, key), expected_old_text: key.into(), insertion: new_key.into() }, max_sites)?;
                    }
                    byte += item.len() + 1;
                }
            }
        }
        edits.sort_by(|a,b|a.source.document_id.cmp(&b.source.document_id).then_with(||a.source.range.start.cmp(&b.source.range.start)));
        self.revalidate(workspace).map_err(capture_error)?;
        Ok(BibKeyRenamePreview { capture: self, old_key: old_key.into(), new_key: new_key.into(), edits })
    }
}
