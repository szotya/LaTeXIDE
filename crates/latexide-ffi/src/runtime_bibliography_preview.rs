//! Internal readonly BibTeX rename facade on the existing bibliography owner.
//! Descriptions are review data. No apply, transaction, Save, wire or C ABI path.
use super::{Context, Runtime, RuntimeCiteCompletion, try_runtime};
use crate::runtime::analysis_capture::RuntimeAnalysisCapture;
use latexide_core::{CoreError, services::bibliography::{LocalBibliographyProvider,
    workspace_adapter::WorkspaceBibliography,
    key_rename::{KeyRenameEdit, KeyRenameError, KeyRenameLimits}}};
use std::sync::{Mutex, Weak};

#[derive(Debug)]
pub(crate) enum RuntimeBibRenameError {
    Runtime(CoreError),
    Preview(KeyRenameError),
}

#[path = "runtime_bibliography_rename.rs"]
pub(crate) mod rename_apply;
type Result<T> = std::result::Result<T, RuntimeBibRenameError>;

/// No public constructor, deserializer, Clone, mutable accessor or commit port.
/// Only Weak owner identity and bounded immutable capture/derived data survive.
pub(crate) struct RuntimeBibRenamePlan {
    owner: Weak<Mutex<Runtime>>,
    runtime_capture: RuntimeAnalysisCapture,
    bibliography: WorkspaceBibliography,
    context: Context,
    old_key: String,
    new_key: String,
    limits: KeyRenameLimits,
    edits: Vec<KeyRenameEdit>,
}
impl RuntimeBibRenamePlan {
    pub(crate) fn edits(&self) -> &[KeyRenameEdit] { &self.edits }
    pub(crate) fn old_key(&self) -> &str { &self.old_key }
    pub(crate) fn new_key(&self) -> &str { &self.new_key }
    pub(crate) fn capture_id(&self) -> &str { &self.bibliography.snapshot().id }
    pub(crate) fn capture_epoch(&self) -> u64 { self.bibliography.snapshot().epoch }
}
fn unavailable() -> RuntimeBibRenameError {
    RuntimeBibRenameError::Runtime(CoreError::new("runtime_unavailable", "Bibliography preview Runtime is gone or poisoned"))
}
impl RuntimeCiteCompletion {
    /// Requires the production capture's complete text-only project root.
    /// Binary/non-text assets refuse the whole preview with a text-only diagnostic;
    /// unsupported hosts retain their existing analysis refusal.
    pub(crate) fn preview_bib_rename(&self, old_key: &str, new_key: &str) -> Result<RuntimeBibRenamePlan> {
        self.preview_bib_rename_with_limits(old_key, new_key, KeyRenameLimits::default())
    }

    pub(crate) fn preview_bib_rename_with_limits(&self, old_key: &str, new_key: &str,
        limits: KeyRenameLimits) -> Result<RuntimeBibRenamePlan>
    {
        let runtime_capture = self.gate.capture()
            .map_err(|error| RuntimeBibRenameError::Runtime(self.capture_error(error)))?;
        let (context, bibliography, edits) = {
            let owner = self.owner.upgrade().ok_or_else(unavailable)?;
            let mut runtime = try_runtime(&owner).map_err(RuntimeBibRenameError::Runtime)?;
            let context = Context::observe(&runtime).map_err(RuntimeBibRenameError::Runtime)?;
            runtime.workspace.revalidate_analysis_snapshot(runtime_capture.snapshot())
                .map_err(RuntimeBibRenameError::Runtime)?;
            let bibliography = LocalBibliographyProvider.inspect_workspace(&mut runtime.workspace)
                .map_err(RuntimeBibRenameError::Runtime)?;
            let edits = {
                let checked = bibliography.preview_key_rename_with_limits(&mut runtime.workspace,
                    old_key, new_key, &limits).map_err(RuntimeBibRenameError::Preview)?;
                checked.edits().to_vec()
            };
            (context, bibliography, edits)
        };
        let plan = RuntimeBibRenamePlan { owner: self.owner.clone(), runtime_capture, bibliography,
            context, old_key: old_key.into(), new_key: new_key.into(), limits, edits };
        self.revalidate_bib_rename(&plan)?;
        Ok(plan)
    }

    /// A fresh readonly observation, never a lease or authorization to mutate.
    /// Recompute against private bound keys/limits; copied display rows are not input.
    pub(crate) fn revalidate_bib_rename(&self, plan: &RuntimeBibRenamePlan) -> Result<()> {
        if !Weak::ptr_eq(&self.owner, &plan.owner) {
            return Err(RuntimeBibRenameError::Runtime(CoreError::new(
                "runtime_bib_preview_owner_mismatch", "Bibliography preview belongs to another Runtime allocation")));
        }
        self.gate.revalidate(&plan.runtime_capture).map_err(RuntimeBibRenameError::Runtime)?;
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner).map_err(RuntimeBibRenameError::Runtime)?;
        if Context::observe(&runtime).map_err(RuntimeBibRenameError::Runtime)? != plan.context {
            return Err(RuntimeBibRenameError::Runtime(CoreError::new(
                "stale_runtime_bib_context", "Bibliography preview Runtime context changed")));
        }
        let current = plan.bibliography.preview_key_rename_with_limits(&mut runtime.workspace,
            &plan.old_key, &plan.new_key, &plan.limits).map_err(RuntimeBibRenameError::Preview)?;
        if current.edits() != plan.edits.as_slice() {
            return Err(RuntimeBibRenameError::Runtime(CoreError::new(
                "runtime_bib_preview_changed", "Bibliography preview no longer matches its exact derived rows")));
        }
        Ok(())
    }
}
