//! Stdio bridge. Connects to the GUI's protected descriptor, never a second workspace.
use latexide_protocol::{Request, SessionDescriptor, ipc};
use rmcp::{ServerHandler, ServiceExt, RoleServer, ErrorData as McpError,
    service::RequestContext, model::*};
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};

#[derive(Clone)]
struct Bridge { descriptor: Arc<SessionDescriptor> }

fn definitions() -> Vec<Tool> {
    let string = json!({"type":"string"});
    let integer = json!({"type":"integer","minimum":0});
    let position = json!({"type":"object","required":["line","character"],
        "properties":{"line":integer,"character":integer},"additionalProperties":false});
    let edit = json!({"type":"object","required":["start","end","text"],
        "properties":{"start":position,"end":position,"text":string},"additionalProperties":false});
    let items = vec![
        ("workspace_info", "Current workspace, open document revisions and build state.", json!({}), vec![]),
        ("list_project_files", "Page through confined project-relative files.", json!({"offset":integer,"limit":integer}), vec![]),
        ("read_document", "Read current in-memory text in UTF-8 chunks, including unsaved edits. Use next_offset and revision for subsequent chunks.", json!({"document_id":string,"revision":integer,"offset_bytes":integer,"limit_bytes":integer}), vec!["document_id"]),
        ("apply_edits", "Apply edits directly and atomically with shared IDE undo. Positions are zero-based line and UTF-16 column; expected_revision is required. Saving is separate.", json!({"document_id":string,"expected_revision":integer,"edits":{"type":"array","items":edit}}), vec!["document_id","expected_revision","edits"]),
        ("save_document", "Save the specified revision, retaining undo history. External disk conflicts are errors.", json!({"document_id":string,"expected_revision":integer}), vec!["document_id","expected_revision"]),
        ("get_diagnostics", "Diagnostics from the latest compiler operation.", json!({}), vec![]),
        ("request_build", "Compile saved sources with the user-configured compiler. Returns an asynchronous operation ID.", json!({"main_file":string}), vec!["main_file"]),
        ("get_operation", "Poll build state, bounded log, PDF path and source staleness.", json!({"id":string}), vec!["id"]),
        ("cancel_build", "Cancel an operation and its compiler process tree.", json!({"id":string}), vec!["id"]),
    ];
    items.into_iter().map(|(name, description, properties, required)| {
        let schema = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
        Tool::new(name, description, schema.as_object().unwrap().clone())
    }).collect()
}

impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("latexide-local", env!("CARGO_PKG_VERSION")))
            .with_instructions("Edit through apply_edits to share the IDE's revision checks and undo history. Read dirty documents here before editing. Save explicitly before requesting a build.")
    }
    async fn list_tools(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(definitions()))
    }
    fn get_tool(&self, name: &str) -> Option<Tool> { definitions().into_iter().find(|tool| tool.name == name) }
    async fn call_tool(&self, params: CallToolRequestParams, _: RequestContext<RoleServer>) -> Result<CallToolResponse, McpError> {
        if self.get_tool(&params.name).is_none() { return Err(McpError::invalid_params("Unknown tool", None)); }
        let mut arguments = params.arguments.unwrap_or_default();
        arguments.insert("op".into(), Value::String(params.name.into_owned()));
        let request: Request = serde_json::from_value(Value::Object(arguments))
            .map_err(|error| McpError::invalid_params(error.to_string(), None))?;
        let descriptor = self.descriptor.clone();
        let response = tokio::task::spawn_blocking(move || ipc::call(&descriptor, request)).await
            .map_err(|_| McpError::internal_error("IPC worker failed", None))?;
        let (ok, text) = match response {
            Ok(reply) => (reply.ok, String::from_utf8(reply.encode()).expect("UTF-8 JSON")),
            Err(error) => (false, json!({"ok":false,"error":{"code":"ipc_unavailable","message":error.to_string()}}).to_string()),
        };
        Ok(if ok { CallToolResult::success(vec![ContentBlock::text(text)]) }
            else { CallToolResult::error(vec![ContentBlock::text(text)]) }.into())
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await { eprintln!("latexide-mcp: {error}"); std::process::exit(1); }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 2 || arguments[0] != "--session" { return Err("usage: latexide-mcp --session <protected-descriptor-path>".into()); }
    let descriptor = Arc::new(ipc::read_descriptor(Path::new(&arguments[1]))?);
    let bridge = Bridge { descriptor };
    let service = bridge.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
