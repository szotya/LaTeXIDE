//! Shared native IDE document and workspace contracts.

extern crate self as latexide_core;
pub use latexide_contracts as contracts;

mod document;
mod workspace;
mod tex_scan;
mod project_index;
pub mod syntax;
pub mod completion;
pub mod definition;

pub use document::{DocumentInfo, DocumentSnapshot, LineEnding, Position, TextEdit};
pub use workspace::{FileEntry, Workspace};
use serde::{Deserialize, Serialize};

pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_OPEN_DOCUMENTS: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreError {
    pub code: String,
    pub message: String,
}

impl CoreError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}
impl std::fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for CoreError {}
impl From<std::io::Error> for CoreError {
    fn from(error: std::io::Error) -> Self { Self::new("io_error", error.to_string()) }
}
pub type Result<T> = std::result::Result<T, CoreError>;
