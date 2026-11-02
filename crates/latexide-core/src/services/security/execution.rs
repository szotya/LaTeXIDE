//! Host-owned offline launch adapter. No wire DTO or application registration.
//! A successful policy grant alone never selects an unrestricted execution path.
use std::{ffi::OsString, path::PathBuf, sync::atomic::AtomicBool, time::Duration};

#[derive(Clone)]
pub struct WritableRoots {
    pub output: PathBuf,
    pub cache: PathBuf,
    pub temporary: PathBuf,
}
#[derive(Clone)]
pub struct HostCommand {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}
#[derive(Clone)]
pub enum ExecutionEnvironment {
    Minimal,
    TexLive {
        prefix: PathBuf,
        font_config: PathBuf,
    },
}
/// Construct only from host configuration, never from client/manifest data.
#[derive(Clone)]
pub struct HostExecutionConfig {
    pub trusted: bool,
    pub project_root: PathBuf,
    /// Explicit host-owned build inputs; never infer read authority from a path
    /// supplied by the TeX source. A future P09 manifest must bind these inputs.
    pub project_inputs: Vec<PathBuf>,
    pub read_only_roots: Vec<PathBuf>,
    pub writable: WritableRoots,
    pub command: HostCommand,
    pub executable_allowlist: Vec<PathBuf>,
    pub environment: ExecutionEnvironment,
    pub sandbox_executable: PathBuf,
    pub timeout: Duration,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionError {
    Untrusted,
    InvalidConfiguration,
    IsolationUnavailable,
    IdentityChanged,
    OutputNotFresh,
    SpawnFailed,
    SupervisionFailed,
    EvidenceChanged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionState {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}
#[derive(Debug)]
pub struct ExecutionOutcome {
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub stdout_log: Option<PathBuf>,
    pub stderr_log: Option<PathBuf>,
    pub profile: Option<PathBuf>,
}

pub struct CompilerSandbox {
    #[cfg(target_os = "macos")]
    prepared: macos::Prepared,
}
pub(super) enum GuardedRunError<E> {
    Execution(ExecutionError),
    Gate(E),
}
impl<E> From<ExecutionError> for GuardedRunError<E> {
    fn from(error: ExecutionError) -> Self {
        Self::Execution(error)
    }
}
impl CompilerSandbox {
    pub fn new(config: HostExecutionConfig) -> Result<Self, ExecutionError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                prepared: macos::Prepared::new(config)?,
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = config;
            Err(ExecutionError::IsolationUnavailable)
        }
    }
    pub fn run(&self, cancelled: &AtomicBool) -> Result<ExecutionOutcome, ExecutionError> {
        #[cfg(target_os = "macos")]
        {
            self.prepared.run(cancelled, &self.prepared.profile)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = cancelled;
            Err(ExecutionError::IsolationUnavailable)
        }
    }
    /// Host-only generation gate. Its guard remains alive through the actual
    /// spawn, then releases so edits can continue during compilation.
    pub(super) fn run_guarded<E, G, F>(
        &self,
        cancelled: &AtomicBool,
        gate: F,
    ) -> Result<ExecutionOutcome, GuardedRunError<E>>
    where
        F: FnOnce() -> Result<G, E>,
    {
        #[cfg(target_os = "macos")]
        {
            self.prepared
                .run_guarded(cancelled, &self.prepared.profile, gate)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (cancelled, gate);
            Err(GuardedRunError::Execution(
                ExecutionError::IsolationUnavailable,
            ))
        }
    }
    #[cfg(all(test, target_os = "macos"))]
    pub(super) fn test_blueprint(&self) -> Result<std::process::Command, ExecutionError> {
        self.prepared.command(&self.prepared.profile)
    }
}

#[cfg(all(test, target_os = "macos"))]
std::thread_local! {
    static COMMAND_CAPTURE:std::cell::RefCell<Option<std::path::PathBuf>>=const {std::cell::RefCell::new(None)};
}
/// Test-only observation of the actual Command at the spawn site. It grants no
/// execution authority and cannot alter arguments, environment, or the gate.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn with_test_command_capture<R>(prefix: &std::path::Path, run: impl FnOnce() -> R) -> R {
    assert!(prefix.is_absolute());
    COMMAND_CAPTURE.with(|capture| {
        assert!(capture.borrow().is_none());
        *capture.borrow_mut() = Some(prefix.into());
    });
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COMMAND_CAPTURE.with(|capture| *capture.borrow_mut() = None);
        }
    }
    let _reset = Reset;
    run()
}
#[cfg(all(test, target_os = "macos"))]
fn record_test_command(command: &std::process::Command) -> std::io::Result<()> {
    use std::{
        fs::OpenOptions,
        io::Write,
        os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    };
    COMMAND_CAPTURE.with(|capture| {
        let Some(prefix) = capture.borrow().clone() else {
            return Ok(());
        };
        let create = |suffix: &str| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(0x2000_0000)
                .open(prefix.with_extension(suffix))
        };
        let mut argv = create("argv.nul")?;
        for argument in std::iter::once(command.get_program()).chain(command.get_args()) {
            argv.write_all(argument.as_bytes())?;
            argv.write_all(&[0])?;
        }
        let mut env = create("env.nul")?;
        for (key, value) in command.get_envs() {
            if let Some(value) = value {
                env.write_all(key.as_bytes())?;
                env.write_all(&[0])?;
                env.write_all(value.as_bytes())?;
                env.write_all(&[0])?;
            }
        }
        let mut cwd = create("cwd.nul")?;
        if let Some(path) = command.get_current_dir() {
            cwd.write_all(path.as_os_str().as_bytes())?;
            cwd.write_all(&[0])?;
        }
        Ok(())
    })
}

#[cfg(target_os = "macos")]
#[path = "child_exec.rs"]
mod child_exec;

#[cfg(target_os = "macos")]
mod macos {
    use super::super::{ProjectSecurity, identity::PinnedPath};
    use super::*;
    use latexide_contracts::security::{ExecutionPolicy, ResourceRequest, SecurityPort};
    use std::{
        collections::BTreeSet,
        fs::{self, File, OpenOptions},
        io::{self, Read, Write},
        os::unix::{
            fs::{MetadataExt, OpenOptionsExt},
            process::CommandExt,
        },
        path::Path,
        process::{Child, Command, Stdio},
        sync::atomic::Ordering,
        time::Instant,
    };

    const LOG_FILE_LIMIT: u64 = 16 * 1024 * 1024;
    struct Watch {
        configured: PathBuf,
        canonical: PathBuf,
        identity: PinnedPath,
    }
    impl Watch {
        fn new(path: &Path, directory: bool) -> Result<Self, ExecutionError> {
            if !path.is_absolute()
                || path
                    .components()
                    .any(|p| matches!(p, std::path::Component::ParentDir))
                || path
                    .to_str()
                    .is_none_or(|s| s.chars().any(char::is_control))
            {
                return Err(ExecutionError::InvalidConfiguration);
            }
            let canonical = path
                .canonicalize()
                .map_err(|_| ExecutionError::IdentityChanged)?;
            let metadata = fs::metadata(&canonical).map_err(|_| ExecutionError::IdentityChanged)?;
            if if directory {
                !metadata.is_dir()
            } else {
                !metadata.is_file()
            } {
                return Err(ExecutionError::InvalidConfiguration);
            }
            let identity =
                PinnedPath::new(&canonical).map_err(|_| ExecutionError::IdentityChanged)?;
            Ok(Self {
                configured: path.into(),
                canonical,
                identity,
            })
        }
        fn revalidate(&self) -> Result<(), ExecutionError> {
            if self
                .configured
                .canonicalize()
                .map_err(|_| ExecutionError::IdentityChanged)?
                != self.canonical
            {
                return Err(ExecutionError::IdentityChanged);
            }
            self.identity
                .revalidate()
                .map_err(|_| ExecutionError::IdentityChanged)
        }
    }
    pub(super) struct Prepared {
        config: HostExecutionConfig,
        watches: Vec<Watch>,
        _policy: ProjectSecurity,
        pub(super) profile: String,
    }
    fn literal(path: &Path) -> Result<String, ExecutionError> {
        let text = path.to_str().ok_or(ExecutionError::InvalidConfiguration)?;
        if text.chars().any(char::is_control) {
            return Err(ExecutionError::InvalidConfiguration);
        }
        Ok(format!(
            "\"{}\"",
            text.replace('\\', "\\\\").replace('"', "\\\"")
        ))
    }
    fn cancelled_outcome() -> ExecutionOutcome {
        ExecutionOutcome {
            state: ExecutionState::Cancelled,
            exit_code: None,
            stdout_log: None,
            stderr_log: None,
            profile: None,
        }
    }
    impl Prepared {
        pub(super) fn new(mut config: HostExecutionConfig) -> Result<Self, ExecutionError> {
            if !config.trusted {
                return Err(ExecutionError::Untrusted);
            }
            // Only this OS backend is supported; a host override cannot substitute
            // a success-only utility or an unrestricted launcher.
            if config.sandbox_executable != Path::new("/usr/bin/sandbox-exec")
                || !config.sandbox_executable.is_file()
            {
                return Err(ExecutionError::IsolationUnavailable);
            }
            if config.timeout.is_zero()
                || config.timeout > Duration::from_secs(600)
                || config.read_only_roots.len() > 32
                || config.project_inputs.len() > 1024
                || config.executable_allowlist.is_empty()
                || config.executable_allowlist.len() > 32
                || config
                    .command
                    .arguments
                    .iter()
                    .map(|a| a.len())
                    .sum::<usize>()
                    > 64 * 1024
            {
                return Err(ExecutionError::InvalidConfiguration);
            }
            let mut watches = vec![Watch::new(&config.sandbox_executable, false)?];
            let project = Watch::new(&config.project_root, true)?;
            config.project_root = project.canonical.clone();
            watches.push(project);
            let mut writable = Vec::new();
            for path in [
                &mut config.writable.output,
                &mut config.writable.cache,
                &mut config.writable.temporary,
            ] {
                let watch = Watch::new(path, true)?;
                unsafe extern "C" {
                    fn geteuid() -> u32;
                }
                let metadata =
                    fs::metadata(&watch.canonical).map_err(|_| ExecutionError::IdentityChanged)?;
                if metadata.uid() != unsafe { geteuid() }
                    || metadata.mode() & 0o077 != 0
                    || config.project_root.starts_with(&watch.canonical)
                {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                // This standalone slice leases fresh private roots per run.
                // Do not inherit an outside hardlink from a reused cache/output.
                if fs::read_dir(&watch.canonical)
                    .map_err(|_| ExecutionError::InvalidConfiguration)?
                    .next()
                    .is_some()
                {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                *path = watch.canonical.clone();
                writable.push(watch.canonical.clone());
                watches.push(watch);
            }
            for (i, left) in writable.iter().enumerate() {
                if writable
                    .iter()
                    .skip(i + 1)
                    .any(|right| left.starts_with(right) || right.starts_with(left))
                {
                    return Err(ExecutionError::InvalidConfiguration);
                }
            }
            for path in &mut config.read_only_roots {
                let watch = Watch::new(path, true)?;
                if watch.canonical == Path::new("/")
                    || watch.canonical.starts_with(&config.project_root)
                    || config.project_root.starts_with(&watch.canonical)
                    || writable
                        .iter()
                        .any(|w| w.starts_with(&watch.canonical) || watch.canonical.starts_with(w))
                {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                *path = watch.canonical.clone();
                watches.push(watch);
            }
            let mut executable_paths = BTreeSet::new();
            let mut command_allowed = false;
            for path in &config.executable_allowlist {
                let watch = Watch::new(path, false)?;
                let system_tool = ["/usr/bin/perl", "/bin/sh", "/bin/bash", "/usr/bin/env"]
                    .iter()
                    .any(|p| watch.canonical == Path::new(p));
                if !system_tool
                    && !config
                        .read_only_roots
                        .iter()
                        .any(|r| watch.canonical.starts_with(r))
                {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                if writable.iter().any(|w| watch.canonical.starts_with(w)) {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                if path == &config.command.executable {
                    command_allowed = true;
                }
                executable_paths.insert(path.clone());
                executable_paths.insert(watch.canonical.clone());
                watches.push(watch);
            }
            if !command_allowed {
                return Err(ExecutionError::InvalidConfiguration);
            }
            // On this macOS /bin/sh is a selector stub. Bind its single public
            // system selector and require the selected interpreter explicitly;
            // never grant an arbitrary interpreter directory or a fallback.
            let shell_selector = executable_paths.contains(Path::new("/bin/sh"));
            if shell_selector {
                let selector = Watch::new(Path::new("/private/var/select/sh"), false)?;
                let link = fs::symlink_metadata("/private/var/select/sh")
                    .map_err(|_| ExecutionError::IdentityChanged)?;
                let target = fs::metadata(&selector.canonical)
                    .map_err(|_| ExecutionError::IdentityChanged)?;
                if link.uid() != 0 || target.uid() != 0 || target.mode() & 0o022 != 0 {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                if !executable_paths.contains(&selector.canonical) {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                watches.push(selector);
            }
            match &mut config.environment {
                ExecutionEnvironment::Minimal => (),
                ExecutionEnvironment::TexLive {
                    prefix,
                    font_config,
                } => {
                    *prefix = prefix
                        .canonicalize()
                        .map_err(|_| ExecutionError::IdentityChanged)?;
                    if !config.read_only_roots.contains(prefix) {
                        return Err(ExecutionError::InvalidConfiguration);
                    }
                    let font = Watch::new(font_config, false)?;
                    if !font.canonical.starts_with(&config.project_root)
                        && !writable.iter().any(|w| font.canonical.starts_with(w))
                    {
                        return Err(ExecutionError::InvalidConfiguration);
                    }
                    *font_config = font.canonical.clone();
                    watches.push(font);
                }
            }
            let resource_policy = ExecutionPolicy {
                project_root: config
                    .project_root
                    .to_str()
                    .ok_or(ExecutionError::InvalidConfiguration)?
                    .into(),
                writable_roots: writable
                    .iter()
                    .map(|p| {
                        p.to_str()
                            .map(str::to_owned)
                            .ok_or(ExecutionError::InvalidConfiguration)
                    })
                    .collect::<Result<_, _>>()?,
                trusted: config.trusted,
                network: false,
            };
            let policy = ProjectSecurity::new(&resource_policy)
                .map_err(|_| ExecutionError::IdentityChanged)?;
            let mut input_files = BTreeSet::new();
            let mut input_directories = BTreeSet::from([config.project_root.clone()]);
            for input in &mut config.project_inputs {
                let grant = policy
                    .authorize_resource(
                        &resource_policy,
                        ResourceRequest {
                            path: input
                                .to_str()
                                .ok_or(ExecutionError::InvalidConfiguration)?
                                .into(),
                            write: false,
                            network: false,
                        },
                    )
                    .map_err(|_| ExecutionError::IdentityChanged)?;
                let canonical = PathBuf::from(grant.canonical_path);
                if !canonical.starts_with(&config.project_root) {
                    return Err(ExecutionError::InvalidConfiguration);
                }
                let watch = Watch::new(input, false)?;
                if watch.canonical != canonical {
                    return Err(ExecutionError::IdentityChanged);
                }
                for parent in canonical
                    .ancestors()
                    .skip(1)
                    .take_while(|p| p.starts_with(&config.project_root))
                {
                    input_directories.insert(parent.to_path_buf());
                }
                input_files.insert(canonical.clone());
                *input = canonical;
                watches.push(watch);
            }
            let mut profile = String::from(
                "(version 1) (deny default) (deny file-link file-clone) (import \"dyld-support.sb\")\n(allow sysctl-read)\n(allow process-fork)\n",
            );
            for path in &executable_paths {
                profile.push_str(&format!("(allow process-exec (literal {}))\n(allow file-read* file-map-executable (literal {}))\n", literal(path)?,literal(path)?));
            }
            // Deliberately avoid broad /System (which also names Data aliases).
            for path in [
                "/System/Library",
                "/usr/lib",
                "/usr/share/locale",
                "/usr/share/zoneinfo",
            ] {
                profile.push_str(&format!(
                    "(allow file-read* file-map-executable (subpath {}))\n",
                    literal(Path::new(path))?
                ));
            }
            for path in ["/dev/null", "/dev/urandom", "/dev/random"] {
                profile.push_str(&format!(
                    "(allow file-read* (literal {}))\n",
                    literal(Path::new(path))?
                ));
            }
            if shell_selector {
                profile.push_str("(allow file-read* (literal \"/private/var/select/sh\"))\n");
            }
            // Untrusted project data is literal-file-only. A subtree read grant
            // would also grant a pre-existing hardlink to an outside file.
            profile.push_str(&format!("(allow file-read-metadata (subpath {}))\n(allow file-read-metadata (path-ancestors {}))\n",literal(&config.project_root)?,literal(&config.project_root)?));
            for path in input_files.iter().chain(input_directories.iter()) {
                profile.push_str(&format!(
                    "(allow file-read* (literal {}))\n",
                    literal(path)?
                ));
            }
            for path in config.read_only_roots.iter().chain(writable.iter()) {
                profile.push_str(&format!("(allow file-read* (subpath {}))\n(allow file-read-metadata (path-ancestors {}))\n",literal(path)?,literal(path)?));
            }
            for path in &writable {
                profile.push_str(&format!(
                    "(allow file-write* (subpath {}))\n",
                    literal(path)?
                ));
            }
            if profile.len() > 64 * 1024 {
                return Err(ExecutionError::InvalidConfiguration);
            }
            Ok(Self {
                config,
                watches,
                _policy: policy,
                profile,
            })
        }
        fn revalidate_fresh_writers(&self) -> Result<(), ExecutionError> {
            for (root, output) in [
                (&self.config.writable.output, true),
                (&self.config.writable.cache, false),
                (&self.config.writable.temporary, false),
            ] {
                for entry in fs::read_dir(root).map_err(|_| ExecutionError::IdentityChanged)? {
                    let entry = entry.map_err(|_| ExecutionError::IdentityChanged)?;
                    let expected = output
                        && entry.file_name().to_str().is_some_and(|name| {
                            [
                                "isolation.profile.sb",
                                "compiler.stdout.log",
                                "compiler.stderr.log",
                            ]
                            .contains(&name)
                        });
                    let metadata = fs::symlink_metadata(entry.path())
                        .map_err(|_| ExecutionError::IdentityChanged)?;
                    if !expected
                        || !metadata.is_file()
                        || metadata.is_symlink()
                        || metadata.nlink() != 1
                    {
                        return Err(ExecutionError::IdentityChanged);
                    }
                }
            }
            Ok(())
        }
        pub(super) fn command(&self, profile: &str) -> Result<Command, ExecutionError> {
            let mut command = Command::new(&self.config.sandbox_executable);
            command
                .args(["-p", profile])
                .arg(&self.config.command.executable)
                .args(&self.config.command.arguments)
                .current_dir(&self.config.project_root)
                .env_clear()
                .stdin(Stdio::null())
                .process_group(0)
                .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
                .env("LANG", "en_US.UTF-8")
                .env("TZ", "UTC")
                .env("TMPDIR", &self.config.writable.temporary);
            // HOME is omitted, never redirected; no inherited startup, credential,
            // TeX search, Perl, loader or agent variables reach the compiler.
            if let ExecutionEnvironment::TexLive {
                prefix,
                font_config,
            } = &self.config.environment
            {
                let bin = prefix.join("bin/universal-darwin");
                command
                    .env(
                        "PATH",
                        std::env::join_paths([
                            bin.as_path(),
                            Path::new("/usr/bin"),
                            Path::new("/bin"),
                            Path::new("/usr/sbin"),
                            Path::new("/sbin"),
                        ])
                        .map_err(|_| ExecutionError::InvalidConfiguration)?,
                    )
                    .env("TEXMFSYSVAR", prefix.join("texmf-var"))
                    .env("TEXMFSYSCONFIG", prefix.join("texmf-config"))
                    .env("FONTCONFIG_FILE", font_config)
                    .env(
                        "FONTCONFIG_PATH",
                        font_config
                            .parent()
                            .ok_or(ExecutionError::InvalidConfiguration)?,
                    );
                for (key, tail) in [
                    ("TEXMFHOME", "texmf-home"),
                    ("TEXMFVAR", "texmf-var"),
                    ("TEXMFCONFIG", "texmf-config"),
                    ("TEXMFCACHE", "texmf-cache"),
                    ("VARTEXFONTS", "fonts"),
                    ("XDG_CACHE_HOME", "xdg-cache"),
                    ("XDG_CONFIG_HOME", "xdg-config"),
                ] {
                    command.env(key, self.config.writable.cache.join(tail));
                }
                command.env("MKTEXFMT", "0");
            }
            // Darwin SDK resource.h: 64-bit rlim_t. Apply before the sandbox utility
            // and every target instruction. These are per-process/per-file limits.
            #[repr(C)]
            struct Limit {
                current: u64,
                maximum: u64,
            }
            unsafe extern "C" {
                fn setrlimit(resource: i32, limit: *const Limit) -> i32;
            }
            unsafe {
                command.pre_exec(|| {
                    for (resource, value) in [(1, LOG_FILE_LIMIT), (4, 0), (8, 128)] {
                        let limit = Limit {
                            current: value,
                            maximum: value,
                        };
                        if setrlimit(resource, &limit) != 0 {
                            return Err(io::Error::last_os_error());
                        }
                    }
                    Ok(())
                });
            }
            super::child_exec::install(&mut command).map_err(|_| ExecutionError::SpawnFailed)?;
            Ok(command)
        }
        pub(super) fn run(
            &self,
            cancelled: &AtomicBool,
            profile: &str,
        ) -> Result<ExecutionOutcome, ExecutionError> {
            match self.run_guarded(
                cancelled,
                profile,
                || Ok::<(), std::convert::Infallible>(()),
            ) {
                Ok(outcome) => Ok(outcome),
                Err(GuardedRunError::Execution(error)) => Err(error),
                Err(GuardedRunError::Gate(never)) => match never {},
            }
        }
        pub(super) fn run_guarded<E, G, F>(
            &self,
            cancelled: &AtomicBool,
            profile: &str,
            gate: F,
        ) -> Result<ExecutionOutcome, GuardedRunError<E>>
        where
            F: FnOnce() -> Result<G, E>,
        {
            let started = Instant::now();
            if cancelled.load(Ordering::Acquire) {
                return Ok(cancelled_outcome());
            }
            for watch in &self.watches {
                watch.revalidate()?;
            }
            let profile_path = self.config.writable.output.join("isolation.profile.sb");
            let stdout_path = self.config.writable.output.join("compiler.stdout.log");
            let stderr_path = self.config.writable.output.join("compiler.stderr.log");
            let create = |path: &Path| {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .custom_flags(0x2000_0000)
                    .open(path)
                    .map_err(|_| ExecutionError::OutputNotFresh)
            };
            create(&profile_path)?
                .write_all(profile.as_bytes())
                .map_err(|_| ExecutionError::OutputNotFresh)?;
            let stdout = create(&stdout_path)?;
            let stderr = create(&stderr_path)?;
            let mut command = self.command(profile)?;
            command
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr));
            for watch in &self.watches {
                watch.revalidate()?;
            }
            self.revalidate_fresh_writers()?;
            if cancelled.load(Ordering::Acquire) {
                return Ok(cancelled_outcome());
            }
            if started.elapsed() >= self.config.timeout {
                return Ok(ExecutionOutcome {
                    state: ExecutionState::TimedOut,
                    exit_code: None,
                    stdout_log: Some(stdout_path),
                    stderr_log: Some(stderr_path),
                    profile: Some(profile_path),
                });
            }
            let host_guard = gate().map_err(GuardedRunError::Gate)?;
            if cancelled.load(Ordering::Acquire) {
                return Ok(cancelled_outcome());
            }
            if started.elapsed() >= self.config.timeout {
                return Ok(ExecutionOutcome {
                    state: ExecutionState::TimedOut,
                    exit_code: None,
                    stdout_log: Some(stdout_path),
                    stderr_log: Some(stderr_path),
                    profile: Some(profile_path),
                });
            }
            #[cfg(test)]
            super::record_test_command(&command).map_err(|_| ExecutionError::EvidenceChanged)?;
            let child = command.spawn().map_err(|_| ExecutionError::SpawnFailed)?;
            drop(host_guard);
            let mut tree = Tree {
                child,
                reaped: false,
            };
            let state = loop {
                if cancelled.load(Ordering::Acquire) {
                    break ExecutionState::Cancelled;
                }
                if started.elapsed() >= self.config.timeout {
                    break ExecutionState::TimedOut;
                }
                if tree.exited_without_reaping()? {
                    break ExecutionState::Succeeded;
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            // The main PID remains reserved until group termination, avoiding a
            // kill-after-reap PID/group reuse race. There is no unrestricted retry.
            tree.terminate();
            let status = tree
                .child
                .wait()
                .map_err(|_| ExecutionError::SupervisionFailed)?;
            tree.reaped = true;
            let mut content = Vec::new();
            File::open(&profile_path)
                .map_err(|_| ExecutionError::EvidenceChanged)?
                .take(profile.len() as u64 + 1)
                .read_to_end(&mut content)
                .map_err(|_| ExecutionError::EvidenceChanged)?;
            if content != profile.as_bytes() {
                return Err(ExecutionError::EvidenceChanged.into());
            }
            let state = if state == ExecutionState::Succeeded && !status.success() {
                ExecutionState::Failed
            } else {
                state
            };
            Ok(ExecutionOutcome {
                state,
                exit_code: status.code(),
                stdout_log: Some(stdout_path),
                stderr_log: Some(stderr_path),
                profile: Some(profile_path),
            })
        }
    }

    struct Tree {
        child: Child,
        reaped: bool,
    }
    impl Tree {
        fn terminate(&mut self) {
            unsafe extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if let Ok(pid) = i32::try_from(self.child.id()) {
                if pid > 1 {
                    let _ = unsafe { kill(-pid, 9) };
                }
            }
            let _ = self.child.kill();
        }
        fn exited_without_reaping(&self) -> Result<bool, ExecutionError> {
            // Darwin wait.h: P_PID=1, WNOHANG|WEXITED|WNOWAIT. siginfo's first
            // field is si_signo (signal.h); oversized aligned storage avoids
            // assuming the layout of its unused tail. No child.wait/try_wait here.
            #[repr(C, align(16))]
            struct Info([u8; 256]);
            unsafe extern "C" {
                fn waitid(kind: i32, id: u32, info: *mut std::ffi::c_void, options: i32) -> i32;
            }
            let mut info = Info([0; 256]);
            if unsafe { waitid(1, self.child.id(), info.0.as_mut_ptr().cast(), 1 | 4 | 32) } != 0 {
                return Err(ExecutionError::SupervisionFailed);
            }
            Ok(i32::from_ne_bytes(info.0[..4].try_into().unwrap()) != 0)
        }
    }
    impl Drop for Tree {
        fn drop(&mut self) {
            if !self.reaped {
                self.terminate();
                let _ = self.child.wait();
                self.reaped = true;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn drop_owned_child_for_test(child: Child) {
        drop(Tree {
            child,
            reaped: false,
        });
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "execution_tests.rs"]
mod tests;
