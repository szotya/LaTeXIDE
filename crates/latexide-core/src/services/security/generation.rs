//! Host-owned input generations; no wire authority and no Runtime registration.

use super::execution::{
    CompilerSandbox, ExecutionEnvironment, ExecutionError, ExecutionState, GuardedRunError,
    HostExecutionConfig,
};
use std::{
    collections::BTreeSet,
    ops::Deref,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[path = "generation_digest.rs"]
mod digest;
pub use digest::Digest;
#[cfg(target_os = "macos")]
#[path = "generation_io.rs"]
mod capture_io;

const MAX_INPUTS: usize = 10;
const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 10 * MAX_INPUT_BYTES;

#[derive(Debug, PartialEq, Eq)]
pub enum GenerationError {
    IsolationUnavailable,
    HostUnavailable,
    StaleHost,
    InvalidInput,
    Quota,
    Cancelled,
    SourceChanged,
    SnapshotChanged,
    Storage,
    Configuration,
    Execution(ExecutionError),
    CompilerFailed(ExecutionState),
    OutputChanged,
}

/// Raw compiler bytes, not a decoded/normalized document snapshot. A dirty
/// buffer has independent compiler bytes and an expected on-disk baseline.
#[derive(Clone)]
pub struct HostInput {
    relative: String,
    revision: u64,
    bytes: Vec<u8>,
    digest: Digest,
    disk_digest: Digest,
}
fn input_path(text: &str) -> Result<&Path, GenerationError> {
    if text.is_empty()
        || text.len() > 4096
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | ':'))
        || text
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | ".."))
        || text.split('/').count() > 16
    {
        return Err(GenerationError::InvalidInput);
    }
    let path = Path::new(text);
    if !path.is_relative() {
        return Err(GenerationError::InvalidInput);
    }
    Ok(path)
}
impl HostInput {
    pub fn disk(relative: &str, revision: u64, bytes: Vec<u8>) -> Result<Self, GenerationError> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(GenerationError::Quota);
        }
        let digest = Digest::of(&bytes)?;
        Self::buffer(relative, revision, bytes, digest)
    }
    pub fn buffer(
        relative: &str,
        revision: u64,
        bytes: Vec<u8>,
        disk_digest: Digest,
    ) -> Result<Self, GenerationError> {
        input_path(relative)?;
        if revision == 0 {
            return Err(GenerationError::InvalidInput);
        }
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(GenerationError::Quota);
        }
        Ok(Self {
            relative: relative.into(),
            revision,
            digest: Digest::of(&bytes)?,
            bytes,
            disk_digest,
        })
    }
}

/// Opaque, in-process binding. It cannot be decoded from a client manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBinding {
    workspace: [u8; 16],
    generation: u64,
    digest: Digest,
}
impl InputBinding {
    pub fn workspace(&self) -> [u8; 16] {
        self.workspace
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
}

/// Construct only from the one authoritative host Runtime. This value carries
/// bytes, not authority by itself. No serde, token, trusted flag, or wire port.
#[derive(Clone)]
pub struct HostSnapshot {
    binding: InputBinding,
    project: PathBuf,
    main: String,
    files: Vec<HostInput>,
}
impl HostSnapshot {
    pub fn new(
        workspace: [u8; 16],
        generation: u64,
        project: PathBuf,
        main: String,
        mut files: Vec<HostInput>,
    ) -> Result<Self, GenerationError> {
        if generation == 0 || workspace == [0; 16] || files.is_empty() {
            return Err(GenerationError::InvalidInput);
        }
        if files.len() > MAX_INPUTS {
            return Err(GenerationError::Quota);
        }
        let root = project.to_str().ok_or(GenerationError::InvalidInput)?;
        if root.len() > 4096 || !project.is_absolute() || super::checked_path(root).is_err() {
            return Err(GenerationError::InvalidInput);
        }
        input_path(&main)?;
        files.sort_by(|a, b| a.relative.cmp(&b.relative));
        if files.windows(2).any(|f| f[0].relative == f[1].relative)
            || !files.iter().any(|f| f.relative == main)
        {
            return Err(GenerationError::InvalidInput);
        }
        let total = files.iter().try_fold(0usize, |size, file| {
            size.checked_add(file.bytes.len())
                .ok_or(GenerationError::Quota)
        })?;
        if total > MAX_TOTAL_BYTES {
            return Err(GenerationError::Quota);
        }
        let mut manifest = b"latexide-p18-host-input-generation-v1\0".to_vec();
        manifest.extend_from_slice(&workspace);
        manifest.extend_from_slice(&generation.to_be_bytes());
        let field = |manifest: &mut Vec<u8>, bytes: &[u8]| {
            manifest.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            manifest.extend_from_slice(bytes);
        };
        field(&mut manifest, root.as_bytes());
        field(&mut manifest, main.as_bytes());
        manifest.extend_from_slice(&(files.len() as u64).to_be_bytes());
        for file in &files {
            field(&mut manifest, file.relative.as_bytes());
            manifest.extend_from_slice(&file.revision.to_be_bytes());
            manifest.extend_from_slice(&(file.bytes.len() as u64).to_be_bytes());
            manifest.extend_from_slice(&file.digest.0);
            manifest.extend_from_slice(&file.disk_digest.0);
        }
        if manifest.len() > 64 * 1024 {
            return Err(GenerationError::Quota);
        }
        Ok(Self {
            binding: InputBinding {
                workspace,
                generation,
                digest: Digest::of(&manifest)?,
            },
            project,
            main,
            files,
        })
    }
    pub fn binding(&self) -> &InputBinding {
        &self.binding
    }
}

/// The host must hold its authoritative read lock for this lease's lifetime.
/// Acquisition MUST be nonblocking: contention/teardown returns HostUnavailable,
/// never an unbounded lock wait that prevents cancel/Stop/Join from progressing.
/// Edits, input-set/trust changes advance a monotonic generation, without ABA
/// rollback. Returning an unlocked copied snapshot does NOT meet this contract.
/// The module does not create another Runtime or enforce a caller's lock design.
pub trait HostInputAuthority {
    type Lease<'a>: Deref<Target = HostSnapshot>
    where
        Self: 'a;
    fn try_lease_current(&self) -> Result<Self::Lease<'_>, GenerationError>;
}
fn lease<'a, A: HostInputAuthority>(
    authority: &'a A,
    expected: &InputBinding,
) -> Result<A::Lease<'a>, GenerationError> {
    let current = authority.try_lease_current()?;
    if current.binding() != expected {
        return Err(GenerationError::StaleHost);
    }
    Ok(current)
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), GenerationError> {
    if cancel.load(Ordering::Acquire) {
        Err(GenerationError::Cancelled)
    } else {
        Ok(())
    }
}

pub struct GenerationLayout {
    input: PathBuf,
    inputs: Vec<PathBuf>,
    main: PathBuf,
    output: PathBuf,
    cache: PathBuf,
    temporary: PathBuf,
}
impl GenerationLayout {
    pub fn input_root(&self) -> &Path {
        &self.input
    }
    pub fn inputs(&self) -> &[PathBuf] {
        &self.inputs
    }
    pub fn main_input(&self) -> &Path {
        &self.main
    }
    pub fn output_root(&self) -> &Path {
        &self.output
    }
    pub fn cache_root(&self) -> &Path {
        &self.cache
    }
    pub fn temporary_root(&self) -> &Path {
        &self.temporary
    }
}
pub struct CapturedInputs {
    binding: InputBinding,
    layout: GenerationLayout,
    #[cfg(target_os = "macos")]
    storage: capture_io::Directory,
    #[cfg(target_os = "macos")]
    original: capture_io::Directory,
    #[cfg(target_os = "macos")]
    sources: Vec<capture_io::FileWitness>,
    #[cfg(target_os = "macos")]
    snapshot: capture_io::Directory,
    #[cfg(target_os = "macos")]
    captured: Vec<capture_io::FileWitness>,
}
#[cfg(all(test, target_os = "macos"))]
enum CapturePoint {
    AfterInput(usize),
    BeforePublish,
}

impl CapturedInputs {
    pub fn capture<A: HostInputAuthority>(
        authority: &A,
        expected: &InputBinding,
        directory: &Path,
        cancel: &AtomicBool,
    ) -> Result<Self, GenerationError> {
        #[cfg(target_os = "macos")]
        {
            Self::capture_inner(
                authority,
                expected,
                directory,
                cancel,
                #[cfg(test)]
                None,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (authority, expected, directory, cancel);
            Err(GenerationError::IsolationUnavailable)
        }
    }
    pub fn input_root(&self) -> &Path {
        &self.layout.input
    }
    pub fn binding(&self) -> &InputBinding {
        &self.binding
    }
    #[cfg(all(test, target_os = "macos"))]
    fn capture_with_hook<A: HostInputAuthority>(
        authority: &A,
        expected: &InputBinding,
        directory: &Path,
        cancel: &AtomicBool,
        hook: &mut dyn FnMut(CapturePoint),
    ) -> Result<Self, GenerationError> {
        Self::capture_inner(authority, expected, directory, cancel, Some(hook))
    }
    #[cfg(target_os = "macos")]
    fn capture_inner<A: HostInputAuthority>(
        authority: &A,
        expected: &InputBinding,
        directory: &Path,
        cancel: &AtomicBool,
        #[cfg(test)] mut hook: Option<&mut dyn FnMut(CapturePoint)>,
    ) -> Result<Self, GenerationError> {
        check_cancel(cancel)?;
        let current = lease(authority, expected)?;
        if !directory.is_absolute()
            || directory
                .to_str()
                .is_none_or(|s| super::checked_path(s).is_err())
            || directory.starts_with(&current.project)
            || current.project.starts_with(directory)
        {
            return Err(GenerationError::InvalidInput);
        }
        let original = capture_io::Directory::open(&current.project, false)
            .map_err(|_| GenerationError::SourceChanged)?;
        let storage =
            capture_io::Directory::create_private(directory, &original).map_err(|error| {
                if error.kind() == std::io::ErrorKind::InvalidInput {
                    GenerationError::InvalidInput
                } else {
                    GenerationError::Storage
                }
            })?;
        let mut builder =
            capture_io::Builder::new(&storage).map_err(|_| GenerationError::Storage)?;
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        for (index, input) in current.files.iter().enumerate() {
            check_cancel(cancel)?;
            let source = capture_io::FileWitness::capture(
                &original,
                Path::new(&input.relative),
                MAX_INPUT_BYTES,
                false,
            )
            .map_err(|_| GenerationError::SourceChanged)?;
            if source.digest != input.disk_digest {
                return Err(GenerationError::SourceChanged);
            }
            if !seen.insert(source.identity()) {
                return Err(GenerationError::InvalidInput);
            }
            builder
                .write(Path::new(&input.relative), &input.bytes, cancel)
                .map_err(|_| {
                    if cancel.load(Ordering::Acquire) {
                        GenerationError::Cancelled
                    } else {
                        GenerationError::Storage
                    }
                })?;
            sources.push(source);
            #[cfg(test)]
            if let Some(callback) = hook.as_mut() {
                callback(CapturePoint::AfterInput(index));
            }
            #[cfg(not(test))]
            let _ = index;
        }
        #[cfg(test)]
        if let Some(callback) = hook.as_mut() {
            callback(CapturePoint::BeforePublish);
        }
        check_cancel(cancel)?;
        for source in &sources {
            source
                .revalidate(&original, MAX_INPUT_BYTES)
                .map_err(|_| GenerationError::SourceChanged)?;
        }
        builder.seal().map_err(|_| GenerationError::Storage)?;
        check_cancel(cancel)?;
        storage
            .publish_input()
            .map_err(|_| GenerationError::Storage)?;
        let input = directory.join("input");
        let mut snapshot = capture_io::Directory::open(&input, true)
            .map_err(|_| GenerationError::SnapshotChanged)?;
        snapshot
            .hold_sealed_change()
            .map_err(|_| GenerationError::SnapshotChanged)?;
        let mut captured = Vec::new();
        let mut inputs = Vec::new();
        for file in &current.files {
            let witness = capture_io::FileWitness::capture(
                &snapshot,
                Path::new(&file.relative),
                MAX_INPUT_BYTES,
                true,
            )
            .map_err(|_| GenerationError::SnapshotChanged)?;
            if witness.digest != file.digest {
                return Err(GenerationError::SnapshotChanged);
            }
            inputs.push(input.join(&file.relative));
            captured.push(witness);
        }
        let output = storage
            .create_child("output")
            .map_err(|_| GenerationError::Storage)?
            .path;
        let cache = storage
            .create_child("cache")
            .map_err(|_| GenerationError::Storage)?
            .path;
        let temporary = storage
            .create_child("temporary")
            .map_err(|_| GenerationError::Storage)?
            .path;
        let result = Self {
            binding: expected.clone(),
            layout: GenerationLayout {
                main: input.join(&current.main),
                input,
                inputs,
                output,
                cache,
                temporary,
            },
            storage,
            original,
            sources,
            snapshot,
            captured,
        };
        result.validate_files()?;
        check_cancel(cancel)?;
        Ok(result)
    }
    #[cfg(target_os = "macos")]
    fn validate_files(&self) -> Result<(), GenerationError> {
        self.storage
            .revalidate()
            .map_err(|_| GenerationError::Storage)?;
        for source in &self.sources {
            source
                .revalidate(&self.original, MAX_INPUT_BYTES)
                .map_err(|_| GenerationError::SourceChanged)?;
        }
        self.snapshot
            .revalidate()
            .map_err(|_| GenerationError::SnapshotChanged)?;
        for captured in &self.captured {
            captured
                .revalidate(&self.snapshot, MAX_INPUT_BYTES)
                .map_err(|_| GenerationError::SnapshotChanged)?;
        }
        Ok(())
    }
    pub fn prepare<A: HostInputAuthority, F>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        configure: F,
    ) -> Result<PreparedGeneration, GenerationError>
    where
        F: FnOnce(&GenerationLayout) -> Result<HostExecutionConfig, GenerationError>,
    {
        #[cfg(target_os = "macos")]
        {
            check_cancel(cancel)?;
            let _host = lease(authority, &self.binding)?;
            self.validate_files()?;
            let config = configure(&self.layout)?;
            let expected: BTreeSet<_> = self.layout.inputs.iter().collect();
            if config.project_root != self.layout.input
                || config.project_inputs.len() != expected.len()
                || config.project_inputs.iter().collect::<BTreeSet<_>>() != expected
                || config.writable.output != self.layout.output
                || config.writable.cache != self.layout.cache
                || config.writable.temporary != self.layout.temporary
            {
                return Err(GenerationError::Configuration);
            }
            let original = self
                .original
                .path
                .canonicalize()
                .map_err(|_| GenerationError::SourceChanged)?;
            for root in &config.read_only_roots {
                let root = root
                    .canonicalize()
                    .map_err(|_| GenerationError::Configuration)?;
                if root.starts_with(&original) || original.starts_with(&root) {
                    return Err(GenerationError::Configuration);
                }
            }
            if let ExecutionEnvironment::TexLive { font_config, .. } = &config.environment {
                if !self.layout.inputs.contains(font_config) {
                    return Err(GenerationError::Configuration);
                }
            }
            let output = capture_io::Directory::open(&self.layout.output, true)
                .map_err(|_| GenerationError::Storage)?;
            let runner = CompilerSandbox::new(config).map_err(GenerationError::Execution)?;
            Ok(PreparedGeneration {
                inputs: self,
                runner,
                output,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (authority, cancel, configure);
            Err(GenerationError::IsolationUnavailable)
        }
    }
}

pub struct PreparedGeneration {
    inputs: CapturedInputs,
    runner: CompilerSandbox,
    #[cfg(target_os = "macos")]
    output: capture_io::Directory,
}
pub struct PendingOutput {
    inputs: CapturedInputs,
    #[cfg(target_os = "macos")]
    output: capture_io::Directory,
    #[cfg(target_os = "macos")]
    witness: capture_io::FileWitness,
    #[cfg(target_os = "macos")]
    _reservation: capture_io::OutputSlot,
}
/// A current candidate's bytes. PDF parsing is a separate host publication
/// concern; process success and content freshness are not PDF validity.
pub struct CurrentArtifact<'a> {
    binding: &'a InputBinding,
    bytes: &'a [u8],
    digest: Digest,
    run: &'a Path,
}
impl CurrentArtifact<'_> {
    pub fn binding(&self) -> &InputBinding {
        self.binding
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }
    pub fn content_digest(&self) -> Digest {
        self.digest
    }
    pub fn run_directory(&self) -> &Path {
        self.run
    }
}
#[cfg(all(test, target_os = "macos"))]
enum GenerationPoint {
    PrelaunchValidated,
    FinalOutputRead,
}
impl PreparedGeneration {
    pub fn run<A: HostInputAuthority>(
        self,
        authority: &A,
        cancel: &AtomicBool,
    ) -> Result<PendingOutput, GenerationError> {
        #[cfg(target_os = "macos")]
        {
            self.run_inner(
                authority,
                cancel,
                #[cfg(test)]
                None,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (authority, cancel);
            Err(GenerationError::IsolationUnavailable)
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    fn run_with_hook<A: HostInputAuthority>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        hook: &mut dyn FnMut(GenerationPoint),
    ) -> Result<PendingOutput, GenerationError> {
        self.run_inner(authority, cancel, Some(hook))
    }
    #[cfg(target_os = "macos")]
    fn run_inner<A: HostInputAuthority>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        #[cfg(test)] mut hook: Option<&mut dyn FnMut(GenerationPoint)>,
    ) -> Result<PendingOutput, GenerationError> {
        check_cancel(cancel)?;
        let mut relative = PathBuf::from(
            self.inputs
                .layout
                .main
                .file_name()
                .ok_or(GenerationError::Configuration)?,
        );
        relative.set_extension("pdf");
        let mut reservation = None;
        let outcome = self
            .runner
            .run_guarded(cancel, || {
                check_cancel(cancel)?;
                let host = lease(authority, &self.inputs.binding)?;
                self.inputs.validate_files()?;
                #[cfg(test)]
                if let Some(callback) = hook.as_mut() {
                    callback(GenerationPoint::PrelaunchValidated);
                }
                self.inputs.validate_files()?;
                check_cancel(cancel)?;
                reservation = Some(
                    self.output
                        .reserve_output(&relative)
                        .map_err(|_| GenerationError::OutputChanged)?,
                );
                Ok(host)
            })
            .map_err(|error| match error {
                GuardedRunError::Execution(error) => GenerationError::Execution(error),
                GuardedRunError::Gate(error) => error,
            })?;
        if outcome.state != ExecutionState::Succeeded {
            return Err(GenerationError::CompilerFailed(outcome.state));
        }
        check_cancel(cancel)?;
        let reservation = reservation.ok_or(GenerationError::OutputChanged)?;
        let witness = reservation
            .completed(&self.output, &relative, 16 * 1024 * 1024)
            .map_err(|_| GenerationError::OutputChanged)?;
        Ok(PendingOutput {
            inputs: self.inputs,
            output: self.output,
            witness,
            _reservation: reservation,
        })
    }
}
impl PendingOutput {
    pub fn with_current_result<A: HostInputAuthority, R, F>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        publish: F,
    ) -> Result<R, GenerationError>
    where
        F: FnOnce(CurrentArtifact<'_>) -> R,
    {
        #[cfg(target_os = "macos")]
        {
            self.accept_inner(
                authority,
                cancel,
                publish,
                #[cfg(test)]
                None,
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (authority, cancel, publish);
            Err(GenerationError::IsolationUnavailable)
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    fn with_current_result_and_hook<A: HostInputAuthority, R, F>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        publish: F,
        hook: &mut dyn FnMut(GenerationPoint),
    ) -> Result<R, GenerationError>
    where
        F: FnOnce(CurrentArtifact<'_>) -> R,
    {
        self.accept_inner(authority, cancel, publish, Some(hook))
    }
    #[cfg(target_os = "macos")]
    fn accept_inner<A: HostInputAuthority, R, F>(
        self,
        authority: &A,
        cancel: &AtomicBool,
        publish: F,
        #[cfg(test)] mut hook: Option<&mut dyn FnMut(GenerationPoint)>,
    ) -> Result<R, GenerationError>
    where
        F: FnOnce(CurrentArtifact<'_>) -> R,
    {
        check_cancel(cancel)?;
        let host = lease(authority, &self.inputs.binding)?;
        self.inputs.validate_files()?;
        let bytes = self
            .witness
            .revalidate(&self.output, 16 * 1024 * 1024)
            .map_err(|_| GenerationError::OutputChanged)?;
        #[cfg(test)]
        if let Some(callback) = hook.as_mut() {
            callback(GenerationPoint::FinalOutputRead);
        }
        self.inputs.validate_files()?;
        self.witness
            .revalidate(&self.output, 16 * 1024 * 1024)
            .map_err(|_| GenerationError::OutputChanged)?;
        check_cancel(cancel)?;
        let result = publish(CurrentArtifact {
            binding: &self.inputs.binding,
            bytes: &bytes,
            digest: self.witness.digest,
            run: &self.inputs.storage.path,
        });
        drop(host);
        Ok(result)
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "generation_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "generation_tex_tests.rs"]
mod tex_tests;
