//! Internal consumption of the original opaque Runtime bibliography preview.
//! The same real Runtime guard covers final validation and the Core commit.
//! No second editable workspace, wire endpoint or implicit Save is introduced.
use super::{RuntimeBibRenameError, RuntimeBibRenamePlan};
use super::super::{Context, Runtime, RuntimeCiteCompletion, try_runtime};
use latexide_core::{CoreError, DocumentInfo, contracts::WorkspaceSnapshot,
    services::{bibliography::key_rename::BibKeyRenamePreview,
        transactions::bib_rename_adapter::prepare_bib_rename}};
use std::sync::Weak;

type Result<T> = std::result::Result<T, RuntimeBibRenameError>;

/// Private retained owner capture and original preview. Display rows cannot
/// construct this token. It owns no Arc or lock and cannot be cloned.
pub(crate) struct RuntimeBibRenameApply {
    preview: Option<RuntimeBibRenamePlan>,
    retained: WorkspaceSnapshot,
}
impl RuntimeBibRenameApply {
    pub(crate) fn is_consumed(&self) -> bool { self.preview.is_none() }
}
fn refused(code: &str, message: &str) -> RuntimeBibRenameError {
    RuntimeBibRenameError::Runtime(CoreError::new(code, message))
}
fn unavailable() -> RuntimeBibRenameError {
    refused("runtime_unavailable", "Bibliography rename Runtime is gone or poisoned")
}

// This borrow is of the immutable bibliography, never the mutable Runtime.
// Rebuild the genuine Core preview rather than accepting copied edit rows.
fn checked_preview<'a>(runtime: &mut Runtime, plan: &'a RuntimeBibRenamePlan)
    -> Result<BibKeyRenamePreview<'a>>
{
    if Context::observe(runtime).map_err(RuntimeBibRenameError::Runtime)? != plan.context {
        return Err(refused("stale_runtime_bib_context", "Bibliography Runtime context changed"));
    }
    runtime.workspace.revalidate_analysis_snapshot(plan.runtime_capture.snapshot())
        .map_err(RuntimeBibRenameError::Runtime)?;
    let current = plan.bibliography.preview_key_rename_with_limits(&mut runtime.workspace,
        &plan.old_key, &plan.new_key, &plan.limits).map_err(RuntimeBibRenameError::Preview)?;
    if current.edits() != plan.edits.as_slice() {
        return Err(refused("runtime_bib_preview_changed", "Exact private rename rows changed"));
    }
    Ok(current)
}

impl RuntimeCiteCompletion {
    pub(crate) fn prepare_bib_rename_apply(&self, preview: RuntimeBibRenamePlan)
        -> Result<RuntimeBibRenameApply>
    {
        if !Weak::ptr_eq(&self.owner, &preview.owner) {
            return Err(refused("runtime_bib_apply_owner_mismatch", "Rename preview belongs to another Runtime allocation"));
        }
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner).map_err(RuntimeBibRenameError::Runtime)?;
        let retained = runtime.workspace.transaction_snapshot().map_err(RuntimeBibRenameError::Runtime)?;
        let current = checked_preview(&mut runtime, &preview)?;
        // Admit every retained target, expected old text and scalar-safe UTF16
        // endpoint now. A borrowed Core token cannot be stored self-referentially;
        // final apply recreates and immediately consumes it under one guard.
        let prepared = prepare_bib_rename(&mut runtime.workspace, current)
            .map_err(RuntimeBibRenameError::Runtime)?;
        drop(prepared);
        runtime.workspace.revalidate_transaction_snapshot(&retained)
            .map_err(RuntimeBibRenameError::Runtime)?;
        Ok(RuntimeBibRenameApply { preview: Some(preview), retained })
    }

    /// Every explicit attempt consumes the token, including busy/foreign/gone
    /// refusals. No owner unlock occurs between final all-target checks and the
    /// existing atomic memory/history commit. Disk observation is sequential,
    /// not hostile filesystem race isolation or an atomic multi-file Save.
    pub(crate) fn apply_bib_rename(&self, token: &mut RuntimeBibRenameApply, origin: &str)
        -> Result<Vec<DocumentInfo>>
    {
        let preview = token.preview.take().ok_or_else(|| refused(
            "runtime_bib_apply_consumed", "Runtime bibliography rename token was already consumed"))?;
        if !Weak::ptr_eq(&self.owner, &preview.owner) {
            return Err(refused("runtime_bib_apply_owner_mismatch", "Rename token belongs to another Runtime allocation"));
        }
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner).map_err(RuntimeBibRenameError::Runtime)?;
        // Bind the original retained host generation/epoch and every revision,
        // including edit -> undo ABA that restores all original text bytes.
        runtime.workspace.revalidate_transaction_snapshot(&token.retained)
            .map_err(RuntimeBibRenameError::Runtime)?;
        let current = checked_preview(&mut runtime, &preview)?;
        let mut prepared = prepare_bib_rename(&mut runtime.workspace, current)
            .map_err(RuntimeBibRenameError::Runtime)?;
        prepared.commit(&mut runtime.workspace, origin).map_err(RuntimeBibRenameError::Runtime)
    }
}

#[cfg(test)]
#[path = "runtime_bibliography_rename_tests.rs"]
mod tests;
