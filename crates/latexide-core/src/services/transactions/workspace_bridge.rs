//! Planning over the real Workspace owner capture; no mutation or retained lease.
use super::TransactionService;
use crate::{CoreError, Workspace};
use latexide_contracts::{RevisionedEdit, WorkspaceSnapshot};
use latexide_contracts::transactions::{PreparedTransaction, TransactionError, TransactionsPort};

/// Owned immutable planning data captured by this bridge from an actual Workspace.
///
/// The fields are private to preserve the capture/plan pairing through this API.
/// This Rust-only value has no serialization, authentication or apply authority.
/// It retains no Workspace borrow or Runtime lock after a method returns.
#[derive(Debug)]
pub struct WorkspaceTransaction {
    snapshot: WorkspaceSnapshot,
    plan: PreparedTransaction,
}

impl WorkspaceTransaction {
    pub fn snapshot(&self) -> &WorkspaceSnapshot {
        &self.snapshot
    }

    pub fn plan(&self) -> &PreparedTransaction {
        &self.plan
    }
}

/// Preserve the real owner error separately from v1 planning refusal.
/// This is an in-process Rust error, not a new contract/wire DTO.
#[derive(Debug)]
pub enum WorkspaceTransactionError {
    Workspace(CoreError),
    Transaction(TransactionError),
}

impl std::fmt::Display for WorkspaceTransactionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Workspace(error) => write!(formatter, "Workspace capture: {error}"),
            Self::Transaction(error) => write!(formatter, "Transaction planning: {error:?}"),
        }
    }
}

impl std::error::Error for WorkspaceTransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Workspace(error) => Some(error),
            Self::Transaction(_) => None,
        }
    }
}

impl TransactionService {
    /// Capture retained documents from their actual owner and validate the whole plan.
    /// Caller edits provide expected revisions/ranges; the owner supplies identity/epoch.
    /// Capture budgets may lower the host limits and cannot raise the editor ceilings.
    pub fn prepare_workspace_transaction(
        &self,
        workspace: &Workspace,
        edits: Vec<RevisionedEdit>,
    ) -> Result<WorkspaceTransaction, WorkspaceTransactionError> {
        let snapshot = workspace
            .transaction_snapshot_with_limits(&self.limits)
            .map_err(WorkspaceTransactionError::Workspace)?;
        let plan = self
            .prepare_transaction(&snapshot, edits)
            .map_err(WorkspaceTransactionError::Transaction)?;
        workspace
            .revalidate_transaction_snapshot(&snapshot)
            .map_err(WorkspaceTransactionError::Workspace)?;
        Ok(WorkspaceTransaction { snapshot, plan })
    }

    /// Recheck the owner generation/epoch/content and the entire v1 plan binding.
    ///
    /// The shared Workspace borrow covers only this call. A future mutating Runtime
    /// caller must keep its real owner lock/lease across revalidation and mutation;
    /// this successful read does not create or retain that critical section.
    pub fn revalidate_workspace_transaction(
        &self,
        workspace: &Workspace,
        transaction: &WorkspaceTransaction,
    ) -> Result<(), WorkspaceTransactionError> {
        workspace
            .revalidate_transaction_snapshot(&transaction.snapshot)
            .map_err(WorkspaceTransactionError::Workspace)?;
        self.revalidate_transaction(&transaction.snapshot, &transaction.plan)
            .map_err(WorkspaceTransactionError::Transaction)
    }
}
