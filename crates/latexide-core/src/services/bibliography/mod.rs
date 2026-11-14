//! Read-only local first slice. No Runtime hook, file write, macro expansion or import.
//!
//! `source` is supplied text: a U+FEFF is content, not a physical encoding BOM.
//! CRLF and all trivia remain literal. Integration must supply authoritative
//! document identity/revision; the standalone diagnostic anchor uses
//! `bibliography.bib`/revisionNone and a real digest of exactly this input.
mod fingerprint;
mod parser;

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
