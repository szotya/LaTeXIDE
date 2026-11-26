//! Internal cite application on the one native/Session Arc Runtime allocation.
//! Weak handles never extend its lifetime; try_lock never waits on its owner.
//! The gate checks preparation/context; the reviewed Core plan does final exact
//! token/revision/capture/canonical checks and mutation under the same final guard.
//! No Session, wire/C ABI, implicit Save or second editable Workspace is added.
use super::{Runtime, analysis_capture::{RuntimeAnalysisCapture, RuntimeAnalysisGate}};
use latexide_build::{Backend, Engine};
use latexide_core::{CoreError, Result, services::bibliography::{LocalBibliographyProvider,
    completion_adapter::{CiteCompletion, CiteCompletionPlan, CiteCompletionEdit, CiteCompletionApplied}}};
use std::{path::PathBuf, sync::{Arc, Mutex, MutexGuard, TryLockError, Weak}, time::Duration};

#[derive(PartialEq, Eq)]
struct Context {
    session_descriptor: PathBuf,
    compiler: Option<(PathBuf, PathBuf, PathBuf, u8, Duration)>,
}
impl Context {
    fn observe(runtime: &Runtime) -> Result<Self> {
        if runtime.composition.is_some() {
            return Err(CoreError::new("composition_active", "Citation apply waits for committed native composition"));
        }
        let compiler = runtime.compiler.as_ref().map(|settings| {
            let backend = match settings.backend {
                Backend::Tectonic => 0,
                Backend::LatexMk { engine: Engine::PdfLaTex } => 1,
                Backend::LatexMk { engine: Engine::XeLaTex } => 2,
                Backend::LatexMk { engine: Engine::LuaLaTex } => 3,
            };
            (settings.path.clone(), settings.cache.clone(), settings.output.clone(), backend, settings.timeout)
        });
        // Same observable context as the existing P00 gate; no extra Runtime
        // generation/policy field or caller-supplied authentication is invented.
        Ok(Self { session_descriptor: runtime.session_descriptor.clone(), compiler })
    }
}

struct Query { document_id: String, revision: u64, caret_utf16: u32, candidate: String }
pub(crate) struct RuntimeCiteToken {
    owner: Weak<Mutex<Runtime>>,
    capture: RuntimeAnalysisCapture,
    context: Context,
    query: Query,
    plan: Option<CiteCompletionPlan>,
}
impl RuntimeCiteToken {
    pub(crate) fn is_consumed(&self) -> bool { self.plan.is_none() }
    pub(crate) fn description(&self) -> Option<&CiteCompletionEdit> {
        self.plan.as_ref().map(CiteCompletionPlan::description)
    }
}

pub(crate) struct RuntimeCiteCompletion {
    owner: Weak<Mutex<Runtime>>,
    gate: RuntimeAnalysisGate,
}
fn unavailable() -> CoreError {
    CoreError::new("runtime_unavailable", "Citation Runtime owner is gone or poisoned")
}
fn try_runtime(owner: &Arc<Mutex<Runtime>>) -> Result<MutexGuard<'_, Runtime>> {
    owner.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => CoreError::new("runtime_busy", "Citation Runtime is busy"),
        TryLockError::Poisoned(_) => unavailable(),
    })
}
fn consumed() -> CoreError { CoreError::new("runtime_cite_plan_consumed", "Runtime citation token was already consumed") }

impl RuntimeCiteCompletion {
    pub(crate) fn new(owner: &Arc<Mutex<Runtime>>) -> Self {
        Self { owner: Arc::downgrade(owner), gate: RuntimeAnalysisGate::new(owner) }
    }

    pub(crate) fn prepare(&self, document_id: &str, revision: u64, caret_utf16: u32, candidate: &str)
        -> Result<RuntimeCiteToken>
    {
        let capture = self.gate.capture()?;
        let (context, plan) = {
            let owner = self.owner.upgrade().ok_or_else(unavailable)?;
            let mut runtime = try_runtime(&owner)?;
            let context = Context::observe(&runtime)?;
            runtime.workspace.revalidate_analysis_snapshot(capture.snapshot())?;
            let plan = match LocalBibliographyProvider.plan_citation_completion(
                &mut runtime.workspace, document_id, revision, caret_utf16, candidate)?
            {
                CiteCompletion::Planned(plan) => plan,
                CiteCompletion::Unsupported => return Err(CoreError::new("cite_completion_unsupported", "Citation query is unsupported")),
                CiteCompletion::NoCandidate => return Err(CoreError::new("cite_completion_missing", "Canonical citation candidate is unavailable")),
            };
            (context, plan)
        };
        // That guard and upgraded Arc are gone before this independent check.
        self.gate.revalidate(&capture)?;
        Ok(RuntimeCiteToken { owner: self.owner.clone(), capture, context,
            query: Query { document_id: document_id.into(), revision, caret_utf16, candidate: candidate.into() },
            plan: Some(plan) })
    }

    /// Readonly observation only; not a reservation for a later apply.
    pub(crate) fn revalidate(&self, token: &RuntimeCiteToken) -> Result<()> {
        if token.is_consumed() { return Err(consumed()); }
        self.gate.revalidate(&token.capture)
    }

    /// Every explicit attempt spends the opaque token, even busy/foreign/context
    /// refusals. A caller must prepare a fresh token for a new attempt.
    pub(crate) fn apply(&self, token: &mut RuntimeCiteToken) -> Result<CiteCompletionApplied> {
        let mut plan = token.plan.take().ok_or_else(consumed)?;
        if !Weak::ptr_eq(&self.owner, &token.owner) {
            return Err(CoreError::new("runtime_cite_owner_mismatch", "Citation token belongs to another Runtime allocation"));
        }
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner)?;
        if Context::observe(&runtime)? != token.context {
            return Err(CoreError::new("stale_runtime_cite_context", "Citation Runtime context changed"));
        }
        // Query is private immutable data from prepare, not a caller-provided
        // edit. Core apply rechecks the actual current token/context/key/capture.
        let edit = plan.description();
        let end = edit.source.range.start.checked_add(edit.source.range.length)
            .ok_or_else(|| CoreError::new("invalid_position", "Citation range overflow"))?;
        if edit.source.document_id != token.query.document_id || edit.source.revision != Some(token.query.revision)
            || edit.insertion != token.query.candidate || token.query.caret_utf16 < edit.source.range.start
            || token.query.caret_utf16 > end
        { return Err(CoreError::new("runtime_cite_query_mismatch", "Citation query and exact Core plan differ")); }
        let result = plan.apply(&mut runtime.workspace);
        drop(runtime);
        result
    }
}

#[path = "runtime_bibliography_preview.rs"]
pub(crate) mod rename_preview;
