//! Frozen data boundaries for independently developed IDE services.

pub mod integration;
pub mod version;
pub mod common;
pub mod envelope;
pub mod coordinator;
pub use integration::*;
pub use version::*;
pub use common::*;
pub use envelope::*;

// BEGIN GENERATED PORTS
pub mod integration_port;
pub mod recovery;
pub mod transactions;
pub mod runtime;
pub mod vfs;
pub mod language;
pub mod navigation;
pub mod editor;
pub mod workspace;
pub mod build;
pub mod pdf;
pub mod research;
pub mod bibliography;
pub mod git;
pub mod assistant;
pub mod extensions;
pub mod terminal;
pub mod developer_tools;
pub mod security;
pub mod quality_corpus;
pub mod release;
pub mod windows;
pub mod linux;
pub mod performance_accessibility;
pub mod verticals;
// END GENERATED PORTS
