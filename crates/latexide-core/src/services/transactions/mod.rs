//! Preparation over a trusted immutable runtime snapshot, with no apply/save/history authority.
//!
//! `PreparedTransaction` is a plan, not an authenticated capability or an apply receipt.
//! The runtime must revalidate at apply time and retain physical BOM/EOL metadata itself.
//! Cancellation/deadline/auth checks belong to the trusted caller before invoking this v1 port.
use latexide_contracts::{
    ContractError, RevisionedEdit, SnapshotLimits, WorkspaceSnapshot,
    common::{MAX_FILE_BYTES, validate_edits},
    integration::is_relative_path,
    transactions::{PreparedTransaction, TransactionError, TransactionsPort},
};
use std::collections::BTreeMap;

// Byte-identical reuse of the frozen dependency-free P01 helper; no P01 source is modified.
mod sha256;
mod workspace_bridge;
pub use workspace_bridge::{WorkspaceTransaction, WorkspaceTransactionError};

/// Owns only host-chosen budgets. No documents, transaction counter, history, I/O or mutable state.
pub struct TransactionService {
    limits: SnapshotLimits,
}

impl TransactionService {
    pub fn new(limits: SnapshotLimits) -> Self {
        Self { limits }
    }

    /// Check an existing plan against a genuinely current immutable owner snapshot.
    ///
    /// Repeats the complete batch/range/quota validation and binds every snapshot field,
    /// logical byte and ordered edit. The caller still owns snapshot authenticity,
    /// cancellation/deadline checks and the critical section through any eventual apply.
    /// Success grants no apply/save authority and consumes no state or capability.
    pub fn revalidate_transaction(
        &self,
        snapshot: &WorkspaceSnapshot,
        prepared: &PreparedTransaction,
    ) -> Result<(), TransactionError> {
        let current_identity = self.validated_plan_identity(snapshot, &prepared.edits)?;
        if prepared.base_snapshot != snapshot.id || prepared.transaction_id != current_identity {
            return Err(TransactionError::StaleRevision);
        }
        Ok(())
    }

    // Share validation without cloning an unvalidated PreparedTransaction edit vector.
    fn validated_plan_identity(
        &self,
        snapshot: &WorkspaceSnapshot,
        edits: &[RevisionedEdit],
    ) -> Result<String, TransactionError> {
        // Lexical confinement is not OS identity/symlink validation or permission to write.
        if snapshot
            .files
            .iter()
            .any(|f| !is_relative_path(&f.path, false))
            || edits
                .iter()
                .any(|e| !is_relative_path(&e.document_id, false))
        {
            return Err(TransactionError::OutsideRoot);
        }
        snapshot.validate(&self.limits).map_err(map_boundary)?;
        // Validate the COMPLETE batch before constructing any prepared result.
        validate_edits(snapshot, edits).map_err(map_boundary)?;
        // An eventual edit needs a representable next revision; preparation never increments it.
        if edits
            .iter()
            .any(|e| e.expected_revision.checked_add(1).is_none())
        {
            return Err(TransactionError::Quota);
        }
        validate_projected_budgets(snapshot, edits, &self.limits)?;
        Ok(plan_identity(snapshot, edits))
    }
}

impl TransactionsPort for TransactionService {
    fn prepare_transaction(
        &self,
        snapshot: &WorkspaceSnapshot,
        edits: Vec<RevisionedEdit>,
    ) -> Result<PreparedTransaction, TransactionError> {
        let transaction_id = self.validated_plan_identity(snapshot, &edits)?;
        Ok(PreparedTransaction {
            transaction_id,
            edits,
            base_snapshot: snapshot.id.clone(),
        })
    }
}

fn map_boundary(error: ContractError) -> TransactionError {
    match error {
        ContractError::StaleRevision => TransactionError::StaleRevision,
        ContractError::Quota => TransactionError::Quota,
        // Frozen validate_edits intentionally folds overlap into InvalidRange.
        // Malformed snapshot/encoding/digest/range also has no more specific v1 service variant.
        _ => TransactionError::InvalidRange,
    }
}

/// Only computes logical byte sizes after common validation; builds no candidate text or state.
fn validate_projected_budgets(
    snapshot: &WorkspaceSnapshot,
    edits: &[RevisionedEdit],
    limits: &SnapshotLimits,
) -> Result<(), TransactionError> {
    let mut by_file: BTreeMap<&str, Vec<&RevisionedEdit>> = BTreeMap::new();
    for edit in edits {
        by_file.entry(&edit.document_id).or_default().push(edit);
    }
    let mut total = 0usize;
    for file in &snapshot.files {
        let mut size = file.utf8.len();
        if let Some(file_edits) = by_file.get_mut(file.path.as_str()) {
            file_edits.sort_unstable_by_key(|e| e.start_utf16);
            let text =
                std::str::from_utf8(&file.utf8).map_err(|_| TransactionError::InvalidRange)?;
            let mut characters = text.chars();
            let (mut units, mut bytes) = (0u32, 0usize);
            for edit in file_edits {
                advance(&mut characters, &mut units, &mut bytes, edit.start_utf16)?;
                let start_byte = bytes;
                let end = edit
                    .start_utf16
                    .checked_add(edit.length_utf16)
                    .ok_or(TransactionError::InvalidRange)?;
                advance(&mut characters, &mut units, &mut bytes, end)?;
                size = size
                    .checked_sub(bytes - start_byte)
                    .and_then(|n| n.checked_add(edit.replacement.len()))
                    .ok_or(TransactionError::Quota)?;
            }
        }
        if size > limits.max_file_bytes.min(MAX_FILE_BYTES) {
            return Err(TransactionError::Quota);
        }
        total = total.checked_add(size).ok_or(TransactionError::Quota)?;
        if total > limits.max_total_bytes {
            return Err(TransactionError::Quota);
        }
    }
    Ok(())
}

// Monotone size accounting for already-validated endpoints; no per-character offset table.
fn advance(
    characters: &mut std::str::Chars<'_>,
    units: &mut u32,
    bytes: &mut usize,
    target: u32,
) -> Result<(), TransactionError> {
    if target < *units {
        return Err(TransactionError::InvalidRange);
    }
    while *units < target {
        let character = characters.next().ok_or(TransactionError::InvalidRange)?;
        *units += character.len_utf16() as u32;
        *bytes += character.len_utf8();
    }
    if *units != target {
        return Err(TransactionError::InvalidRange);
    }
    Ok(())
}

/// A deterministic, length-framed SHA-256 correlation label over exact inputs, not authorization.
/// Files sort by exact UTF-8 path; edits retain caller order. No time, RNG, counter or pointer input.
pub(crate) fn plan_identity(snapshot: &WorkspaceSnapshot, edits: &[RevisionedEdit]) -> String {
    fn field(out: &mut Vec<u8>, value: &[u8]) {
        out.extend_from_slice(&(value.len() as u64).to_be_bytes());
        out.extend_from_slice(value);
    }
    let mut input = b"latexide-p02-plan-v1\0".to_vec();
    field(&mut input, snapshot.id.as_bytes());
    input.extend_from_slice(&snapshot.epoch.to_be_bytes());
    input.extend_from_slice(&(snapshot.files.len() as u64).to_be_bytes());
    let mut files: Vec<_> = snapshot.files.iter().collect();
    files.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    for file in files {
        field(&mut input, file.path.as_bytes());
        input.push(u8::from(file.revision.is_some()));
        input.extend_from_slice(&file.revision.unwrap_or(0).to_be_bytes());
        field(&mut input, file.content_sha256.as_bytes());
        field(&mut input, &file.utf8);
        input.push(u8::from(file.dirty));
    }
    input.extend_from_slice(&(edits.len() as u64).to_be_bytes());
    for edit in edits {
        field(&mut input, edit.document_id.as_bytes());
        input.extend_from_slice(&edit.expected_revision.to_be_bytes());
        input.extend_from_slice(&edit.start_utf16.to_be_bytes());
        input.extend_from_slice(&edit.length_utf16.to_be_bytes());
        field(&mut input, edit.replacement.as_bytes());
    }
    format!("p02-sha256-{}", content_sha256(&input))
}

/// Local integrity digest; never a write or authentication capability.
pub fn content_sha256(bytes: &[u8]) -> String { sha256::hex(bytes) }
