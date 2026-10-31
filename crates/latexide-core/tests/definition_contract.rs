use latexide_core::{definition::DefinitionSet, Position, TextEdit, Workspace};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("latexide-definition-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&path).unwrap(); Self(path)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path); fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, text).unwrap();
    }
    fn open(&self, marked: &str) -> (Workspace, u32) {
        let (left, right) = marked.split_once('|').unwrap(); self.write("main.tex", &format!("{left}{right}"));
        let mut ws = Workspace::open(self.0.clone()).unwrap(); ws.open_document("main.tex").unwrap();
        (ws, left.encode_utf16().count() as u32)
    }
    fn query(&self, marked: &str) -> DefinitionSet { let (mut ws, at) = self.open(marked); ws.definition("main.tex", 1, at).unwrap() }
}
impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn macro_click_resolves_the_whole_name_and_multiline_unicode_declaration_range() {
    let p = Project::new(); p.write("macros.sty", "😀 pre\n  \\newcommand{\n  \\mean}[1]{#1}\n");
    for source in ["😀 |\\mean{x}", "😀 \\me|an{x}", "😀 \\mea|n{x}"] {
        let set = p.query(source); let target = set.target.unwrap();
        assert_eq!(set.symbol.as_deref(), Some("\\mean"));
        assert_eq!((set.source_start_utf16, set.source_length_utf16), (3, 5));
        assert_eq!(target.document_id, "macros.sty");
        assert_eq!((target.start_utf16, target.length_utf16, target.line), (24, 5, 3));
        assert_eq!(target.revision, None);
    }
}

#[test]
fn ref_and_cite_choose_the_clicked_full_key_with_options_and_list_neighbors() {
    let p = Project::new(); p.write("labels.tex", "😀\n\\label{ sec:méthod }\n\\label{other}");
    let set = p.query("\\cref{other, sec:m|éthod}"); let target = set.target.unwrap();
    assert_eq!(set.symbol.as_deref(), Some("sec:méthod"));
    assert_eq!((target.start_utf16, target.length_utf16, target.line), (11, 10, 2));
    p.write("refs.bib", "😀\n@book{\n árvíz2026, title={A}}\n@article{other, title={B}}");
    let set = p.query("\\citep*[see][p. 2]{other, ár|víz2026}"); let target = set.target.unwrap();
    assert_eq!(target.document_id, "refs.bib");
    assert_eq!((target.start_utf16, target.length_utf16, target.line), (11, 9, 3));
}

#[test]
fn exact_lookup_does_not_navigate_prefixes_delimiters_prose_or_ignored_text() {
    let p = Project::new(); p.write("defs.tex", "\\newcommand{\\mean}{}\\label{sec:one}"); p.write("refs.bib", "@book{knuth, title={A}}");
    for source in ["\\me|", "\\mean|{}", "\\ref{|}", "\\ref{sec:|}", "\\ref{sec:one|}", "\\cite{knuth|,}", "plain |mean", "% \\m|ean\n", "\\verb!\\m|ean!", "\\begin{verbatim}\\m|ean\\end{verbatim}", "\\begin{comment}\\ref{sec:|one}\\end{comment}", "\\cite[kn|uth]{other}", "\\section{ti|tle}"] {
        assert!(p.query(source).target.is_none(), "unexpected definition for {source}");
    }
}

#[test]
fn every_macro_character_before_a_comment_or_verbatim_boundary_keeps_its_definition() {
    let p = Project::new(); p.write("defs.tex", "\\newcommand{\\mean}{}");
    for tail in ["% comment", "\\verb!\\mean!", "\\begin{verbatim}\\mean\\end{verbatim}"] {
        let (mut ws, _) = p.open(&format!("|\\mean{tail}"));
        for at in 0..5 { assert_eq!(ws.definition("main.tex", 1, at).unwrap().symbol.as_deref(), Some("\\mean"), "offset={at}, tail={tail}"); }
        assert!(ws.definition("main.tex", 1, 5).unwrap().target.is_none());
        assert!(ws.definition("main.tex", 1, 6).unwrap().target.is_none());
    }
}

#[test]
fn duplicate_bibtex_keys_use_the_same_last_declaration_per_file_policy() {
    let p = Project::new(); p.write("refs.bib", "@book{same, title={First}}\n@book{same, title={Last}}");
    let set = p.query("\\cite{s|ame}"); let target = set.target.unwrap();
    assert_eq!(target.line, 2); assert_eq!(set.alternatives, 1);
}

#[test]
fn dirty_target_edit_and_undo_move_the_definition_without_disk_writes() {
    let p = Project::new(); p.write("labels.tex", "\\label{sec:one}");
    let (mut ws, at) = p.open("\\ref{sec:o|ne}"); ws.open_document("labels.tex").unwrap();
    ws.apply_edits("labels.tex", 1, vec![TextEdit { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 0 }, text: "😀\n".into() }], "agent").unwrap();
    let target = ws.definition("main.tex", 1, at).unwrap().target.unwrap();
    assert_eq!((target.start_utf16, target.line, target.revision), (10, 2, Some(2)));
    ws.undo().unwrap(); let target = ws.definition("main.tex", 1, at).unwrap().target.unwrap();
    assert_eq!((target.start_utf16, target.line, target.revision), (7, 1, Some(3)));
    assert_eq!(fs::read_to_string(p.0.join("labels.tex")).unwrap(), "\\label{sec:one}");
}

#[test]
fn navigation_query_forces_unchanged_size_and_mtime_disk_content_refresh() {
    let p = Project::new(); p.write("labels.tex", "\\label{sec:one}\n");
    let (mut ws, at) = p.open("\\ref{sec:o|ne}"); assert!(ws.definition("main.tex", 1, at).unwrap().target.is_some());
    let path = p.0.join("labels.tex"); let modified = fs::metadata(&path).unwrap().modified().unwrap();
    p.write("labels.tex", "\n\\label{sec:one}");
    fs::File::options().write(true).open(path).unwrap().set_times(fs::FileTimes::new().set_modified(modified)).unwrap();
    let target = ws.definition("main.tex", 1, at).unwrap().target.unwrap();
    assert_eq!((target.start_utf16, target.line), (8, 2));
}

#[test]
fn unopened_bom_crlf_target_offsets_match_the_document_opening_decoder() {
    let p = Project::new(); p.write("defs.tex", "\u{feff}😀\r\n\\label{sec:one}\r\n");
    let (mut ws, at) = p.open("\\ref{sec:o|ne}"); let before = ws.definition("main.tex", 1, at).unwrap().target.unwrap();
    assert_eq!((before.start_utf16, before.length_utf16, before.line), (10, 7, 2));
    let opened = ws.open_document("defs.tex").unwrap(); assert_eq!(opened.text, "😀\n\\label{sec:one}\n");
    let after = ws.definition("main.tex", 1, at).unwrap().target.unwrap();
    assert_eq!((before.start_utf16, before.length_utf16), (after.start_utf16, after.length_utf16));
}

#[test]
fn lexical_precedence_matches_completion_and_reports_alternative_files() {
    let p = Project::new(); p.write("a.tex", "\\newcommand{\\mean}{}"); p.write("z.tex", "\\newcommand{\\mean}{}");
    let (mut ws, at) = p.open("\\m|ean");
    assert_eq!(ws.definition("main.tex", 1, at).unwrap().target.unwrap().document_id, "a.tex");
    ws.open_document("z.tex").unwrap(); let set = ws.definition("main.tex", 1, at).unwrap();
    assert_eq!(set.alternatives, 2); assert_eq!(set.target.unwrap().document_id, "z.tex");
    ws.apply_edits("main.tex", 1, vec![TextEdit { start: Position { line: 0, character: 5 }, end: Position { line: 0, character: 5 }, text: "\n\\newcommand{\\mean}{}\n\\renewcommand{\\mean}{}".into() }], "user").unwrap();
    let target = ws.definition("main.tex", 2, at).unwrap().target.unwrap();
    assert_eq!(target.document_id, "main.tex"); assert_eq!(target.line, 3);
}

#[test]
fn queries_do_not_open_target_buffers_edit_save_or_consume_history() {
    let p = Project::new(); for i in 0..20 { p.write(&format!("label{i:02}.tex"), &format!("\\label{{key:{i}}}")); }
    let (mut ws, at) = p.open("\\ref{key:|19}"); let before = ws.snapshot("main.tex").unwrap();
    for _ in 0..3 { assert_eq!(ws.definition("main.tex", 1, at).unwrap().target.unwrap().document_id, "label19.tex"); }
    assert_eq!(ws.document_infos().len(), 1); assert!(!ws.can_undo()); assert!(!ws.can_redo());
    assert_eq!(ws.snapshot("main.tex").unwrap().text, before.text); assert_eq!(ws.snapshot("main.tex").unwrap().revision, 1);
}

#[test]
fn rejects_stale_revisions_and_invalid_utf16_positions() {
    let p = Project::new(); let (mut ws, _) = p.open("😀 |\\mean");
    assert_eq!(ws.definition("main.tex", 9, 3).unwrap_err().code, "revision_conflict");
    for at in [1, 999] { assert_eq!(ws.definition("main.tex", 1, at).unwrap_err().code, "invalid_position"); }
}

#[test]
fn missing_or_over_budget_targets_remain_explicitly_partial_without_navigation() {
    let p = Project::new(); p.write("big.tex", &format!("{}\\label{{key:big}}", "x".repeat(1_048_576)));
    let set = p.query("\\ref{key:|big}"); assert!(set.target.is_none()); assert!(set.index_status.unwrap().truncated);
}

#[cfg(unix)]
#[test]
fn definition_targets_cannot_escape_through_a_symlink() {
    let p = Project::new(); let outside = Project::new(); outside.write("outside.tex", "\\newcommand{\\mean}{}");
    std::os::unix::fs::symlink(outside.0.join("outside.tex"), p.0.join("alias.tex")).unwrap();
    assert!(p.query("\\m|ean").target.is_none());
}
