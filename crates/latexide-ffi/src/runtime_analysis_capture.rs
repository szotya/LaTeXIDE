//! Internal analysis gate over the actual native/Session Arc<Mutex<Runtime>>.
//!
//! A Weak binds the allocation without extending Runtime lifetime. Snapshot IDs
//! and the session descriptor path are observations, never authentication or a
//! lease. Existing Session authentication remains at its unchanged boundary.
//! Capture/revalidation use try_lock and release the guard before returning;
//! consumers receive owned immutable input and must do expensive work outside.
//! A successful check does not reserve state for a later edit or compiler job.
use super::{CompilerSettings, Runtime};
use latexide_build::{Backend, Engine};
use latexide_core::{CoreError, Result, contracts::{SnapshotLimits, WorkspaceSnapshot}};
use std::{path::PathBuf, sync::{Arc, Mutex, MutexGuard, TryLockError, Weak}, time::Duration};

#[derive(Debug, PartialEq, Eq)]
enum BackendBinding { Tectonic, PdfLaTex, XeLaTex, LuaLaTex }

#[derive(Debug, PartialEq, Eq)]
struct CompilerBinding {
    path: PathBuf,
    cache: PathBuf,
    output: PathBuf,
    backend: BackendBinding,
    timeout: Duration,
}

impl From<&CompilerSettings> for CompilerBinding {
    fn from(settings: &CompilerSettings) -> Self {
        let backend = match settings.backend {
            Backend::Tectonic => BackendBinding::Tectonic,
            Backend::LatexMk {engine: Engine::PdfLaTex} => BackendBinding::PdfLaTex,
            Backend::LatexMk {engine: Engine::XeLaTex} => BackendBinding::XeLaTex,
            Backend::LatexMk {engine: Engine::LuaLaTex} => BackendBinding::LuaLaTex,
        };
        Self {path: settings.path.clone(), cache: settings.cache.clone(),
            output: settings.output.clone(), backend, timeout: settings.timeout}
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RuntimeContext {
    session_descriptor: PathBuf,
    compiler: Option<CompilerBinding>,
}

impl RuntimeContext {
    fn observe(runtime: &Runtime) -> Result<Self> {
        if runtime.composition.is_some() {
            return Err(CoreError::new("composition_active", "Analysis waits for committed native composition"));
        }
        // This host has no additional Runtime generation or trust-policy field.
        // Workspace's own generation/epoch is checked by its unchanged API.
        Ok(Self {session_descriptor: runtime.session_descriptor.clone(),
            compiler: runtime.compiler.as_ref().map(CompilerBinding::from)})
    }
}

#[derive(Clone)]
pub(crate) struct RuntimeAnalysisGate { owner: Weak<Mutex<Runtime>> }

pub(crate) struct RuntimeAnalysisCapture {
    owner: Weak<Mutex<Runtime>>,
    context: RuntimeContext,
    snapshot: WorkspaceSnapshot,
}

impl RuntimeAnalysisCapture {
    pub(crate) fn snapshot(&self) -> &WorkspaceSnapshot { &self.snapshot }
}

fn unavailable() -> CoreError {
    CoreError::new("runtime_unavailable", "Analysis Runtime owner is gone or poisoned")
}

fn try_runtime(owner: &Arc<Mutex<Runtime>>) -> Result<MutexGuard<'_, Runtime>> {
    owner.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => CoreError::new("runtime_busy", "Analysis Runtime is busy; retry from the caller"),
        TryLockError::Poisoned(_) => unavailable(),
    })
}

impl RuntimeAnalysisGate {
    /// Trusted Rust callers pass the same Arc used by the native host/Session.
    /// No caller-provided token, document copy or substitute host is accepted.
    pub(crate) fn new(owner: &Arc<Mutex<Runtime>>) -> Self { Self {owner: Arc::downgrade(owner)} }

    pub(crate) fn capture(&self) -> Result<RuntimeAnalysisCapture> { self.capture_inner(None) }

    /// The caller can only lower the unchanged Workspace capture ceilings.
    pub(crate) fn capture_with_limits(&self, limits: &SnapshotLimits) -> Result<RuntimeAnalysisCapture> {
        self.capture_inner(Some(limits))
    }

    fn capture_inner(&self, limits: Option<&SnapshotLimits>) -> Result<RuntimeAnalysisCapture> {
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner)?;
        let context = RuntimeContext::observe(&runtime)?;
        let snapshot = match limits {
            Some(limits) => runtime.workspace.analysis_snapshot_with_limits(limits)?,
            None => runtime.workspace.analysis_snapshot()?,
        };
        drop(runtime);
        Ok(RuntimeAnalysisCapture {owner: self.owner.clone(), context, snapshot})
    }

    pub(crate) fn revalidate(&self, capture: &RuntimeAnalysisCapture) -> Result<()> {
        if !Weak::ptr_eq(&self.owner, &capture.owner) {
            return Err(CoreError::new("runtime_analysis_owner_mismatch", "Analysis belongs to another Runtime allocation"));
        }
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut runtime = try_runtime(&owner)?;
        if RuntimeContext::observe(&runtime)? != capture.context {
            return Err(CoreError::new("stale_runtime_analysis", "Runtime analysis context changed"));
        }
        runtime.workspace.revalidate_analysis_snapshot(&capture.snapshot)
    }
}
