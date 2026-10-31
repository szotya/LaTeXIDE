//! Versioned commands shared by the native frontends and local agent bridge.

pub use latexide_contracts as contracts;

pub mod ipc;
use latexide_core::{CoreError, TextEdit};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const READ_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    WorkspaceInfo,
    ListProjectFiles { #[serde(default)] offset: usize, #[serde(default = "page_limit")] limit: usize },
    OpenDocument { path: String },
    ReadDocument { document_id: String, #[serde(default)] revision: Option<u64>,
        #[serde(default)] offset_bytes: usize, #[serde(default = "read_limit")] limit_bytes: usize },
    GetSyntax { document_id: String, expected_revision: u64, offset_utf16: u32, length_utf16: u32 },
    GetCompletions { document_id: String, expected_revision: u64, caret_utf16: u32,
        #[serde(default)] refresh_project: bool },
    GetDefinition { document_id: String, expected_revision: u64, offset_utf16: u32 },
    ApplyEdits { document_id: String, expected_revision: u64, edits: Vec<TextEdit> },
    BeginComposition { document_id: String, expected_revision: u64 },
    EndComposition { document_id: String, expected_revision: u64, edits: Vec<TextEdit> },
    SaveDocument { document_id: String, expected_revision: u64 },
    Undo,
    Redo,
    GetDiagnostics,
    ConfigureCompiler { compiler_path: String, cache_directory: String, output_directory: String,
        backend: String, #[serde(default = "engine_default")] engine: String,
        #[serde(default = "timeout_default")] timeout_seconds: u64 },
    RequestBuild { main_file: String },
    CancelBuild { id: String },
    GetOperation { id: String },
}
fn page_limit() -> usize { 256 }
fn read_limit() -> usize { READ_CHUNK_BYTES }
fn engine_default() -> String { "xelatex".into() }
fn timeout_default() -> u64 { 120 }

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CoreError>,
}
impl Reply {
    pub fn success(value: Value) -> Self { Self { ok: true, result: Some(value), error: None } }
    pub fn failure(error: CoreError) -> Self { Self { ok: false, result: None, error: Some(error) } }
    pub fn encode(&self) -> Vec<u8> {
        let encoded = serde_json::to_vec(self).expect("JSON values serialize");
        if encoded.len() > MAX_MESSAGE_BYTES {
            serde_json::to_vec(&Self::failure(CoreError::new("response_too_large", "Az eredményt kisebb részletekben kell lekérni."))).expect("bounded error")
        } else { encoded }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDescriptor {
    pub version: u32,
    pub session_id: String,
    pub endpoint: String,
    pub token: String,
    pub project_root: String,
}

#[derive(Deserialize, Serialize)]
pub struct Envelope { pub version: u32, pub token: String, pub request: Request }
