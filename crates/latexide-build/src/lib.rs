//! Supervised local compiler processes and immutable PDF generations.

extern crate self as latexide_build;
pub use latexide_contracts as contracts;

mod process;
use latexide_core::{CoreError, Result};
use serde::Serialize;
use std::{collections::VecDeque, fs::{self, File}, io::{Read, Write}, path::{Component, Path, PathBuf},
    process::{Command, Stdio}, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}},
    thread::{self, JoinHandle}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

static NEXT: AtomicU64 = AtomicU64::new(1);
const PREVIEW_LIMIT: usize = 128 * 1024;
const LOG_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub enum Backend { Tectonic, LatexMk { engine: Engine } }
#[derive(Debug, Clone, Copy)]
pub enum Engine { PdfLaTex, XeLaTex, LuaLaTex }

#[derive(Clone)]
pub struct BuildRequest {
    pub root: PathBuf,
    pub main_file: String,
    pub compiler_path: PathBuf,
    pub cache_directory: PathBuf,
    pub output_directory: PathBuf,
    pub timeout: Duration,
    pub backend: Backend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildState { Queued, Running, Succeeded, Failed, Cancelled, TimedOut }
impl BuildState { pub fn is_terminal(self) -> bool { matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled | Self::TimedOut) } }

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic { pub severity: String, pub file: Option<String>, pub line: Option<u32>, pub message: String }

#[derive(Debug, Clone, Serialize)]
pub struct BuildStatus {
    pub id: String,
    pub state: BuildState,
    pub exit_code: Option<i32>,
    pub pdf_path: Option<PathBuf>,
    pub log_path: PathBuf,
    pub log: String,
    pub log_truncated: bool,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Default)]
struct State { records: VecDeque<BuildStatus>, last_success: Option<PathBuf> }
pub struct BuildSupervisor { shared: Arc<Mutex<State>>, worker: Option<JoinHandle<()>>, cancel: Option<(String, Arc<AtomicBool>)> }
impl Default for BuildSupervisor { fn default() -> Self { Self::new() } }

impl BuildSupervisor {
    pub fn new() -> Self { Self { shared: Arc::new(Mutex::new(State::default())), worker: None, cancel: None } }
    pub fn status(&self, id: &str) -> Option<BuildStatus> { self.shared.lock().ok()?.records.iter().find(|record| record.id == id).cloned() }
    pub fn latest(&self) -> Option<BuildStatus> { self.shared.lock().ok()?.records.back().cloned() }
    pub fn last_successful_pdf(&self) -> Option<PathBuf> { self.shared.lock().ok()?.last_success.clone() }

    pub fn start(&mut self, request: BuildRequest) -> Result<String> {
        if self.latest().is_some_and(|record| !record.state.is_terminal()) {
            return Err(CoreError::new("build_busy", "Ehhez a projekthez már fut fordítás."));
        }
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
        let root = request.root.canonicalize()?;
        let relative = std::path::Path::new(&request.main_file);
        if relative.is_absolute() || relative.components().any(|component| matches!(component, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
            return Err(CoreError::new("path_escape", "A főfájlnak a projekten belül kell lennie."));
        }
        let source = root.join(relative).canonicalize()?;
        if !source.starts_with(&root) || !source.is_file() { return Err(CoreError::new("path_escape", "A főfájl nem a projekt része.")); }
        if !request.compiler_path.is_file() { return Err(CoreError::new("compiler_missing", "A beállított fordító nem található.")); }
        let id = format!("{}-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed));
        let output = request.output_directory.join(&id);
        fs::create_dir_all(&output)?;
        fs::create_dir_all(&request.cache_directory)?;
        let record = BuildStatus { id: id.clone(), state: BuildState::Queued, exit_code: None, pdf_path: None,
            log_path: output.join("compiler-output.log"), log: String::new(), log_truncated: false, diagnostics: Vec::new() };
        {
            let mut state = self.shared.lock().map_err(|_| CoreError::new("build_unavailable", "A fordítás állapota nem érhető el."))?;
            state.records.push_back(record);
            while state.records.len() > 8 { state.records.pop_front(); }
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some((id.clone(), Arc::clone(&cancel)));
        let shared = Arc::clone(&self.shared);
        let build_id = id.clone();
        self.worker = Some(thread::Builder::new().name("latexide-compiler".into()).spawn(move || {
            if let Ok(mut state) = shared.lock() {
                if let Some(record) = state.records.iter_mut().find(|record| record.id == build_id) { record.state = BuildState::Running; }
            }
            let result = execute(&request, &root, &source, &output, &cancel);
            let mut state = match shared.lock() { Ok(state) => state, Err(_) => return };
            if let Some(record) = state.records.iter_mut().find(|record| record.id == build_id) {
                match result {
                    Ok(outcome) => {
                        record.state = outcome.state;
                        record.exit_code = outcome.exit_code;
                        record.log = outcome.log;
                        record.log_truncated = outcome.log_truncated;
                        record.diagnostics = parse_diagnostics(&record.log);
                        if outcome.state == BuildState::Succeeded { record.pdf_path = Some(pdf_output(&output, &source)); }
                    }
                    Err(error) => { record.state = BuildState::Failed; record.log = error.to_string(); }
                }
                let success = record.pdf_path.clone();
                if success.is_some() { state.last_success = success; }
            }
        }).map_err(CoreError::from)?);
        Ok(id)
    }

    pub fn cancel(&self, id: &str) -> Result<()> {
        if self.status(id).is_none_or(|record| record.state.is_terminal()) { return Err(CoreError::new("build_not_running", "A fordítás már nem fut.")); }
        if let Some((active_id, flag)) = &self.cancel {
            if active_id == id { flag.store(true, Ordering::Release); return Ok(()); }
        }
        Err(CoreError::new("build_not_running", "Nincs ilyen futó fordítás."))
    }
}

impl Drop for BuildSupervisor {
    fn drop(&mut self) {
        if let Some((_, flag)) = &self.cancel { flag.store(true, Ordering::Release); }
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

struct OutputLog { preview: VecDeque<u8>, file: File, written: usize, truncated: bool }
fn drain(input: impl Read + Send + 'static, log: Arc<Mutex<OutputLog>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut input = input;
        let mut buffer = [0; 8192];
        loop {
            let length = match input.read(&mut buffer) { Ok(0) => break, Ok(length) => length, Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue, Err(_) => break };
            let mut log = match log.lock() { Ok(log) => log, Err(_) => break };
            let keep = length.min(LOG_LIMIT.saturating_sub(log.written));
            if log.file.write_all(&buffer[..keep]).is_err() { log.truncated = true; }
            log.written += keep;
            if keep < length { log.truncated = true; }
            log.preview.extend(&buffer[..length]);
            let excess = log.preview.len().saturating_sub(PREVIEW_LIMIT);
            log.preview.drain(..excess);
        }
    })
}

struct Outcome { state: BuildState, exit_code: Option<i32>, log: String, log_truncated: bool }
fn execute(request: &BuildRequest, root: &std::path::Path, source: &std::path::Path, output: &std::path::Path, cancel: &AtomicBool) -> Result<Outcome> {
    if cancel.load(Ordering::Acquire) { return Ok(Outcome { state: BuildState::Cancelled, exit_code: None, log: String::new(), log_truncated: false }); }
    let output_file = File::create(output.join("compiler-output.log"))?;
    let mut command = Command::new(&request.compiler_path);
    command.current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    process::environment(&mut command, &request.compiler_path, &request.cache_directory)?;
    match request.backend {
        Backend::Tectonic => { command.args(["-X", "compile", "--untrusted", "--keep-logs", "--outdir"]).arg(output).arg(source)
            .env("TECTONIC_CACHE_DIR", &request.cache_directory).env("TECTONIC_UNTRUSTED_MODE", "1"); }
        Backend::LatexMk { engine } => { command.args(["-norc", match engine { Engine::PdfLaTex => "-pdf", Engine::XeLaTex => "-xelatex", Engine::LuaLaTex => "-lualatex" }, "-interaction=nonstopmode", "-file-line-error", "-synctex=1", "-no-shell-escape"])
            .arg(format!("-outdir={}", output.display())).arg(source); }
    }
    process::prepare(&mut command);
    let mut tree = process::Tree::attach(command.spawn()?)?;
    let log = Arc::new(Mutex::new(OutputLog { preview: VecDeque::new(), file: output_file, written: 0, truncated: false }));
    let stdout = drain(tree.child.stdout.take().expect("piped stdout"), Arc::clone(&log));
    let stderr = drain(tree.child.stderr.take().expect("piped stderr"), Arc::clone(&log));
    let started = Instant::now();
    let (exit_code, mut state) = loop {
        if cancel.load(Ordering::Acquire) {
            tree.terminate();
            break (tree.child.wait()?.code(), BuildState::Cancelled);
        }
        if started.elapsed() >= request.timeout {
            tree.terminate();
            break (tree.child.wait()?.code(), BuildState::TimedOut);
        }
        if let Some(status) = tree.child.try_wait()? {
            break (status.code(), if status.success() { BuildState::Succeeded } else { BuildState::Failed });
        }
        thread::sleep(Duration::from_millis(15));
    };
    // Kill any descendants remaining after the main compiler exited before joining inherited pipes.
    tree.terminate();
    let _ = stdout.join();
    let _ = stderr.join();
    let log = log.lock().map_err(|_| CoreError::new("log_unavailable", "A fordítási napló nem érhető el."))?;
    let bytes: Vec<u8> = log.preview.iter().copied().collect();
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    let pdf = pdf_output(&output, &source);
    if state == BuildState::Succeeded && (!pdf.is_file() || fs::metadata(&pdf)?.len() < 5) {
        state = BuildState::Failed;
        text.push_str("\nA fordító nem hozott létre PDF-et.\n");
    }
    Ok(Outcome { state, exit_code, log: text, log_truncated: log.truncated })
}

fn pdf_output(output: &Path, source: &Path) -> PathBuf {
    // Replace the original filename's extension exactly once (thesis.final.tex).
    output.join(source.file_name().unwrap_or_default()).with_extension("pdf")
}

fn file_line_message(text: &str) -> Option<(&str, u32, &str)> {
    // Search rather than split once: Windows drive letters also contain a colon.
    text.match_indices(':').find_map(|(index, _)| {
        let (number, message) = text[index + 1..].split_once(':')?;
        let number = number.trim().parse::<u32>().ok()?;
        Some((&text[..index], number, message.trim()))
    })
}
fn parse_diagnostics(log: &str) -> Vec<Diagnostic> {
    log.lines().filter_map(|line| {
        let explicit = line.strip_prefix("error: ").map(|body| ("error", body))
            .or_else(|| line.strip_prefix("warning: ").map(|body| ("warning", body)))
            .or_else(|| line.strip_prefix("! ").map(|body| ("error", body)));
        let body = explicit.map_or(line, |(_, body)| body);
        let located = file_line_message(body);
        let lower = body.to_ascii_lowercase();
        let severity = explicit.map(|(severity, _)| severity).or_else(|| {
            if lower.contains("warning:") { Some("warning") }
            else if located.is_some() { Some("error") } else { None }
        })?;
        let (file, number, message) = match located {
            Some((file, number, message)) => (Some(file.into()), Some(number), message),
            None => (None, None, body.trim()),
        };
        Some(Diagnostic { severity: severity.into(), file, line: number, message: message.into() })
    }).take(200).collect()
}
