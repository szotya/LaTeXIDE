//! Exact lexical definition lookup. Queries never open buffers or change history.
use crate::{completion::{self, CompletionIndexStatus, CompletionKind, Mode}, document::Document, project_index::ProjectIndex, Result};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct DefinitionTarget {
    pub document_id: String,
    pub revision: Option<u64>,
    pub start_utf16: u32,
    pub length_utf16: u32,
    pub line: u32,
}
#[derive(Debug, Default, Serialize)]
pub struct DefinitionSet {
    pub symbol: Option<String>,
    pub kind: Option<CompletionKind>,
    pub source_start_utf16: u32,
    pub source_length_utf16: u32,
    pub target: Option<DefinitionTarget>,
    pub alternatives: u32,
    pub index_status: Option<CompletionIndexStatus>,
}

pub(crate) fn context(text: &str, offset: u32) -> Result<DefinitionSet> {
    let byte = completion::byte_at(text, offset)?;
    let Some(character) = text[byte..].chars().next() else { return Ok(DefinitionSet::default()); };
    // A click owns this character, not the following comment/verbatim boundary.
    // Only the leading backslash needs to enter the command's caret context.
    let lookup = if character == '\\' { offset + 1 } else { offset };
    let context = completion::context(text, lookup)?;
    let set = context.set;
    let end = set.replace_start_utf16 + set.replace_length_utf16;
    if offset < set.replace_start_utf16 || offset >= end { return Ok(DefinitionSet::default()); }
    let kind = match context.mode { Mode::Command { .. } => CompletionKind::Macro, Mode::Key { kind, .. } => kind, Mode::None => return Ok(DefinitionSet::default()) };
    let start_byte = completion::byte_at(text, set.replace_start_utf16)?;
    let end_byte = completion::byte_at(text, end)?;
    Ok(DefinitionSet { symbol: Some(text[start_byte..end_byte].into()), kind: Some(kind),
        source_start_utf16: set.replace_start_utf16, source_length_utf16: set.replace_length_utf16, ..Default::default() })
}

pub(crate) fn with_index(mut set: DefinitionSet, index: &ProjectIndex, documents: &BTreeMap<String, Document>) -> DefinitionSet {
    set.index_status = Some(index.status.clone());
    let Some(symbol) = &set.symbol else { return set; };
    let name = if set.kind == Some(CompletionKind::Macro) { symbol.strip_prefix('\\').unwrap_or(symbol) } else { symbol };
    for item in index.symbols().filter(|item| Some(item.kind) == set.kind && item.name == name) {
        set.alternatives += 1;
        if set.target.is_none() {
            set.target = Some(DefinitionTarget { document_id: item.path.to_string(), revision: documents.get(item.path.as_ref()).map(|d| d.revision),
                start_utf16: item.start_utf16, length_utf16: item.length_utf16, line: item.line });
        }
    }
    set
}
