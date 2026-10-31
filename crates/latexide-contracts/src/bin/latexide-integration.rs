//! Read-only queue inspection; no manifest command or patch is executed.
use latexide_contracts::{ContractError, coordinator::inspect_queue};
use std::{io::Write, path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let root = match args.as_slice() {
        [] => std::env::current_dir().map_err(|_| ContractError::ForeignPath),
        [flag, root] if flag == "--root" => Ok(PathBuf::from(root)),
        _ => Err(ContractError::InvalidManifest),
    };
    let result = root.and_then(|root| inspect_queue(&root));
    let (json, success) = match result {
        Ok(queue) => (serde_json::to_value(queue).expect("queue serializes"), true),
        Err(error) => (serde_json::json!({"schema": 1, "status": "failed", "error": error}), false),
    };
    let mut output = std::io::stdout().lock();
    let written = serde_json::to_writer(&mut output, &json).is_ok() && writeln!(output).is_ok();
    if success && written { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
