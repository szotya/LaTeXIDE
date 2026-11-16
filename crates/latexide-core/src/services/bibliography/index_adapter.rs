//! Project-index projection of canonical entries, not another BibTeX parser.
//! The caller supplies authoritative logical text and owns path/revision/cache.
//! Provider diagnostics retain their standalone anchor and are not projected.
use crate::{services::bibliography::{parse_bibliography, FragmentKind}, tex_scan};
use std::ops::Range;

pub(super) struct Citation {
    pub key: String,
    pub entry_type: String,
    pub key_bytes: Range<usize>,
}

pub(super) fn citations(source: &str, limit: usize) -> (Vec<Citation>, bool) {
    // The host's unchanged per-file and aggregate budgets bound the full parse.
    if source.len() > crate::MAX_FILE_BYTES { return (Vec::new(), true); }
    let parsed = parse_bibliography(source);
    let mut fragments = parsed.fragments().iter().filter(|part| part.kind == FragmentKind::Entry);
    let mut citations = Vec::with_capacity(limit.min(parsed.report().entries.len()));
    let mut partial = false;
    for entry in &parsed.report().entries {
        let Some(fragment) = fragments.next() else { return (citations, true); };
        // Preserve the existing completion/definition key syntax and byte cap.
        if !tex_scan::valid_key(&entry.key) { continue; }
        if citations.len() == limit { return (citations, true); }
        let Some(key_bytes) = key_bytes(source, fragment.bytes.clone(), &entry.entry_type, &entry.key) else {
            partial = true;
            continue;
        };
        citations.push(Citation { key: entry.key.clone(), entry_type: entry.entry_type.to_ascii_lowercase(), key_bytes });
    }
    (citations, partial)
}

fn key_bytes(source: &str, entry: Range<usize>, kind: &str, key: &str) -> Option<Range<usize>> {
    // Only locate the header span of an entry already accepted by the provider.
    // A search for the key/opener could otherwise land in a preceding % comment.
    let kind_start = entry.start.checked_add(1)?;
    let kind_end = kind_start.checked_add(kind.len())?;
    if source.get(entry.start..kind_start)? != "@" || source.get(kind_start..kind_end)? != kind { return None; }
    let open = trivia_end(source, kind_end, entry.end);
    if !matches!(source.as_bytes().get(open), Some(b'{' | b'(')) { return None; }
    let start = trivia_end(source, open + 1, entry.end);
    let end = start.checked_add(key.len())?;
    (end <= entry.end && source.get(start..end) == Some(key)).then_some(start..end)
}

fn trivia_end(source: &str, mut byte: usize, end: usize) -> usize {
    loop {
        while byte < end && source.as_bytes()[byte].is_ascii_whitespace() { byte += 1; }
        if byte < end && source.as_bytes()[byte] == b'%' {
            while byte < end && source.as_bytes()[byte] != b'\n' { byte += 1; }
        } else { return byte; }
    }
}
