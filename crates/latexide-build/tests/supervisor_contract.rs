#![cfg(unix)]
use latexide_build::{Backend, BuildRequest, BuildState, BuildSupervisor};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}, thread, time::{Duration, Instant}};
use std::os::unix::fs::PermissionsExt;

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new(script: &str) -> Self {
        let root = std::env::temp_dir().join(format!("latexide-build-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("main.tex"), "controlled fixture").unwrap();
        fs::write(root.join("compiler"), format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(root.join("compiler"), fs::Permissions::from_mode(0o755)).unwrap();
        Self(root)
    }
    fn request(&self, timeout: Duration) -> BuildRequest {
        BuildRequest { root: self.0.clone(), main_file: "main.tex".into(), compiler_path: self.0.join("compiler"),
            cache_directory: self.0.join("cache"), output_directory: self.0.join("builds"), timeout, backend: Backend::Tectonic }
    }
}
impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn terminal(supervisor: &BuildSupervisor, id: &str) -> latexide_build::BuildStatus {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = supervisor.status(id).unwrap();
        if status.state.is_terminal() { return status; }
        assert!(Instant::now() < deadline, "Compiler lifecycle did not finish within the test deadline");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn compiler_exit_failure_retains_diagnostics() {
    let project = Project::new("echo 'error: main.tex:1: controlled error' >&2\nexit 7");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_secs(2))).unwrap();
    let status = terminal(&supervisor, &id);
    assert_eq!(status.state, BuildState::Failed);
    assert_eq!(status.exit_code, Some(7));
    assert!(status.log.contains("controlled error"));
    assert!(!status.diagnostics.is_empty());
    assert!(status.pdf_path.is_none());
}

#[test]
fn timeout_terminates_the_parent_and_descendant_holding_output_open() {
    let project = Project::new("sleep 30 &\nwait");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_millis(150))).unwrap();
    assert_eq!(terminal(&supervisor, &id).state, BuildState::TimedOut);
}

#[test]
fn cancelling_and_dropping_a_build_do_not_leave_waiting_children() {
    let project = Project::new("echo ready\nsleep 30 &\nwait");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_secs(60))).unwrap();
    supervisor.cancel(&id).unwrap();
    assert_eq!(terminal(&supervisor, &id).state, BuildState::Cancelled);
    supervisor.start(project.request(Duration::from_secs(60))).unwrap();
    let start = Instant::now();
    drop(supervisor);
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn workspace_cannot_run_two_compilers_into_the_same_state() {
    let project = Project::new("sleep 30 &\nwait");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_secs(60))).unwrap();
    assert_eq!(supervisor.start(project.request(Duration::from_secs(60))).unwrap_err().code, "build_busy");
    supervisor.cancel(&id).unwrap();
    terminal(&supervisor, &id);
}

#[test]
fn an_active_compiler_is_reported_as_running() {
    let project = Project::new("sleep 30 &\nwait");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_secs(60))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if supervisor.status(&id).unwrap().state == BuildState::Running { break; }
        assert!(Instant::now() < deadline, "A running compiler must not stay in queued state");
        thread::sleep(Duration::from_millis(5));
    }
    supervisor.cancel(&id).unwrap();
    terminal(&supervisor, &id);
}

#[test]
fn failed_build_does_not_relabel_previous_pdf_as_fresh() {
    let project = Project::new("out=''\nwhile [ \"$#\" -gt 0 ]; do\n if [ \"$1\" = '--outdir' ]; then shift; out=\"$1\"; fi\n shift\ndone\nprintf '%%PDF-1.5\\nfixture\\n' > \"$out/main.pdf\"");
    let mut supervisor = BuildSupervisor::new();
    let first = supervisor.start(project.request(Duration::from_secs(2))).unwrap();
    let success = terminal(&supervisor, &first);
    assert_eq!(success.state, BuildState::Succeeded);
    let previous = success.pdf_path.unwrap();
    fs::write(project.0.join("compiler"), "#!/bin/sh\nexit 2\n").unwrap();
    let second = supervisor.start(project.request(Duration::from_secs(2))).unwrap();
    let failure = terminal(&supervisor, &second);
    assert_eq!(failure.state, BuildState::Failed);
    assert!(failure.pdf_path.is_none());
    assert_eq!(supervisor.last_successful_pdf(), Some(previous.clone()));
    assert!(previous.exists());
}

#[test]
fn dotted_source_filename_retains_the_complete_pdf_stem() {
    let project = Project::new("out=''\nwhile [ \"$#\" -gt 0 ]; do\n if [ \"$1\" = '--outdir' ]; then shift; out=\"$1\"; fi\n shift\ndone\nprintf '%%PDF-1.5\\nfixture\\n' > \"$out/article.v1.pdf\"");
    fs::write(project.0.join("article.v1.tex"), "controlled fixture").unwrap();
    let mut request = project.request(Duration::from_secs(2));
    request.main_file = "article.v1.tex".into();
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(request).unwrap();
    let result = terminal(&supervisor, &id);
    assert_eq!(result.state, BuildState::Succeeded);
    assert_eq!(result.pdf_path.unwrap().file_name().unwrap(), "article.v1.pdf");
}

#[test]
fn real_xelatex_file_line_error_is_available_as_a_diagnostic() {
    let project = Project::new("echo './main.tex:4: Package fontspec Error: The font cannot be found.' >&2\nexit 1");
    let mut supervisor = BuildSupervisor::new();
    let id = supervisor.start(project.request(Duration::from_secs(2))).unwrap();
    let result = terminal(&supervisor, &id);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].file.as_deref(), Some("./main.tex"));
    assert_eq!(result.diagnostics[0].line, Some(4));
    assert_eq!(result.diagnostics[0].severity, "error");
}
