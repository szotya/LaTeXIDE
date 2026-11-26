//! Local inspection and explicit host-bound cite completion. The parser is read-only;
//! completion_adapter can edit an existing Workspace. No Runtime hook, implicit
//! Save, macro expansion or network import is introduced.
//!
//! `source` is supplied text: a U+FEFF is content, not a physical encoding BOM.
//! CRLF and all trivia remain literal. Integration must supply authoritative
//! document identity/revision; the standalone diagnostic anchor uses
//! `bibliography.bib`/revisionNone and a real digest of exactly this input.
mod fingerprint;
mod parser;
pub mod workspace_adapter;
pub mod completion_adapter;
pub mod key_rename;

use latexide_contracts::bibliography::{BibliographyPort, BibliographyReport};
pub use parser::{Fragment, FragmentKind, LosslessBibliography, parse_bibliography};

#[derive(Debug, Default, Clone, Copy)]
pub struct LocalBibliographyProvider;

impl BibliographyPort for LocalBibliographyProvider {
    fn inspect_bibliography(&self, source: &str) -> BibliographyReport {
        inspect_bibliography(source)
    }
}

pub fn inspect_bibliography(source: &str) -> BibliographyReport {
    parse_bibliography(source).report().clone()
}
