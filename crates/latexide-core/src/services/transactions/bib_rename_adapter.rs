//! Explicit consumption of a canonical immutable P12 rename preview.
//! One existing Workspace owns all editable buffers and history. This module
//! introduces no wire endpoint, Save operation, parser or document store.
use super::{TransactionService, WorkspaceTransaction, WorkspaceTransactionError};
use crate::{CoreError, DocumentInfo, Position, Result, TextEdit, Workspace, MAX_FILE_BYTES, MAX_OPEN_DOCUMENTS};
use crate::contracts::{RevisionedEdit, SnapshotFile, SnapshotLimits, SourceAnchor};
use crate::services::bibliography::key_rename::{BibKeyRenamePreview, KeyRenameError, RenameSiteKind};
use std::collections::BTreeMap;

const MAX_EDITS: usize = 1000;

fn service() -> TransactionService {
    TransactionService::new(SnapshotLimits { max_files: MAX_OPEN_DOCUMENTS,
        max_file_bytes: MAX_FILE_BYTES, max_total_bytes: 8 * MAX_FILE_BYTES })
}
fn unsupported() -> CoreError {
    CoreError::new("bib_rename_target_not_open", "Rename requires every target to be an already open retained buffer")
}
fn invalid() -> CoreError {
    CoreError::new("bib_rename_anchor_mismatch", "Rename anchor, digest, coordinates or expected old text differ")
}
fn preview_error(error: KeyRenameError) -> CoreError {
    let code=match &error {
        KeyRenameError::StaleCapture => "bib_rename_stale_capture",
        KeyRenameError::OutsideRoot => "bib_rename_outside_root",
        KeyRenameError::Quota => "bib_rename_quota",
        _ => "bib_rename_preview_refused",
    };
    CoreError::new(code,format!("Immutable rename preview refused: {error:?}"))
}
fn transaction_error(error: WorkspaceTransactionError) -> CoreError {
    match error {
        WorkspaceTransactionError::Workspace(error) => error,
        WorkspaceTransactionError::Transaction(error) => {
            let code=if matches!(&error,crate::contracts::transactions::TransactionError::Quota) {
                "bib_rename_quota"
            } else { "bib_rename_transaction_refused" };
            CoreError::new(code,format!("Rename transaction refused: {error:?}"))
        }
    }
}

/// A monotone scalar-safe cursor resolves all sorted endpoints once per file.
struct Endpoints<'a> {
    characters: std::str::CharIndices<'a>,
    byte: usize,
    units: u32,
    line: u32,
    column: u32,
}
impl<'a> Endpoints<'a> {
    fn new(text: &'a str) -> Self {
        Self { characters: text.char_indices(), byte: 0, units: 0, line: 0, column: 0 }
    }
    fn at(&mut self, target: u32) -> Result<(usize, Position)> {
        if target < self.units { return Err(invalid()); }
        while self.units < target {
            let (byte, ch) = self.characters.next().ok_or_else(invalid)?;
            self.units = self.units.checked_add(ch.len_utf16() as u32).ok_or_else(invalid)?;
            self.byte = byte + ch.len_utf8();
            if ch == '\n' {
                self.line = self.line.checked_add(1).ok_or_else(invalid)?; self.column = 0;
            } else {
                self.column = self.column.checked_add(ch.len_utf16() as u32).ok_or_else(invalid)?;
            }
        }
        if self.units != target { return Err(invalid()); }
        Ok((self.byte, Position { line: self.line, character: self.column }))
    }
}
#[derive(Clone, Copy)]
struct ResolvedAnchor { first: usize, last: usize, start: Position, finish: Position }
fn check_anchor(file: &SnapshotFile, text: &str, source: &SourceAnchor, expected: &str,
    cursor: &mut Endpoints<'_>) -> Result<ResolvedAnchor>
{
    if source.revision.is_none() { return Err(unsupported()); }
    if expected.is_empty() || file.path!=source.document_id || file.revision!=source.revision
        || file.content_sha256!=source.content_sha256
        || u32::try_from(expected.encode_utf16().count()).map_err(|_|invalid())?!=source.range.length
    { return Err(invalid()); }
    let end=source.range.start.checked_add(source.range.length).ok_or_else(invalid)?;
    let(first,start)=cursor.at(source.range.start)?;
    let(last,finish)=cursor.at(end)?;
    if start.line!=source.line || start.character!=source.character || text.get(first..last)!=Some(expected) {
        return Err(invalid());
    }
    Ok(ResolvedAnchor { first, last, start, finish })
}

/// Read-only validation of a review description against the actual owner.
/// This produces no token or mutation authority. The constructor below accepts
/// only the original immutable preview, never caller-edited descriptions.
pub fn validate_rename_anchor(workspace: &Workspace, source: &SourceAnchor, expected: &str) -> Result<()> {
    let capture=workspace.transaction_snapshot()?;
    let file=capture.files.iter().find(|f|f.path==source.document_id).ok_or_else(unsupported)?;
    let text=file.text().map_err(|_|invalid())?;
    check_anchor(file,text,source,expected,&mut Endpoints::new(text)).map(|_|())
}

fn checked_edits(preview: &BibKeyRenamePreview<'_>) -> Result<Vec<RevisionedEdit>> {
    if preview.edits().is_empty() || preview.edits().len()>MAX_EDITS {
        return Err(CoreError::new("bib_rename_quota","Rename exceeds the existing atomic group edit budget"));
    }
    if preview.edits().iter().filter(|e|e.kind==RenameSiteKind::Declaration).count()!=1 {
        return Err(invalid());
    }
    preview.edits().iter().map(|edit| {
        if edit.expected_old_text!=preview.old_key() || edit.insertion!=preview.new_key() { return Err(invalid()); }
        Ok(RevisionedEdit { document_id:edit.source.document_id.clone(),
            expected_revision:edit.source.revision.ok_or_else(unsupported)?,
            start_utf16:edit.source.range.start,length_utf16:edit.source.range.length,
            replacement:edit.insertion.clone() })
    }).collect()
}
fn validate_pair(transaction: &WorkspaceTransaction, preview: &BibKeyRenamePreview<'_>)
    -> Result<Vec<ResolvedAnchor>>
{
    let expected=checked_edits(preview)?;
    if expected.len()!=transaction.plan().edits.len() { return Err(invalid()); }
    let mut by_file:BTreeMap<&str,Vec<usize>>=BTreeMap::new();
    for (index, actual) in transaction.plan().edits.iter().enumerate() {
        by_file.entry(&actual.document_id).or_default().push(index);
    }
    let mut resolved=vec![None;expected.len()];
    for (id, mut indices) in by_file {
        let file=transaction.snapshot().files.iter().find(|f|f.path==id).ok_or_else(unsupported)?;
        let text=file.text().map_err(|_|invalid())?;
        indices.sort_unstable_by_key(|index|transaction.plan().edits[*index].start_utf16);
        let mut cursor=Endpoints::new(text);
        for index in indices {
            let site=&preview.edits()[index];let wanted=&expected[index];let actual=&transaction.plan().edits[index];
            if actual.document_id!=wanted.document_id || actual.expected_revision!=wanted.expected_revision
                || actual.start_utf16!=wanted.start_utf16 || actual.length_utf16!=wanted.length_utf16
                || actual.replacement!=wanted.replacement { return Err(invalid()); }
            resolved[index]=Some(check_anchor(file,text,&site.source,&site.expected_old_text,&mut cursor)?);
        }
    }
    resolved.into_iter().map(|row|row.ok_or_else(invalid)).collect()
}
fn batches(transaction: &WorkspaceTransaction, positions: &[ResolvedAnchor]) -> Result<Vec<(String,u64,Vec<TextEdit>)>> {
    if positions.len()!=transaction.plan().edits.len() { return Err(invalid()); }
    let mut grouped:BTreeMap<String,(u64,Vec<TextEdit>)>=BTreeMap::new();
    for (edit, position) in transaction.plan().edits.iter().zip(positions) {
        let file=transaction.snapshot().files.iter().find(|f|f.path==edit.document_id).ok_or_else(unsupported)?;
        let revision=file.revision.ok_or_else(unsupported)?;
        if revision!=edit.expected_revision { return Err(invalid()); }
        let entry=grouped.entry(edit.document_id.clone()).or_insert_with(||(revision,Vec::new()));
        if entry.0!=revision { return Err(invalid()); }
        entry.1.push(TextEdit{start:position.start,end:position.finish,text:edit.replacement.clone()});
    }
    Ok(grouped.into_iter().map(|(id,(revision,edits))|(id,revision,edits)).collect())
}
fn preflight_encoding(workspace: &Workspace, transaction: &WorkspaceTransaction, positions: &[ResolvedAnchor]) -> Result<()> {
    if positions.len()!=transaction.plan().edits.len() { return Err(invalid()); }
    let mut grouped:BTreeMap<&str,Vec<usize>>=BTreeMap::new();
    for (index, edit) in transaction.plan().edits.iter().enumerate() {
        grouped.entry(&edit.document_id).or_default().push(index);
    }
    for (id, mut indices) in grouped {
        let file=transaction.snapshot().files.iter().find(|f|f.path==id).ok_or_else(unsupported)?;
        let live=workspace.snapshot(id)?;
        if Some(live.revision)!=file.revision || live.text.as_bytes()!=file.utf8.as_slice() { return Err(invalid()); }
        indices.sort_unstable_by_key(|index|std::cmp::Reverse(transaction.plan().edits[*index].start_utf16));
        let mut projected=live.text;
        for index in indices {
            let edit=&transaction.plan().edits[index];let position=&positions[index];
            projected.replace_range(position.first..position.last,&edit.replacement);
        }
        // Use the retained document's normal BOM/CRLF encoder, not a logical
        // UTF8-size estimate. The actual apply path repeats the same guard.
        crate::document::validate_encoded_size(&projected,live.line_ending,live.bom)?;
    }
    Ok(())
}

/// Private immutable preview/plan pairing; never retains a Workspace borrow.
/// Every explicit commit attempt consumes this local token, including refusal.
#[derive(Debug)]
pub struct PreparedBibRename<'a> {
    preview: BibKeyRenamePreview<'a>,
    transaction: WorkspaceTransaction,
    consumed: bool,
}
impl PreparedBibRename<'_> {
    pub fn is_consumed(&self) -> bool { self.consumed }

    /// Full owner/capture/plan/anchor validation and one prevalidated group edit
    /// share the caller's exclusive Workspace borrow. Filesystem observation is
    /// sequential; this does not claim hostile-race isolation or multi-file Save.
    pub fn commit(&mut self, workspace: &mut Workspace, origin: &str) -> Result<Vec<DocumentInfo>> {
        if self.consumed { return Err(CoreError::new("bib_rename_consumed","Rename token was already consumed")); }
        self.consumed=true;
        service().revalidate_workspace_transaction(workspace,&self.transaction).map_err(transaction_error)?;
        self.preview.revalidate(workspace).map_err(preview_error)?;
        let positions=validate_pair(&self.transaction,&self.preview)?;
        let batches=batches(&self.transaction,&positions)?;
        workspace.apply_edit_group(batches,origin)
    }
}

pub fn prepare_bib_rename<'a>(workspace: &mut Workspace, preview: BibKeyRenamePreview<'a>) -> Result<PreparedBibRename<'a>> {
    preview.revalidate(workspace).map_err(preview_error)?;
    let edits=checked_edits(&preview)?;
    let transaction=service().prepare_workspace_transaction(workspace,edits).map_err(transaction_error)?;
    let positions=validate_pair(&transaction,&preview)?;
    preflight_encoding(workspace,&transaction,&positions)?;
    preview.revalidate(workspace).map_err(preview_error)?;
    Ok(PreparedBibRename{preview,transaction,consumed:false})
}
