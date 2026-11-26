//! Cite-token proposals and explicit, consuming application to their owner.
//! Application validates and edits under one mutable Workspace borrow. Save and
//! native acceptance remain separate; disk revalidation is sequential observation.
use super::{LocalBibliographyProvider, workspace_adapter::{BibliographyTarget, WorkspaceBibliography}};
use crate::{CoreError, Result, Workspace, DocumentInfo, Position, TextEdit,
    completion::{self, CompletionKind, Mode},
    contracts::{SourceAnchor, Utf16Range}, tex_scan::{self, Token, Tokens}};

#[derive(Debug, Clone, PartialEq)]
pub struct CiteCompletionEdit {
    pub source: SourceAnchor,
    pub expected_old_token: String,
    pub insertion: String,
    /// Absolute scalar-safe UTF16 selection after the proposed replacement.
    pub selection_after: Utf16Range,
    pub bibliography: BibliographyTarget,
}

#[derive(Debug)]
pub struct CiteCompletionPlan {
    capture: WorkspaceBibliography,
    edit: CiteCompletionEdit,
    consumed: bool,
}

#[derive(Debug)]
pub struct CiteCompletionApplied {
    pub document: DocumentInfo,
    pub selection_after: Utf16Range,
}

#[derive(Debug)]
pub enum CiteCompletion {
    Planned(CiteCompletionPlan),
    Unsupported,
    NoCandidate,
}

// Core completion owns the exact token range. This guard narrows its supported
// syntax to a closed static argument: it never encodes/repairs TeX delimiters.
fn static_argument(source: &str, caret: usize) -> bool {
    for token in Tokens::new(source) {
        let Token::Command(command) = token else { continue; };
        if command.start > caret { break; }
        if !matches!(command.name, "cite" | "citep" | "citet" | "Citep" | "Citet" |
            "autocite" | "parencite" | "textcite" | "footcite" | "smartcite" | "supercite" | "nocite") { continue; }
        let mut position = tex_scan::skip_space(source, command.end);
        if source.as_bytes().get(position) == Some(&b'*') { position = tex_scan::skip_space(source, position + 1); }
        let mut literal_options = true;
        while let Some(group) = tex_scan::group(source, position, b'[', b']').filter(|g|g.closed) {
            literal_options &= !source[group.content.clone()].chars().any(|c| matches!(c,'\\'|'{'|'}'|'$'|'#'|'%'|'&'));
            position = group.end;
        }
        let Some(group) = tex_scan::group(source, position, b'{', b'}') else { continue; };
        if group.closed && group.content.start <= caret && caret <= group.content.end {
            return literal_options && source[group.content].split(',').all(|item| {
                let key = item.trim(); key.is_empty() || tex_scan::valid_key(key)
            });
        }
    }
    false
}

impl LocalBibliographyProvider {
    /// A requested label must be an actual captured canonical key. The source
    /// must already be open. Planning never edits; explicit apply revalidates it.
    /// The production capture requires a text-only project root; unrelated
    /// binary/non-text assets refuse the whole operation with a diagnostic.
    pub fn plan_citation_completion(&self, workspace: &mut Workspace, document_id: &str,
        expected_revision: u64, caret_utf16: u32, candidate: &str) -> Result<CiteCompletion>
    {
        let live = workspace.snapshot(document_id)?;
        if live.revision != expected_revision {
            return Err(CoreError::new("revision_conflict", "Citation source revision changed"));
        }
        let context = completion::context(&live.text, caret_utf16)?;
        let Mode::Key { kind: CompletionKind::Citation, closing: false, used } = &context.mode else {
            return Ok(CiteCompletion::Unsupported);
        };
        let caret = completion::byte_at(&live.text, caret_utf16)?;
        if !static_argument(&live.text, caret) { return Ok(CiteCompletion::Unsupported); }
        if !tex_scan::valid_key(candidate) || !candidate.starts_with(&context.set.prefix) || used.contains(candidate) {
            return Ok(CiteCompletion::NoCandidate);
        }
        let capture = self.inspect_workspace(workspace)?;
        let Some(target) = capture.completion_target(workspace, document_id, candidate)? else {
            return Ok(CiteCompletion::NoCandidate);
        };
        let file = capture.snapshot().files.iter().find(|file|file.path == document_id)
            .ok_or_else(|| CoreError::new("stale_cite_source", "Open source is missing from analysis capture"))?;
        if file.revision != Some(expected_revision) || file.utf8.as_slice() != live.text.as_bytes() {
            return Err(CoreError::new("stale_cite_source", "Owner source and analysis bytes differ"));
        }
        let range = Utf16Range { start: context.set.replace_start_utf16, length: context.set.replace_length_utf16 };
        let end = range.start.checked_add(range.length)
            .ok_or_else(|| CoreError::new("invalid_position", "Citation token range overflow"))?;
        let first = completion::byte_at(&live.text, range.start)?;
        let last = completion::byte_at(&live.text, end)?;
        let prefix = &live.text[..first];
        let source = SourceAnchor { document_id: file.path.clone(), revision: file.revision,
            content_sha256: file.content_sha256.clone(),
            line: prefix.bytes().filter(|b|*b == b'\n').count() as u32,
            character: prefix.rsplit('\n').next().unwrap_or("").encode_utf16().count() as u32, range };
        // The insertion is copied from the canonical result, never encoded from
        // client provenance or expanded from a dynamic TeX argument.
        let insertion = target.key.clone();
        let selection = source.range.start + insertion.encode_utf16().count() as u32;
        let edit = CiteCompletionEdit { source, expected_old_token: live.text[first..last].into(),
            insertion, selection_after: Utf16Range { start: selection, length: 0 }, bibliography: target };
        let plan = CiteCompletionPlan { capture, edit, consumed: false };
        plan.validate(workspace)?;
        Ok(CiteCompletion::Planned(plan))
    }
}

impl CiteCompletionPlan {
    /// Captured data for review; this reference does not authorize a later edit.
    pub fn description(&self) -> &CiteCompletionEdit { &self.edit }

    pub fn is_consumed(&self) -> bool { self.consumed }

    /// Every explicit attempt consumes the plan, including a refused attempt.
    /// The final Workspace edit retains its normal quota, history and no-op rules.
    /// No Save, new Runtime request, or native completion event is introduced.
    pub fn apply(&mut self, workspace: &mut Workspace) -> Result<CiteCompletionApplied> {
        if self.consumed {
            return Err(CoreError::new("cite_plan_consumed", "Citation plan was already consumed"));
        }
        self.consumed = true;
        let live = workspace.snapshot(&self.edit.source.document_id)?;
        let range = self.edit.source.range.clone();
        let end = range.start.checked_add(range.length)
            .ok_or_else(|| CoreError::new("invalid_position", "Citation token range overflow"))?;
        let first = completion::byte_at(&live.text, range.start)?;
        let last = completion::byte_at(&live.text, end)?;
        if &live.text[first..last] != self.edit.expected_old_token.as_str() {
            return Err(CoreError::new("stale_cite_token", "Expected citation token changed"));
        }
        let context = completion::context(&live.text, end)?;
        let Mode::Key { kind: CompletionKind::Citation, closing: false, used } = &context.mode else {
            return Err(CoreError::new("stale_cite_source", "Citation context changed"));
        };
        if context.set.replace_start_utf16 != range.start
            || context.set.replace_length_utf16 != range.length
            || !static_argument(&live.text, last)
            || used.contains(&self.edit.insertion)
        {
            return Err(CoreError::new("stale_cite_source", "Citation token context changed"));
        }
        // One final full observation revalidates the original capture and its
        // immutable canonical projection. Winner selection then uses that same
        // bound projection and current open-document priority under this borrow.
        self.validate_source(workspace)?;
        let target = self.capture.completion_target(workspace,
            &self.edit.source.document_id, &self.edit.insertion)?;
        if target.as_ref() != Some(&self.edit.bibliography) {
            return Err(CoreError::new("stale_cite_target", "Canonical citation target changed"));
        }
        let revision = self.edit.source.revision.ok_or_else(||
            CoreError::new("document_not_open", "Citation source requires an open owner revision"))?;
        let character_end = self.edit.source.character.checked_add(range.length)
            .ok_or_else(|| CoreError::new("invalid_position", "Citation column overflow"))?;
        let document = workspace.apply_edits(&self.edit.source.document_id, revision, vec![TextEdit {
            start: Position { line: self.edit.source.line, character: self.edit.source.character },
            end: Position { line: self.edit.source.line, character: character_end },
            text: self.edit.insertion.clone(),
        }], "user")?;
        Ok(CiteCompletionApplied { document, selection_after: self.edit.selection_after.clone() })
    }

    fn validate_source(&self, workspace: &Workspace) -> Result<()> {
        let live = workspace.snapshot(&self.edit.source.document_id)?;
        if Some(live.revision) != self.edit.source.revision {
            return Err(CoreError::new("revision_conflict", "Citation source revision changed"));
        }
        let file = self.capture.snapshot().files.iter().find(|file|file.path == self.edit.source.document_id)
            .ok_or_else(|| CoreError::new("stale_cite_source", "Captured source disappeared"))?;
        // Exact owner bytes bind the digest; there is no invented encoder or
        // caller-supplied digest. Preserve typed whole-capture refusal below.
        if file.revision != self.edit.source.revision || file.content_sha256 != self.edit.source.content_sha256
            || file.utf8.as_slice() != live.text.as_bytes()
        { return Err(CoreError::new("stale_cite_source", "Citation source digest/bytes changed")); }
        let end = self.edit.source.range.start.checked_add(self.edit.source.range.length)
            .ok_or_else(|| CoreError::new("invalid_position", "Citation token range overflow"))?;
        let first = completion::byte_at(&live.text, self.edit.source.range.start)?;
        let last = completion::byte_at(&live.text, end)?;
        if &live.text[first..last] != self.edit.expected_old_token.as_str() {
            return Err(CoreError::new("stale_cite_token", "Expected citation token changed"));
        }
        Ok(())
    }

    fn validate(&self, workspace: &mut Workspace) -> Result<()> {
        self.validate_source(workspace)?;
        self.capture.revalidate(workspace)
    }

    /// Return data only after full capture/revision/digest/old-token validation.
    /// This observation does not reserve an edit; use apply for local consumption.
    /// A future Runtime/native consumer still requires its own acceptance contract.
    pub fn validated_edit(&self, workspace: &mut Workspace) -> Result<CiteCompletionEdit> {
        if self.consumed {
            return Err(CoreError::new("cite_plan_consumed", "Citation plan was already consumed"));
        }
        self.validate(workspace)?;
        Ok(self.edit.clone())
    }
}
