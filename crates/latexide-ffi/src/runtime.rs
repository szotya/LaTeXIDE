use latexide_build::{Backend, BuildRequest, BuildSupervisor, Engine};
use latexide_core::{CoreError, DocumentSnapshot, Workspace};
use latexide_protocol::{READ_CHUNK_BYTES, Reply, Request};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

struct CompilerSettings {
    path: PathBuf,
    cache: PathBuf,
    output: PathBuf,
    backend: Backend,
    timeout: Duration,
}
pub struct Runtime {
    pub workspace: Workspace,
    pub session_descriptor: PathBuf,
    compiler: Option<CompilerSettings>,
    supervisor: BuildSupervisor,
    build_revisions: BTreeMap<String, Vec<(String, u64)>>,
    composition: Option<String>,
}

fn document_info(snapshot: &DocumentSnapshot) -> Value {
    json!({"id":snapshot.id,"revision":snapshot.revision,"dirty":snapshot.dirty,
        "last_origin":snapshot.last_origin,"transaction_id":snapshot.transaction_id})
}

impl Runtime {
    pub fn open_project(root: PathBuf) -> Result<Self, CoreError> {
        let root = root.canonicalize()?;
        #[cfg(target_os = "macos")]
        let workspace =
            Workspace::open_with_optional_recovery(root.clone(), root.join(".latexide/recovery"))?;
        #[cfg(not(target_os = "macos"))]
        let workspace = Workspace::open(root)?;
        Ok(Self::new(workspace))
    }
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            session_descriptor: PathBuf::new(),
            compiler: None,
            supervisor: BuildSupervisor::new(),
            build_revisions: BTreeMap::new(),
            composition: None,
        }
    }

    fn operation(&self, id: &str) -> Result<Value, CoreError> {
        let status = self.supervisor.status(id).ok_or_else(|| {
            CoreError::new("operation_not_found", "Nincs ilyen fordítási művelet.")
        })?;
        let current = self.workspace.document_infos();
        let sources = self.build_revisions.get(id);
        let stale = current.iter().any(|document| document.dirty)
            || sources.is_some_and(|sources| {
                sources.iter().any(|(id, revision)| {
                    !current
                        .iter()
                        .any(|document| &document.id == id && &document.revision == revision)
                })
            });
        let mut result = serde_json::to_value(status).expect("build status");
        result["stale"] = json!(stale);
        result["source_revisions"] = json!(sources);
        Ok(result)
    }

    pub fn dispatch(&mut self, request: Request, origin: &str) -> Reply {
        match self.execute(request, origin) {
            Ok(result) => Reply::success(result),
            Err(error) => Reply::failure(error),
        }
    }

    fn execute(&mut self, request: Request, origin: &str) -> Result<Value, CoreError> {
        if self.composition.is_some()
            && matches!(
                &request,
                Request::ApplyEdits { .. }
                    | Request::SaveDocument { .. }
                    | Request::Undo
                    | Request::Redo
                    | Request::RequestBuild { .. }
                    | Request::BeginComposition { .. }
            )
        {
            return Err(CoreError::new(
                "composition_active",
                "A natív szövegbevitel véglegesítéséig ez a művelet várakozik; próbáld újra.",
            ));
        }
        match request {
            Request::WorkspaceInfo => Ok(json!({"protocol_version":1,"root":self.workspace.root(),
                "open_documents":self.workspace.document_infos(),"can_undo":self.workspace.can_undo(),
                "can_redo":self.workspace.can_redo(),"session_descriptor":self.session_descriptor,
                "compiler_ready":self.compiler.is_some(),"recovery":self.workspace.recovery_status(),
                "build":self.supervisor.latest().map(|status| json!({"id":status.id,"state":status.state,"pdf_path":status.pdf_path}))})),
            Request::ListProjectFiles { offset, limit } => {
                let files = self.workspace.list_files()?;
                let total = files.len();
                let selected: Vec<_> = files
                    .into_iter()
                    .skip(offset)
                    .take(limit.clamp(1, 256))
                    .collect();
                Ok(json!({"files":selected,"total":total,"offset":offset}))
            }
            Request::OpenDocument { path } => {
                Ok(document_info(&self.workspace.open_document(&path)?))
            }
            Request::ReadDocument {
                document_id,
                revision,
                offset_bytes,
                limit_bytes,
            } => {
                let document = match self.workspace.snapshot(&document_id) {
                    Ok(document) => document,
                    Err(error) if error.code == "document_not_open" => {
                        self.workspace.open_document(&document_id)?
                    }
                    Err(error) => return Err(error),
                };
                if revision.is_some_and(|expected| expected != document.revision) {
                    return Err(CoreError::new(
                        "revision_conflict",
                        "A dokumentum közben megváltozott.",
                    ));
                }
                if offset_bytes > document.text.len()
                    || !document.text.is_char_boundary(offset_bytes)
                {
                    return Err(CoreError::new(
                        "invalid_range",
                        "Az olvasási határ nem érvényes UTF-8 pozíció.",
                    ));
                }
                let mut end = offset_bytes
                    .saturating_add(limit_bytes.clamp(4, READ_CHUNK_BYTES))
                    .min(document.text.len());
                while !document.text.is_char_boundary(end) {
                    end -= 1;
                }
                Ok(
                    json!({"id":document.id,"revision":document.revision,"offset_bytes":offset_bytes,
                    "next_offset":end,"done":end==document.text.len(),"total_bytes":document.text.len(),
                    "text":&document.text[offset_bytes..end],"dirty":document.dirty,"last_origin":document.last_origin,
                    "line_ending":document.line_ending,"bom":document.bom}),
                )
            }
            Request::GetSyntax {
                document_id,
                expected_revision,
                offset_utf16,
                length_utf16,
            } => {
                let spans = self.workspace.syntax_window(
                    &document_id,
                    expected_revision,
                    offset_utf16,
                    length_utf16,
                )?;
                Ok(
                    json!({"document_id":document_id,"revision":expected_revision,"offset_utf16":offset_utf16,
                    "length_utf16":length_utf16.min(32768),"spans":spans}),
                )
            }
            Request::GetCompletions {
                document_id,
                expected_revision,
                caret_utf16,
                refresh_project,
            } => {
                if refresh_project {
                    self.workspace.refresh_completion_index();
                }
                let completion =
                    self.workspace
                        .completions(&document_id, expected_revision, caret_utf16)?;
                let mut result = serde_json::to_value(completion).expect("completion");
                result["document_id"] = json!(document_id);
                result["revision"] = json!(expected_revision);
                Ok(result)
            }
            Request::GetDefinition {
                document_id,
                expected_revision,
                offset_utf16,
            } => {
                let definition =
                    self.workspace
                        .definition(&document_id, expected_revision, offset_utf16)?;
                let mut result = serde_json::to_value(definition).expect("definition");
                result["document_id"] = json!(document_id);
                result["revision"] = json!(expected_revision);
                Ok(result)
            }
            Request::ApplyEdits {
                document_id,
                expected_revision,
                edits,
            } => Ok(serde_json::to_value(self.workspace.apply_edits(
                &document_id,
                expected_revision,
                edits,
                origin,
            )?)
            .expect("document info")),
            Request::BeginComposition {
                document_id,
                expected_revision,
            } => {
                if origin != "user" {
                    return Err(CoreError::new(
                        "permission_denied",
                        "Kompozíciót a natív szerkesztő kezdhet.",
                    ));
                }
                let snapshot = self.workspace.snapshot(&document_id)?;
                if snapshot.revision != expected_revision {
                    return Err(CoreError::new(
                        "revision_conflict",
                        "A dokumentum közben megváltozott.",
                    ));
                }
                self.composition = Some(document_id);
                Ok(json!({"active":true}))
            }
            Request::EndComposition {
                document_id,
                expected_revision,
                edits,
            } => {
                if origin != "user" {
                    return Err(CoreError::new(
                        "permission_denied",
                        "Kompozíciót a natív szerkesztő zárhat le.",
                    ));
                }
                if self.composition.as_ref() != Some(&document_id) {
                    return Err(CoreError::new(
                        "composition_inactive",
                        "Nincs ilyen aktív kompozíció.",
                    ));
                }
                // End consumes the native composition even on validation failure.
                // The frontend restores the authoritative text on a rejected commit.
                self.composition = None;
                let result =
                    self.workspace
                        .apply_edits(&document_id, expected_revision, edits, origin)?;
                Ok(serde_json::to_value(result).expect("document info"))
            }
            Request::SaveDocument {
                document_id,
                expected_revision,
            } => Ok(document_info(
                &self
                    .workspace
                    .save_document(&document_id, expected_revision)?,
            )),
            Request::Undo => Ok(document_info(&self.workspace.undo()?)),
            Request::Redo => Ok(document_info(&self.workspace.redo()?)),
            Request::GetDiagnostics => match self.supervisor.latest() {
                Some(status) => Ok(json!({"build_id":status.id,"diagnostics":status.diagnostics})),
                None => Ok(json!({"build_id":null,"diagnostics":[]})),
            },
            Request::ConfigureCompiler {
                compiler_path,
                cache_directory,
                output_directory,
                backend,
                engine,
                timeout_seconds,
            } => {
                if origin != "user" {
                    return Err(CoreError::new(
                        "permission_denied",
                        "Az agent nem cserélheti le a fordítót.",
                    ));
                }
                if !(30..=600).contains(&timeout_seconds) {
                    return Err(CoreError::new(
                        "invalid_config",
                        "A fordítási időkorlát 30–600 másodperc lehet.",
                    ));
                }
                let engine = match engine.as_str() {
                    "pdflatex" => Engine::PdfLaTex,
                    "xelatex" => Engine::XeLaTex,
                    "lualatex" => Engine::LuaLaTex,
                    _ => return Err(CoreError::new("invalid_config", "Ismeretlen LaTeX-motor.")),
                };
                let backend = match backend.as_str() {
                    "tectonic" => Backend::Tectonic,
                    "texlive" | "latexmk" => Backend::LatexMk { engine },
                    _ => {
                        return Err(CoreError::new(
                            "invalid_config",
                            "Ismeretlen fordítóbackend.",
                        ));
                    }
                };
                let path = PathBuf::from(compiler_path);
                if !path.is_absolute() {
                    return Err(CoreError::new(
                        "invalid_config",
                        "A fordító útvonala abszolút legyen.",
                    ));
                }
                if !path.canonicalize()?.is_file() {
                    return Err(CoreError::new(
                        "compiler_missing",
                        "Nem található a fordítóprogram.",
                    ));
                }
                // Preserve the verified bin alias: latexmk is a symlink into texmf-dist,
                // but its sibling xelatex/kpsewhich programs live in bin/universal-darwin.
                let cache = PathBuf::from(cache_directory);
                let output = PathBuf::from(output_directory);
                if !cache.is_absolute() || !output.is_absolute() {
                    return Err(CoreError::new(
                        "invalid_config",
                        "A cache és a kimeneti mappa abszolút útvonal legyen.",
                    ));
                }
                self.compiler = Some(CompilerSettings {
                    path,
                    cache,
                    output,
                    backend,
                    timeout: Duration::from_secs(timeout_seconds),
                });
                Ok(json!({"ready":true}))
            }
            Request::RequestBuild { main_file } => {
                let sources = self.workspace.document_infos();
                if sources.iter().any(|document| document.dirty) {
                    return Err(CoreError::new(
                        "needs_save",
                        "Fordítás előtt mentsd a módosított dokumentumokat.",
                    ));
                }
                let compiler = self.compiler.as_ref().ok_or_else(|| {
                    CoreError::new("compiler_not_configured", "Nincs beállított fordító.")
                })?;
                let id = self.supervisor.start(BuildRequest {
                    root: self.workspace.root().to_owned(),
                    main_file,
                    compiler_path: compiler.path.clone(),
                    cache_directory: compiler.cache.clone(),
                    output_directory: compiler.output.clone(),
                    timeout: compiler.timeout,
                    backend: compiler.backend,
                })?;
                self.build_revisions.insert(
                    id.clone(),
                    sources
                        .into_iter()
                        .map(|document| (document.id, document.revision))
                        .collect(),
                );
                while self.build_revisions.len() > 8 {
                    self.build_revisions.pop_first();
                }
                Ok(json!({"id":id}))
            }
            Request::CancelBuild { id } => {
                self.supervisor.cancel(&id)?;
                Ok(json!({"id":id,"cancel_requested":true}))
            }
            Request::GetOperation { id } => self.operation(&id),
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "../../latexide-core/src/services/recovery/ffi_runtime_tests.rs"]
mod recovery_tests;
