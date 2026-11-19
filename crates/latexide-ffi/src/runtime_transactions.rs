//! Internal preparation data over the native/Session Runtime allocation.
//!
//! Trusted Rust callers supply the existing Arc. Weak ownership is not Session
//! authentication. Preparation returns owned immutable input, never a guard.
//! Legacy commit consumes input for one document. commit_multi consumes it for
//! one prevalidated Workspace memory/history group under the same owner guard.
//! Document Save and Session endpoints remain separate.
use super::Runtime;
use latexide_core::{CoreError, DocumentInfo, Position, Result, TextEdit, MAX_FILE_BYTES};
use latexide_core::contracts::{RevisionedEdit, SnapshotLimits, WorkspaceSnapshot};
use latexide_core::contracts::transactions::PreparedTransaction;
use latexide_core::services::transactions::{TransactionService, WorkspaceTransaction, WorkspaceTransactionError};
use std::{collections::{BTreeMap, BTreeSet}, sync::{Arc, Mutex, MutexGuard, TryLockError, Weak}};

#[derive(Clone)]
pub(crate) struct RuntimeTransactionGate { owner: Weak<Mutex<Runtime>> }

/// No public constructor, deserializer, mutable plan or retained owner/guard.
pub(crate) struct RuntimeBoundTransaction {
    owner: Weak<Mutex<Runtime>>,
    transaction: WorkspaceTransaction,
    // Copy of the existing plan's correlation label; never a new auth token.
    plan_id: String,
}

/// Internal Rust result only; no new v1 wire/CABI shape or retained owner/guard.
pub(crate) struct RuntimeMultiCommitResult {
    pub(crate) documents: Vec<DocumentInfo>,
}

impl RuntimeBoundTransaction {
    pub(crate) fn plan(&self) -> &PreparedTransaction { self.transaction.plan() }
    pub(crate) fn snapshot(&self) -> &WorkspaceSnapshot { self.transaction.snapshot() }
}

fn service() -> TransactionService {
    TransactionService::new(SnapshotLimits {max_files: 128,
        max_file_bytes: MAX_FILE_BYTES, max_total_bytes: 8 * MAX_FILE_BYTES})
}

fn unavailable() -> CoreError {
    CoreError::new("runtime_unavailable", "Transaction Runtime owner is gone or poisoned")
}

fn try_runtime(owner: &Arc<Mutex<Runtime>>) -> Result<MutexGuard<'_, Runtime>> {
    owner.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => CoreError::new("runtime_busy", "Transaction Runtime is busy"),
        TryLockError::Poisoned(_) => unavailable(),
    })
}

fn committed(runtime: &Runtime) -> Result<()> {
    if runtime.composition.is_some() {
        Err(CoreError::new("composition_active", "Transaction planning waits for native composition"))
    } else { Ok(()) }
}

fn workspace_error(error: WorkspaceTransactionError) -> CoreError {
    match error {
        WorkspaceTransactionError::Workspace(error) => error,
        WorkspaceTransactionError::Transaction(error) => CoreError::new("transaction_refused", format!("{error:?}")),
    }
}

/// The caller already holds this Runtime's guard; never acquire a second lock.
fn validate_locked(runtime: &Runtime, bound: &RuntimeBoundTransaction) -> Result<()> {
    committed(runtime)?;
    if bound.plan_id != bound.plan().transaction_id {
        return Err(CoreError::new("runtime_transaction_plan_mismatch", "Transaction plan pairing differs"));
    }
    service().revalidate_workspace_transaction(&runtime.workspace, &bound.transaction)
        .map_err(workspace_error)
}

fn invalid_position() -> CoreError {
    CoreError::new("invalid_position", "Transaction offset is not a UTF-16 scalar boundary")
}

/// Map only requested endpoints, once over normalized logical text. FEFF is
/// ordinary content; LF consumes a flat UTF-16 unit and advances the line.
fn position_map(text: &str, endpoints: &BTreeSet<u32>) -> Result<BTreeMap<u32, Position>> {
    let mut wanted = endpoints.iter().copied().peekable();
    let mut mapped = BTreeMap::new();
    let (mut offset, mut line, mut column) = (0u32, 0u32, 0u32);
    for character in text.chars() {
        if wanted.peek() == Some(&offset) {
            mapped.insert(offset, Position {line, character: column});
            wanted.next();
        }
        if wanted.peek().is_some_and(|target| *target < offset) { return Err(invalid_position()); }
        if wanted.peek().is_none() { return Ok(mapped); }
        let width = character.len_utf16() as u32;
        offset = offset.checked_add(width).ok_or_else(invalid_position)?;
        if character == '\n' {
            line = line.checked_add(1).ok_or_else(invalid_position)?;
            column = 0;
        } else {
            column = column.checked_add(width).ok_or_else(invalid_position)?;
        }
    }
    if wanted.peek() == Some(&offset) {
        mapped.insert(offset, Position {line, character: column});
        wanted.next();
    }
    if wanted.peek().is_some() { Err(invalid_position()) } else { Ok(mapped) }
}

fn single_document_required() -> CoreError {
    CoreError::new("transaction_single_document_required", "Commit requires edits for exactly one document")
}

fn single_document_batch(bound: &RuntimeBoundTransaction) -> Result<(String, u64, Vec<TextEdit>)> {
    let edits = &bound.plan().edits;
    let first = edits.first().ok_or_else(single_document_required)?;
    // Reject the whole plan before any Workspace mutation. Snapshot membership
    // can contain multiple open buffers; only edit document IDs are restricted.
    if edits.iter().any(|edit| edit.document_id != first.document_id) {
        return Err(single_document_required());
    }
    let file = bound.snapshot().files.iter().find(|file| file.path == first.document_id)
        .ok_or_else(invalid_position)?;
    let revision = file.revision.ok_or_else(invalid_position)?;
    let text = file.text().map_err(|_| invalid_position())?;
    let mut endpoints = BTreeSet::new();
    for edit in edits {
        endpoints.insert(edit.start_utf16);
        endpoints.insert(edit.start_utf16.checked_add(edit.length_utf16).ok_or_else(invalid_position)?);
    }
    let positions = position_map(text, &endpoints)?;
    let mut batch = Vec::with_capacity(edits.len());
    for edit in edits {
        let end = edit.start_utf16.checked_add(edit.length_utf16).ok_or_else(invalid_position)?;
        batch.push(TextEdit {
            start: *positions.get(&edit.start_utf16).ok_or_else(invalid_position)?,
            end: *positions.get(&end).ok_or_else(invalid_position)?,
            text: edit.replacement.clone(),
        });
    }
    Ok((first.document_id.clone(), revision, batch))
}

fn multi_document_batches(bound: &RuntimeBoundTransaction) -> Result<Vec<(String, u64, Vec<TextEdit>)>> {
    let mut grouped: BTreeMap<&str, Vec<&RevisionedEdit>> = BTreeMap::new();
    for edit in &bound.plan().edits { grouped.entry(&edit.document_id).or_default().push(edit); }
    if grouped.is_empty() {
        return Err(CoreError::new("transaction_empty", "Memory commit requires at least one edited document"));
    }
    let mut batches = Vec::with_capacity(grouped.len());
    for (id, edits) in grouped {
        let file = bound.snapshot().files.iter().find(|file| file.path == id).ok_or_else(invalid_position)?;
        let revision = file.revision.ok_or_else(invalid_position)?;
        let text = file.text().map_err(|_| invalid_position())?;
        let mut endpoints = BTreeSet::new();
        for edit in &edits {
            endpoints.insert(edit.start_utf16);
            endpoints.insert(edit.start_utf16.checked_add(edit.length_utf16).ok_or_else(invalid_position)?);
        }
        let positions = position_map(text, &endpoints)?;
        let mut converted = Vec::with_capacity(edits.len());
        for edit in edits {
            let end = edit.start_utf16.checked_add(edit.length_utf16).ok_or_else(invalid_position)?;
            converted.push(TextEdit {start: *positions.get(&edit.start_utf16).ok_or_else(invalid_position)?,
                end: *positions.get(&end).ok_or_else(invalid_position)?, text: edit.replacement.clone()});
        }
        batches.push((id.to_owned(), revision, converted));
    }
    Ok(batches)
}

impl RuntimeTransactionGate {
    pub(crate) fn new(owner: &Arc<Mutex<Runtime>>) -> Self { Self {owner: Arc::downgrade(owner)} }

    pub(crate) fn prepare(&self, edits: Vec<RevisionedEdit>) -> Result<RuntimeBoundTransaction> {
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let runtime = try_runtime(&owner)?;
        committed(&runtime)?;
        let transaction = service().prepare_workspace_transaction(&runtime.workspace, edits)
            .map_err(workspace_error)?;
        drop(runtime);
        let plan_id = transaction.plan().transaction_id.clone();
        Ok(RuntimeBoundTransaction {owner: self.owner.clone(), transaction, plan_id})
    }

    fn owner_for(&self, bound: &RuntimeBoundTransaction) -> Result<Arc<Mutex<Runtime>>> {
        if !Weak::ptr_eq(&self.owner, &bound.owner) {
            return Err(CoreError::new("runtime_transaction_owner_mismatch", "Transaction belongs to another Runtime"));
        }
        self.owner.upgrade().ok_or_else(unavailable)
    }

    pub(crate) fn revalidate(&self, bound: &RuntimeBoundTransaction) -> Result<()> {
        let owner = self.owner_for(bound)?;
        let runtime = try_runtime(&owner)?;
        let result = validate_locked(&runtime, bound);
        drop(runtime);
        result
    }

    /// Consume one opaque prepared plan. The trusted caller supplies the same
    /// origin used by existing edits; this introduces no Session/auth endpoint.
    /// All validation and one Workspace batch mutation share this exact guard.
    pub(crate) fn commit(&self, bound: RuntimeBoundTransaction, origin: &str) -> Result<DocumentInfo> {
        let owner = self.owner_for(&bound)?;
        let mut runtime = try_runtime(&owner)?;
        validate_locked(&runtime, &bound)?;
        let (document, revision, edits) = single_document_batch(&bound)?;
        let result = runtime.workspace.apply_edits(&document, revision, edits, origin);
        drop(runtime);
        result
    }

    /// Revalidate and prepare all targets before one Workspace group operation.
    /// The same genuine owner guard spans both validation and mutation; no
    /// sequential per-document apply or ad-hoc rollback is used here.
    pub(crate) fn commit_multi(&self, bound: RuntimeBoundTransaction, origin: &str) -> Result<RuntimeMultiCommitResult> {
        let owner = self.owner_for(&bound)?;
        let mut runtime = try_runtime(&owner)?;
        validate_locked(&runtime, &bound)?;
        let batches = multi_document_batches(&bound)?;
        let result = runtime.workspace.apply_edit_group(batches, origin)
            .map(|documents| RuntimeMultiCommitResult {documents});
        drop(runtime);
        result
    }
}

#[cfg(test)]
#[path = "runtime_transactions_tests.rs"]
mod tests;
