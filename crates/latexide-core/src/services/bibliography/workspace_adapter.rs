//! Read-only canonical bibliography projection of an actual owner analysis capture.
//! A result owns one bounded immutable snapshot, never an editable store/cache.
//! Revalidation is sequential observation under the same Workspace borrow, not
//! authentication, a cross-thread publication lease or filesystem race isolation.
use super::LocalBibliographyProvider;
use crate::{CoreError, Result, Workspace, MAX_FILE_BYTES, completion::CompletionKind,
    contracts::{SnapshotLimits, SourceAnchor, Utf16Range, WorkspaceSnapshot}};
use std::{collections::BTreeSet, path::Path};

// Reuse the unchanged accepted-entry projection and its one canonical parser.
// No standalone bibliography.bib diagnostic anchor is forwarded as a project anchor.
#[path = "index_adapter.rs"]
mod canonical_index;

const FILES: usize = 128;
const BYTES: usize = 8 * 1024 * 1024;
const SYMBOLS: usize = 8192;

#[derive(Debug, Clone, PartialEq)]
pub struct BibliographyTarget {
    pub key: String,
    pub entry_type: String,
    pub anchor: SourceAnchor,
    pub dirty: bool,
}

/// Only the existing lexical cite subset is resolved. Macro expansion,
/// crossref/xdata/schema resolution and unknown command semantics are unsupported.
#[derive(Debug, Clone, PartialEq)]
pub enum CitationLookup {
    Resolved { target: BibliographyTarget, alternatives: u32 },
    Missing { key: String },
    Unsupported,
}

#[derive(Debug)]
pub struct WorkspaceBibliography {
    snapshot: WorkspaceSnapshot,
    targets: Vec<BibliographyTarget>,
}

// Human-readable context for a failed whole capture. This does not inspect
// another filesystem view or reinterpret a refused observation as successful.
fn capture_error(mut error: CoreError) -> CoreError {
    if error.code != "analysis_incomplete" { return error; }
    error.message = format!("Bibliography requires a complete text-only project root. Unsupported binary/non-text assets (for example PNG/PDF figures) refuse the whole capture. Underlying producer refusal: {}", error.message);
    error
}

#[derive(Default)]
struct Coordinates { byte: usize, utf16: u32, line: u32, character: u32 }
impl Coordinates {
    fn advance(&mut self, source: &str, byte: usize) {
        for ch in source[self.byte..byte].chars() {
            self.utf16 += ch.len_utf16() as u32;
            if ch == '\n' { self.line += 1; self.character = 0; }
            else { self.character += ch.len_utf16() as u32; }
        }
        self.byte = byte;
    }
}

impl LocalBibliographyProvider {
    /// Read-only explanation of the text-only producer requirement on failure.
    /// It preserves the error code and creates no observation or edit authority.
    pub fn capture_diagnostic(&self, error: CoreError) -> CoreError {
        capture_error(error)
    }
    /// Requires a complete text-only project root from the production capture.
    /// Binary/non-text assets refuse the whole operation; no filtered success is
    /// synthesized. Capture errors retain their code and explain this cause.
    pub fn inspect_workspace(&self, workspace: &mut Workspace) -> Result<WorkspaceBibliography> {
        self.inspect_workspace_with_limits(workspace, &SnapshotLimits {
            max_files: FILES, max_file_bytes: MAX_FILE_BYTES, max_total_bytes: BYTES,
        })
    }

    /// Callers may lower the existing index admission ceilings, never raise them.
    /// P00 capture supplies exact disk/dirty logical UTF8 and refuses incomplete
    /// observation before this adapter can expose any successful partial result.
    /// This includes unrelated binary assets anywhere in the project root.
    pub fn inspect_workspace_with_limits(&self, workspace: &mut Workspace, requested: &SnapshotLimits)
        -> Result<WorkspaceBibliography>
    {
        let limits = SnapshotLimits { max_files: requested.max_files.min(FILES),
            max_file_bytes: requested.max_file_bytes.min(MAX_FILE_BYTES),
            max_total_bytes: requested.max_total_bytes.min(BYTES) };
        if limits.max_files == 0 || limits.max_file_bytes == 0 || limits.max_total_bytes == 0 {
            return Err(CoreError::new("analysis_quota", "Bibliography analysis requires a nonzero bounded capture"));
        }
        let snapshot = workspace.analysis_snapshot_with_limits(&limits)
            .map_err(capture_error)?;
        let mut targets = Vec::new();
        for file in &snapshot.files {
            if !Path::new(&file.path).extension().is_some_and(|e|e.eq_ignore_ascii_case("bib")) { continue; }
            let source = file.text().map_err(|_| CoreError::new("analysis_incomplete", "Invalid owner analysis text"))?;
            let (citations, incomplete) = canonical_index::citations(source, SYMBOLS - targets.len());
            if incomplete {
                return Err(CoreError::new("bibliography_incomplete", "Canonical key projection or symbol budget is incomplete"));
            }
            // Existing index policy: the last declaration per file wins. Keep
            // each winner's accepted header span; across files preserve alternatives.
            let mut seen = BTreeSet::new(); let mut winners = BTreeSet::new();
            for (index, citation) in citations.iter().enumerate().rev() {
                if seen.insert(citation.key.clone()) { winners.insert(index); }
            }
            let mut coordinates = Coordinates::default();
            for (index, citation) in citations.into_iter().enumerate() {
                if !winners.contains(&index) { continue; }
                coordinates.advance(source, citation.key_bytes.start);
                let length = citation.key.encode_utf16().count() as u32;
                targets.push(BibliographyTarget { key: citation.key, entry_type: citation.entry_type,
                    anchor: SourceAnchor { document_id: file.path.clone(), revision: file.revision,
                        content_sha256: file.content_sha256.clone(), line: coordinates.line,
                        character: coordinates.character,
                        range: Utf16Range { start: coordinates.utf16, length } }, dirty: file.dirty });
            }
        }
        // Catch disk or owner changes during parsing before handing out derived data.
        workspace.revalidate_analysis_snapshot(&snapshot)?;
        Ok(WorkspaceBibliography { snapshot, targets })
    }
}

impl WorkspaceBibliography {
    /// Rename needs every accepted declaration, including duplicates hidden by
    /// completion's last-per-file winner policy. Keep that existing policy intact.
    pub(super) fn rename_declarations(&self) -> Result<Vec<BibliographyTarget>> {
        let mut declarations = Vec::new();
        for file in &self.snapshot.files {
            if !Path::new(&file.path).extension().is_some_and(|e|e.eq_ignore_ascii_case("bib")) { continue; }
            let source = file.text().map_err(|_| CoreError::new("analysis_incomplete", "Invalid owner analysis text"))?;
            let (citations, incomplete) = canonical_index::citations(source, SYMBOLS - declarations.len());
            if incomplete {
                return Err(CoreError::new("bibliography_incomplete", "Complete declaration projection is required for rename"));
            }
            let mut coordinates = Coordinates::default();
            for citation in citations {
                coordinates.advance(source, citation.key_bytes.start);
                let length = citation.key.encode_utf16().count() as u32;
                declarations.push(BibliographyTarget { key: citation.key, entry_type: citation.entry_type,
                    anchor: SourceAnchor { document_id: file.path.clone(), revision: file.revision,
                        content_sha256: file.content_sha256.clone(), line: coordinates.line,
                        character: coordinates.character,
                        range: Utf16Range { start: coordinates.utf16, length } }, dirty: file.dirty });
            }
        }
        Ok(declarations)
    }

    /// Immutable captured data, not a statement that the external disk is current.
    pub fn snapshot(&self) -> &WorkspaceSnapshot { &self.snapshot }

    pub fn revalidate(&self, workspace: &mut Workspace) -> Result<()> {
        workspace.revalidate_analysis_snapshot(&self.snapshot)
    }

    // Narrow completion delegation over these same canonical targets. Keep the
    // existing resolver and last-per-file/open/lexical precedence unchanged.
    pub(super) fn completion_target(&self, workspace: &mut Workspace, document_id: &str, key: &str)
        -> Result<Option<BibliographyTarget>>
    {
        self.revalidate(workspace)?;
        let opened: BTreeSet<_> = workspace.document_infos().into_iter().map(|info| info.id).collect();
        let priority = |path: &str| if path == document_id { 0 } else if opened.contains(path) { 1 } else { 2 };
        Ok(self.targets.iter().filter(|target| target.key == key).min_by(|a,b|
            priority(&a.anchor.document_id).cmp(&priority(&b.anchor.document_id))
                .then_with(|| a.anchor.document_id.cmp(&b.anchor.document_id))).cloned())
    }

    pub fn entries<'a>(&'a self, workspace: &mut Workspace) -> Result<&'a [BibliographyTarget]> {
        self.revalidate(workspace)?;
        Ok(&self.targets)
    }

    pub fn resolve_citation(&self, workspace: &mut Workspace, document_id: &str, caret_utf16: u32)
        -> Result<CitationLookup>
    {
        self.revalidate(workspace)?;
        let file = self.snapshot.files.iter().find(|file| file.path == document_id)
            .ok_or_else(|| CoreError::new("bibliography_source_missing", "Citation source is outside the captured analysis set"))?;
        let source = file.text().map_err(|_| CoreError::new("analysis_incomplete", "Invalid owner analysis text"))?;
        let context = crate::definition::context(source, caret_utf16)?;
        if context.kind != Some(CompletionKind::Citation) { return Ok(CitationLookup::Unsupported); }
        let Some(key) = context.symbol.filter(|key| crate::tex_scan::valid_key(key)) else {
            return Ok(CitationLookup::Unsupported);
        };
        let opened: BTreeSet<_> = workspace.document_infos().into_iter().map(|info| info.id).collect();
        let priority = |path: &str| if path == document_id { 0 } else if opened.contains(path) { 1 } else { 2 };
        let mut matches: Vec<_> = self.targets.iter().filter(|target| target.key == key).collect();
        matches.sort_by(|a,b| priority(&a.anchor.document_id).cmp(&priority(&b.anchor.document_id))
            .then_with(|| a.anchor.document_id.cmp(&b.anchor.document_id)));
        Ok(match matches.first() {
            Some(target) => CitationLookup::Resolved { target: (**target).clone(), alternatives: matches.len() as u32 },
            None => CitationLookup::Missing { key },
        })
    }
}
