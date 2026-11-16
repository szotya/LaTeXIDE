//! Real Workspace consumers of the canonical bibliography index; no mock provider.
use latexide_core::{completion::{CompletionKind, CompletionSet}, definition::DefinitionSet, Position, TextEdit, Workspace};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("latexide-bibliography-index-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn workspace(&self, key: &str) -> (Workspace, u32) {
        let source = format!("\\cite{{{key}}}\n\\cite{{}}");
        let caret = source[..source.rfind('}').unwrap()].encode_utf16().count() as u32;
        self.write("main.tex", &source);
        let mut workspace = Workspace::open(self.0.clone()).unwrap();
        workspace.open_document("main.tex").unwrap();
        (workspace, caret)
    }
}
// Keep each owned fixture on disk for bounded-run readback, including failures.
fn labels(set: &CompletionSet) -> Vec<&str> { set.items.iter().map(|item| item.label.as_str()).collect() }
fn complete(ws: &mut Workspace, caret: u32) -> CompletionSet { ws.completions("main.tex", 1, caret).unwrap() }
fn definition(ws: &mut Workspace) -> DefinitionSet { ws.definition("main.tex", 1, 6).unwrap() }
fn replace(ws: &mut Workspace, id: &str, text: &str) -> u64 {
    let old = ws.snapshot(id).unwrap();
    let mut end = Position { line: 0, character: 0 };
    for ch in old.text.chars() {
        if ch == '\n' { end.line += 1; end.character = 0; }
        else { end.character += ch.len_utf16() as u32; }
    }
    ws.apply_edits(id, old.revision, vec![TextEdit { start: Position { line: 0, character: 0 }, end, text: text.into() }], "agent").unwrap().revision
}

#[test]
fn bib_index_01_dirty_bib_shadows_disk_without_save() {
    let p = Project::new();
    let disk = "@book{disk,title={Disk}}\n";
    p.write("refs.bib", disk);
    let (mut ws, caret) = p.workspace("dirty");
    ws.open_document("refs.bib").unwrap();
    let revision = replace(&mut ws, "refs.bib", "@book{dirty,title={Unsent}}\n");
    assert_eq!(revision, 2);
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["dirty"]);
    let set = definition(&mut ws); let target = set.target.unwrap();
    assert_eq!(target.document_id, "refs.bib");
    assert_eq!((target.start_utf16, target.length_utf16, target.revision), (6, 5, Some(2)));
    assert_eq!(set.alternatives, 1);
    assert!(ws.snapshot("refs.bib").unwrap().dirty);
    assert_eq!(fs::read_to_string(p.0.join("refs.bib")).unwrap(), disk);
}

#[test]
fn bib_index_02_new_revision_invalidates_same_size_cached_keys() {
    let p = Project::new(); let disk = "@book{oldKey,title={T}}\n";
    p.write("refs.bib", disk);
    let (mut ws, caret) = p.workspace("newKey");
    ws.open_document("refs.bib").unwrap();
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["oldKey"]);
    assert_eq!(replace(&mut ws, "refs.bib", "@book{newKey,title={T}}\n"), 2);
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["newKey"]);
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.start_utf16, target.length_utf16, target.revision), (6, 6, Some(2)));
    assert_eq!(ws.completions("main.tex", 0, caret).unwrap_err().code, "revision_conflict");
    assert_eq!(fs::read_to_string(p.0.join("refs.bib")).unwrap(), disk);
}

#[test]
fn bib_index_03_astral_key_and_bom_crlf_source_range_are_exact() {
    let p = Project::new();
    p.write("refs.bib", "\u{feff}% 😀\r\n@book{réf𝒜,title={T}}\r\n");
    let (mut ws, caret) = p.workspace("réf𝒜");
    let set = complete(&mut ws, caret);
    assert_eq!(labels(&set), vec!["réf𝒜"]);
    assert_eq!(set.items[0].kind, CompletionKind::Citation);
    let target = definition(&mut ws).target.unwrap();
    assert_eq!(target.document_id, "refs.bib");
    assert_eq!((target.start_utf16, target.length_utf16, target.line, target.revision), (11, 5, 2, None));
    assert_eq!(ws.snapshot("refs.bib").unwrap_err().code, "document_not_open");
    let opened = ws.open_document("refs.bib").unwrap();
    assert_eq!(opened.text, "% 😀\n@book{réf𝒜,title={T}}\n");
    let after = definition(&mut ws).target.unwrap();
    assert_eq!((after.start_utf16, after.length_utf16, after.revision), (11, 5, Some(1)));
}

#[test]
fn bib_index_04_last_per_file_winner_and_cross_file_alternatives_are_explicit() {
    let p = Project::new();
    p.write("a.bib", "@book{dup,title={First}}\n@article{dup,title={Last}}\n");
    p.write("z.bib", "@book{dup,title={Other}}\n");
    let (mut ws, caret) = p.workspace("dup");
    let completions = complete(&mut ws, caret);
    assert_eq!(labels(&completions), vec!["dup"]);
    assert!(completions.items[0].detail.contains("BibTeX · article"));
    assert!(completions.items[0].detail.contains("a.bib:2"));
    let set = definition(&mut ws); let target = set.target.unwrap();
    assert_eq!((target.document_id.as_str(), target.line, set.alternatives), ("a.bib", 2, 2));
    assert_eq!(target.length_utf16, 3);
    // Existing open-document precedence stays authoritative across files.
    ws.open_document("z.bib").unwrap();
    let set = definition(&mut ws);
    assert_eq!(set.target.unwrap().document_id, "z.bib");
    assert_eq!(set.alternatives, 2);
}

#[test]
fn bib_index_05_malformed_header_recovers_the_next_entry() {
    let p = Project::new(); p.write("refs.bib", "@bad\n@book{ok,title={Recovered}}\n");
    let (mut ws, caret) = p.workspace("ok");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["ok"]);
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.start_utf16, target.length_utf16, target.line), (11, 2, 2));
}

#[test]
fn bib_index_06_comment_literal_opener_uses_canonical_entry_boundaries() {
    let p = Project::new();
    p.write("refs.bib", "% @book{fake,title={Ignored}}\n@comment{\"% literal @book{ghost,title={Ignored}}}\n@book{real,title={Kept}}\n");
    let (mut ws, caret) = p.workspace("real");
    let set = complete(&mut ws, caret);
    assert!(labels(&set).contains(&"real"), "CANONICAL_COMMENT_ENTRY_VISIBLE");
    assert_eq!(labels(&set), vec!["real"], "NO_COMMENT_LITERAL_KEYS");
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.document_id.as_str(), target.length_utf16, target.line), ("refs.bib", 4, 3));
}

#[test]
fn bib_index_07_missing_bib_is_a_bounded_empty_result() {
    let p = Project::new(); let (mut ws, caret) = p.workspace("missing");
    assert!(complete(&mut ws, caret).items.is_empty());
    let set = definition(&mut ws);
    assert!(set.target.is_none()); assert_eq!(set.alternatives, 0);
    let status = set.index_status.unwrap();
    assert_eq!(status.indexed_files, 1); assert_eq!(status.skipped_files, 0); assert!(!status.truncated);
    assert_eq!(ws.open_document("absent.bib").unwrap_err().code, "io_error");
}

#[test]
fn bib_index_08_file_byte_and_symbol_quotas_keep_partial_status() {
    let p = Project::new();
    p.write("0000-oversized.bib", &"x".repeat(latexide_core::MAX_FILE_BYTES + 1));
    for n in 0..130 { p.write(&format!("refs/{n:03}.bib"), &format!("@book{{k{n:03},title={{T}}}}\n")); }
    let (mut ws, caret) = p.workspace("k129");
    let set = complete(&mut ws, caret); let status = set.index_status.unwrap();
    assert_eq!(set.items.len(), 20); assert_eq!(status.indexed_files, 128);
    assert!(status.truncated); assert_eq!(status.skipped_files, 1);
    let set = definition(&mut ws); assert!(set.target.is_none()); assert!(set.index_status.unwrap().truncated);
    let p = Project::new();
    let bib = (0..8193).map(|n| format!("@book{{k{n:04},title={{T}}}}\n")).collect::<String>();
    p.write("many.bib", &bib); let (mut ws, caret) = p.workspace("k8192");
    let set = complete(&mut ws, caret); assert_eq!(set.items.len(), 20);
    assert!(set.index_status.as_ref().unwrap().truncated);
    let set = definition(&mut ws); assert!(set.target.is_none());
    assert!(set.index_status.unwrap().truncated);
}

#[test]
fn bib_index_09_header_comment_cannot_forge_the_key_range() {
    let p = Project::new(); p.write("refs.bib", "@book % {real,ignored}\n{real,title={Value}}\n");
    let (mut ws, caret) = p.workspace("real");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["real"]);
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.start_utf16, target.length_utf16, target.line), (24, 4, 2));
}

#[test]
fn bib_index_10_rejected_keys_do_not_shift_later_entry_ranges() {
    let p = Project::new();
    let prefix = format!("% 😀\n@book{{,title={{Empty}}}}\n@book{{bad key,title={{Invalid}}}}\n@book{{{},title={{Long}}}}\n", "x".repeat(257));
    let source = format!("{prefix}@book{{kept,title={{Value}}}}\n");
    p.write("refs.bib", &source);
    let (mut ws, caret) = p.workspace("kept");
    assert_eq!(labels(&complete(&mut ws, caret)), vec!["kept"]);
    let set = definition(&mut ws);
    let target = set.target.unwrap();
    assert_eq!((target.document_id.as_str(), target.start_utf16, target.length_utf16, target.line),
        ("refs.bib", prefix.encode_utf16().count() as u32 + 6, 4, 5));
    assert!(!set.index_status.unwrap().truncated);
}

#[test]
fn bib_index_11_parenthesized_mixed_case_header_trivia_keeps_exact_key() {
    let p = Project::new();
    let prefix = "@BoOk % (parent, fake)\n( % parent ignored\n ";
    let source = format!("{prefix}parent , title = {{parent}})\n@BOOK{{Case,title={{Upper}}}}\n@book{{case,title={{Lower}}}}\n");
    p.write("refs.bib", &source);
    let (mut ws, caret) = p.workspace("parent");
    let completions = complete(&mut ws, caret);
    let names = labels(&completions);
    assert_eq!(names.len(), 3);
    for key in ["parent", "Case", "case"] { assert!(names.contains(&key)); }
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.start_utf16, target.length_utf16, target.line),
        (prefix.encode_utf16().count() as u32, 6, 3));
    assert!(completions.items.iter().find(|item| item.label == "parent").unwrap().detail.contains("BibTeX · book"));
}

#[test]
fn bib_index_12_key_byte_limit_is_preserved_for_unicode() {
    let p = Project::new();
    let accepted = "𝒜".repeat(64); // 256 UTF-8 bytes, 128 UTF-16 units.
    let rejected = format!("{accepted}x");
    p.write("refs.bib", &format!("@book{{{rejected},title={{Rejected}}}}\n@book{{{accepted},title={{Accepted}}}}\n"));
    let (mut ws, caret) = p.workspace(&accepted);
    assert_eq!(labels(&complete(&mut ws, caret)), vec![accepted.as_str()]);
    let target = definition(&mut ws).target.unwrap();
    assert_eq!((target.length_utf16, target.line), (128, 2));
    let text = ws.open_document("refs.bib").unwrap().text;
    assert_eq!(text.encode_utf16().skip(target.start_utf16 as usize).take(target.length_utf16 as usize).collect::<Vec<_>>(), accepted.encode_utf16().collect::<Vec<_>>());
}

#[test]
fn bib_index_13_exact_symbol_budget_remains_complete() {
    let p = Project::new();
    let source = (0..8192).map(|n| format!("@book{{k{n:04},title={{T}}}}\n")).collect::<String>();
    p.write("many.bib", &source);
    let (mut ws, caret) = p.workspace("k8191");
    let completions = complete(&mut ws, caret);
    assert_eq!(completions.items.len(), 20);
    assert!(!completions.index_status.unwrap().truncated);
    let set = definition(&mut ws);
    assert_eq!(set.target.unwrap().line, 8192);
    assert!(!set.index_status.unwrap().truncated);
}
