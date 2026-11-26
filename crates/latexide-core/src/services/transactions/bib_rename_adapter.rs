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

/// Resolve flat logical UTF16 only at complete scalar boundaries.
fn endpoint(text: &str, target: u32) -> Result<(usize,Position)> {
    let(mut units,mut line,mut column)=(0u32,0u32,0u32);
    for(byte,ch) in text.char_indices() {
        if units==target { return Ok((byte,Position{line,character:column})); }
        if units>target { return Err(invalid()); }
        units=units.checked_add(ch.len_utf16() as u32).ok_or_else(invalid)?;
        if ch=='\n' {
            line=line.checked_add(1).ok_or_else(invalid)?;column=0;
        } else { column=column.checked_add(ch.len_utf16() as u32).ok_or_else(invalid)?; }
    }
    if units==target { Ok((text.len(),Position{line,character:column})) } else { Err(invalid()) }
}
fn check_anchor(file: &SnapshotFile, source: &SourceAnchor, expected: &str) -> Result<(Position,Position)> {
    if source.revision.is_none() { return Err(unsupported()); }
    if expected.is_empty() || file.path!=source.document_id || file.revision!=source.revision
        || file.content_sha256!=source.content_sha256
        || u32::try_from(expected.encode_utf16().count()).map_err(|_|invalid())?!=source.range.length
    { return Err(invalid()); }
    let text=file.text().map_err(|_|invalid())?;
    let end=source.range.start.checked_add(source.range.length).ok_or_else(invalid)?;
    let(first,start)=endpoint(text,source.range.start)?;
    let(last,finish)=endpoint(text,end)?;
    if start.line!=source.line || start.character!=source.character || &text[first..last]!=expected {
        return Err(invalid());
    }
    Ok((start,finish))
}

/// Read-only validation of a review description against the actual owner.
/// This produces no token or mutation authority. The constructor below accepts
/// only the original immutable preview, never caller-edited descriptions.
pub fn validate_rename_anchor(workspace: &Workspace, source: &SourceAnchor, expected: &str) -> Result<()> {
    let capture=workspace.transaction_snapshot()?;
    let file=capture.files.iter().find(|f|f.path==source.document_id).ok_or_else(unsupported)?;
    check_anchor(file,source,expected).map(|_|())
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
fn validate_pair(transaction: &WorkspaceTransaction, preview: &BibKeyRenamePreview<'_>) -> Result<()> {
    let expected=checked_edits(preview)?;
    if expected.len()!=transaction.plan().edits.len() { return Err(invalid()); }
    for((site,wanted),actual) in preview.edits().iter().zip(expected.iter()).zip(transaction.plan().edits.iter()) {
        if actual.document_id!=wanted.document_id || actual.expected_revision!=wanted.expected_revision
            || actual.start_utf16!=wanted.start_utf16 || actual.length_utf16!=wanted.length_utf16
            || actual.replacement!=wanted.replacement { return Err(invalid()); }
        let file=transaction.snapshot().files.iter().find(|f|f.path==site.source.document_id).ok_or_else(unsupported)?;
        check_anchor(file,&site.source,&site.expected_old_text)?;
    }
    Ok(())
}
fn batches(transaction: &WorkspaceTransaction) -> Result<Vec<(String,u64,Vec<TextEdit>)>> {
    let mut grouped:BTreeMap<String,(u64,Vec<TextEdit>)>=BTreeMap::new();
    for edit in &transaction.plan().edits {
        let file=transaction.snapshot().files.iter().find(|f|f.path==edit.document_id).ok_or_else(unsupported)?;
        let text=file.text().map_err(|_|invalid())?;
        let end=edit.start_utf16.checked_add(edit.length_utf16).ok_or_else(invalid)?;
        let(_,start)=endpoint(text,edit.start_utf16)?;let(_,finish)=endpoint(text,end)?;
        let revision=file.revision.ok_or_else(unsupported)?;
        let entry=grouped.entry(edit.document_id.clone()).or_insert_with(||(revision,Vec::new()));
        if entry.0!=revision { return Err(invalid()); }
        entry.1.push(TextEdit{start,end:finish,text:edit.replacement.clone()});
    }
    Ok(grouped.into_iter().map(|(id,(revision,edits))|(id,revision,edits)).collect())
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
        validate_pair(&self.transaction,&self.preview)?;
        let batches=batches(&self.transaction)?;
        workspace.apply_edit_group(batches,origin)
    }
}

pub fn prepare_bib_rename<'a>(workspace: &mut Workspace, preview: BibKeyRenamePreview<'a>) -> Result<PreparedBibRename<'a>> {
    preview.revalidate(workspace).map_err(preview_error)?;
    let edits=checked_edits(&preview)?;
    let transaction=service().prepare_workspace_transaction(workspace,edits).map_err(transaction_error)?;
    validate_pair(&transaction,&preview)?;
    preview.revalidate(workspace).map_err(preview_error)?;
    Ok(PreparedBibRename{preview,transaction,consumed:false})
}
