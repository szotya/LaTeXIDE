use latexide_core::{completion::CompletionSet, Position, TextEdit, Workspace};
use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(1);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("latexide-project-completion-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&root).unwrap(); Self(root)
    }
    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path); fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, text).unwrap();
    }
    fn open(&self, marked: &str) -> (Workspace, String, u32) {
        let (before, after) = marked.split_once("<caret>").unwrap();
        let source = format!("{before}{after}"); self.write("main.tex", &source);
        let mut workspace = Workspace::open(self.0.clone()).unwrap(); workspace.open_document("main.tex").unwrap();
        (workspace, source, before.encode_utf16().count() as u32)
    }
    fn complete(&self, marked: &str) -> CompletionSet {
        let (mut workspace, _, caret) = self.open(marked); workspace.completions("main.tex", 1, caret).unwrap()
    }
}
impl Drop for Project { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn labels(set: &CompletionSet) -> Vec<&str> { set.items.iter().map(|item| item.label.as_str()).collect() }
fn inserted(source: &str, set: &CompletionSet, label: &str) -> String {
    let item = set.items.iter().find(|item| item.label == label).unwrap();
    let mut text: Vec<u16> = source.encode_utf16().collect();
    let start = set.replace_start_utf16 as usize; let end = start + set.replace_length_utf16 as usize;
    text.splice(start..end, item.insert_text.encode_utf16()); String::from_utf16(&text).unwrap()
}

#[test]
fn own_macros_from_unopened_project_files_offer_arguments_and_origin() {
    let project = Project::new(); project.write("preamble/macros.sty", "Readable Unicode prose: Árvíz 😀\nA second line.\n\\newcommand{\\mean}[2]{#1+#2}\n");
    let set = project.complete("\\mea<caret>");
    let item = set.items.iter().find(|item| item.label == "\\mean").expect("project macro missing");
    assert_eq!(item.insert_text, "\\mean{}{}"); assert_eq!(item.selection_start_utf16, 6);
    assert!(item.detail.contains("preamble/macros.sty:3"), "{}", item.detail);
}

#[test]
fn dirty_other_document_and_undo_override_cached_disk_symbols() {
    let project = Project::new(); project.write("macros.tex", "\\newcommand{\\mymacroOld}[1]{}");
    let (mut workspace, _, caret) = project.open("\\mym<caret>");
    assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["\\mymacroOld"]);
    let old = workspace.open_document("macros.tex").unwrap();
    let changed = workspace.apply_edits("macros.tex", 1, vec![TextEdit { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: old.text.encode_utf16().count() as u32 }, text: "\\newcommand{\\mymacroNew}[2]{}".into() }], "agent").unwrap();
    assert!(changed.dirty);
    let set = workspace.completions("main.tex", 1, caret).unwrap();
    assert_eq!(labels(&set), vec!["\\mymacroNew"]);
    workspace.undo().unwrap();
    let restored = workspace.completions("main.tex", 1, caret).unwrap();
    assert_eq!(labels(&restored), vec!["\\mymacroOld"]);
    assert_eq!(fs::read_to_string(project.0.join("macros.tex")).unwrap(), "\\newcommand{\\mymacroOld}[1]{}");
}

#[test]
fn ref_keys_replace_the_whole_current_key_with_utf16_coordinates() {
    let project = Project::new(); project.write("chapters/a.tex", "\\label{sec:overview}\n\\label{eq:energy}");
    let (mut workspace, source, caret) = project.open("😀 \\ref{sec:o<caret>ld}");
    let set = workspace.completions("main.tex", 1, caret).unwrap();
    assert_eq!((set.replace_start_utf16, set.replace_length_utf16), (8, 7));
    assert_eq!(set.prefix, "sec:o"); assert_eq!(labels(&set), vec!["sec:overview"]);
    assert_eq!(inserted(&source, &set, "sec:overview"), "😀 \\ref{sec:overview}");
}

#[test]
fn ref_lists_and_citation_options_preserve_neighbors_and_hide_used_keys() {
    let project = Project::new(); project.write("a.tex", "\\label{sec:one}\\label{sec:two}\\label{sec:three}");
    project.write("references.bib", "@article{smith2026, title={A}}\n@book{smith2020, title={B}}\n@article{alpha, title={C}}");
    let set = project.complete("\\cref{sec:one, sec:t<caret>wo}");
    assert_eq!(labels(&set), vec!["sec:three", "sec:two"]);
    let (mut workspace, source, caret) = project.open("\\citep*[see][p.~2]{alpha, smi<caret>ZZ, smith2020}");
    let set = workspace.completions("main.tex", 1, caret).unwrap();
    assert_eq!(labels(&set), vec!["smith2026"]);
    assert_eq!(inserted(&source, &set, "smith2026"), "\\citep*[see][p.~2]{alpha, smith2026, smith2020}");
}

#[test]
fn missing_closing_brace_is_added_without_duplicating_an_existing_one() {
    let project = Project::new(); project.write("refs.bib", "@book{knuth1984, title={The Book}}");
    for (marked, expected) in [("\\cite{kn<caret>", "\\cite{knuth1984}"), ("\\cite{kn<caret>}", "\\cite{knuth1984}")] {
        let (mut workspace, source, caret) = project.open(marked);
        let set = workspace.completions("main.tex", 1, caret).unwrap();
        assert_eq!(inserted(&source, &set, "knuth1984"), expected);
    }
}

#[test]
fn bibtex_parser_skips_pseudoentries_and_nested_fake_entries() {
    let project = Project::new();
    project.write("refs.bib", r#"@comment{ @article{fakeComment, title={X}} }
@string{journal = "@article{fakeString, x}"}
@preamble{"@book{fakePreamble, x}"}
@article{realOne, title={Nested {Title} @book{fakeTitle, x}}, note="@misc{fakeQuote, x}"}
@Book(realTwo, title="Quoted {title}")
@misc{árvíz2026, title={Unicode key}}
"#);
    let set = project.complete("\\cite{<caret>}");
    assert_eq!(labels(&set), vec!["realOne", "realTwo", "árvíz2026"]);
    assert!(set.items.iter().find(|item| item.label == "realTwo").unwrap().detail.contains("book"));
}

#[test]
fn comments_and_verbatim_are_not_definitions_or_completion_contexts() {
    let project = Project::new();
    project.write("labels.tex", "% \\label{fakeComment}\n\\begin{comment}\\label{fakeCommentEnv}\\end{comment}\n\\begin{verbatim}\n\\label{fakeBlock}\n\\end{verbatim}\n\\verb|\\label{fakeVerb}|\n\\begin{minted}{tex}\n\\label{fakeMinted}\n\\end{minted}\n\\label{real}");
    assert_eq!(labels(&project.complete("\\ref{<caret>}")), vec!["real"]);
    for marked in ["% \\ref{re<caret>}", "\\verb|\\ref{re<caret>}|", "\\begin{verbatim}\n\\ref{re<caret>}", "\\begin{lstlisting}\n\\ref{re<caret>}", "\\begin{minted}{tex}\n\\ref{re<caret>}"] {
        assert!(project.complete(marked).items.is_empty(), "{marked}");
    }
}

#[test]
fn conservative_macro_signatures_keep_optional_and_delimited_arguments_safe() {
    let project = Project::new();
    project.write("macros.tex", r#"\newcommand*\myoptional[2][x]{#1#2}
\DeclareRobustCommand{\myrobust}[1]{#1}
\DeclareMathOperator*{\myoperator}{argmax}
\def\myplain#1#2{#1#2}
\gdef\mydelimited#1,#2;{#1#2}
\NewDocumentCommand{\mydocument}{s O{x} +m m}{#3#4}
\NewDocumentCommand{\mycomplex}{r() m}{#1#2}
"#);
    for (prefix, expected) in [("myopt", "\\myoptional{}"), ("myrob", "\\myrobust{}"), ("myoper", "\\myoperator"), ("myplain", "\\myplain{}{}"), ("mydel", "\\mydelimited"), ("mydoc", "\\mydocument{}{}"), ("mycomp", "\\mycomplex")] {
        let set = project.complete(&format!("\\{prefix}<caret>"));
        assert_eq!(set.items.first().map(|item| item.insert_text.as_str()), Some(expected), "{prefix}");
    }
    let set = project.complete("\\myopt<caret>[given]{body}");
    assert_eq!(set.items[0].insert_text, "\\myoptional");
}

#[test]
fn duplicate_definitions_are_deduplicated_and_active_source_overrides_builtin() {
    let project = Project::new(); project.write("a.tex", "\\newcommand{\\section}[1]{}\\label{same}");
    project.write("b.tex", "\\newcommand{\\section}[3]{}\\label{same}");
    let set = project.complete("\\newcommand{\\section}[2]{}\n\\sec<caret>");
    assert_eq!(set.items.iter().filter(|item| item.label == "\\section").count(), 1);
    assert_eq!(set.items[0].insert_text, "\\section{}{}");
    assert!(set.items[0].detail.contains("main.tex:1"));
    assert_eq!(labels(&project.complete("\\ref{sa<caret>}")), vec!["same"]);
}

#[test]
fn indexing_many_unopened_files_does_not_consume_editor_slots_or_mutate_history() {
    let project = Project::new();
    for index in 0..32 { project.write(&format!("chapters/{index:02}.tex"), &format!("\\label{{chapter:{index:02}}}")); }
    let (mut workspace, source, caret) = project.open("\\ref{chapter:31<caret>}");
    let set = workspace.completions("main.tex", 1, caret).unwrap();
    assert_eq!(labels(&set), vec!["chapter:31"]);
    assert_eq!(workspace.document_infos().len(), 1); assert!(!workspace.can_undo());
    let snapshot = workspace.snapshot("main.tex").unwrap();
    assert_eq!(snapshot.text, source); assert_eq!(snapshot.revision, 1); assert!(!snapshot.dirty);
}

#[test]
fn oversized_invalid_and_hidden_sources_do_not_break_valid_project_completion() {
    let project = Project::new(); project.write("ok.tex", "\\label{good}");
    project.write(".latexide/ignored.tex", "\\label{hidden}");
    project.write("huge.tex", &("x".repeat(1024 * 1024) + "\\label{huge}"));
    fs::write(project.0.join("bad.bib"), [0xff, 0xfe]).unwrap();
    assert_eq!(labels(&project.complete("\\ref{<caret>}")), vec!["good"]);
}

#[cfg(unix)]
#[test]
fn project_index_does_not_follow_links_outside_its_root() {
    let project = Project::new(); let outside = Project::new();
    project.write("ok.tex", "\\label{safe}"); outside.write("private.tex", "\\label{outside}");
    std::os::unix::fs::symlink(outside.0.join("private.tex"), project.0.join("linked.tex")).unwrap();
    std::os::unix::fs::symlink(&outside.0, project.0.join("linked-folder")).unwrap();
    assert_eq!(labels(&project.complete("\\ref{<caret>}")), vec!["safe"]);
}

#[test]
fn forced_refresh_removes_changed_and_deleted_disk_symbols_and_discovers_new_files() {
    let project = Project::new(); project.write("a.tex", "\\label{sec:old}");
    let (mut workspace, _, caret) = project.open("\\ref{sec:<caret>}");
    assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["sec:old"]);
    project.write("a.tex", "\\label{sec:new}"); project.write("b.tex", "\\label{sec:added}");
    workspace.refresh_completion_index();
    assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["sec:added", "sec:new"]);
    fs::remove_file(project.0.join("a.tex")).unwrap(); workspace.refresh_completion_index();
    assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["sec:added"]);
    assert_eq!(workspace.snapshot("main.tex").unwrap().revision, 1); assert!(!workspace.can_undo());
}

#[test]
fn partial_file_and_symbol_coverage_is_visible_and_candidate_count_stays_bounded() {
    let project = Project::new();
    for n in 0..130 { project.write(&format!("chapters/{n:03}.tex"), &format!("\\label{{key:{n:03}}}")); }
    let set = project.complete("\\ref{key:<caret>}");
    assert_eq!(set.items.len(), 20);
    assert_eq!(set.index_status.as_ref().unwrap().indexed_files, 128);
    assert!(set.index_status.as_ref().unwrap().truncated);
    let project = Project::new();
    project.write("many.tex", &(0..8193).map(|n| format!("\\label{{key:{n:04}}}\n")).collect::<String>());
    let set = project.complete("\\ref{key:819<caret>}");
    assert_eq!(labels(&set), vec!["key:8190", "key:8191"]);
    assert!(set.index_status.unwrap().truncated);
}

#[test]
fn aggregate_source_budget_preserves_valid_results_and_reports_partial_coverage() {
    let project = Project::new();
    for n in 0..9 {
        let mut text = format!("\\label{{key:{n}}}\n"); text.push_str(&"x".repeat(1024 * 1024 - text.len()));
        project.write(&format!("large/{n}.tex"), &text);
    }
    let set = project.complete("\\ref{key:<caret>}");
    assert_eq!(labels(&set), vec!["key:0", "key:1", "key:2", "key:3", "key:4", "key:5", "key:6"]);
    assert!(set.index_status.as_ref().unwrap().truncated);
    assert!(set.index_status.unwrap().skipped_files > 0);
}

#[test]
fn malformed_tex_and_bibtex_stay_safe_at_every_scalar_caret() {
    let project = Project::new(); project.write("malformed.bib", "@article{broken, title={nested\n @book{notARealEntry, x}");
    for text in ["\\newcommand", "\\newcommand{", "\\newcommand{\\😀}", "\\def\\foo#1,", "\\NewDocumentCommand{\\foo}{O{", "\\begin{minted}", "\\verb😀text", "\\cite[see]{😀,", "\\ref{sec:foo\\bar}"] {
        project.write("main.tex", text);
        let mut workspace = Workspace::open(project.0.clone()).unwrap(); workspace.open_document("main.tex").unwrap();
        let mut caret = 0;
        for ch in text.chars() {
            let set = workspace.completions("main.tex", 1, caret).unwrap();
            assert!(set.items.len() <= 20); caret += ch.len_utf16() as u32;
        }
        workspace.completions("main.tex", 1, caret).unwrap();
    }
}

#[test]
fn report_cold_and_warm_query_cost_on_a_real_bounded_project() {
    let project = Project::new();
    for n in 0..100 { project.write(&format!("chapters/{n:03}.tex"), &(format!("\\label{{sec:{n:03}}}\n") + &"Plain project text.\n".repeat(2000))); }
    let (mut workspace, _, caret) = project.open("\\ref{sec:099<caret>}");
    let cold = std::time::Instant::now();
    assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["sec:099"]);
    let cold_ms = cold.elapsed().as_secs_f64() * 1000.0;
    let mut warm = Vec::new();
    for _ in 0..50 {
        let now = std::time::Instant::now();
        assert_eq!(labels(&workspace.completions("main.tex", 1, caret).unwrap()), vec!["sec:099"]);
        warm.push(now.elapsed().as_secs_f64() * 1000.0);
    }
    warm.sort_by(f64::total_cmp);
    println!("PROJECT_COMPLETION_DEBUG_TIMING files=101 source_bytes~=3800000 cold_ms={cold_ms:.3} warm_p50_ms={:.3} warm_p95_ms={:.3}", warm[25], warm[47]);
}

#[test]
fn a_group_beyond_the_context_window_cannot_create_a_false_closing_brace() {
    let project = Project::new(); project.write("labels.tex", "\\label{key:one}");
    let marked = "\\ref{key:<caret>,".to_owned() + &"other,".repeat(3000) + "last}";
    assert!(project.complete(&marked).items.is_empty(), "An unverified truncated argument must not insert an extra closing brace");
}

#[test]
fn completing_an_earlier_key_in_an_unfinished_list_keeps_later_keys_inside_it() {
    let project = Project::new(); project.write("labels.tex", "\\label{alpha}\\label{beta}");
    project.write("refs.bib", "@book{alpha, title={A}}\n@book{beta, title={B}}");
    for (marked, expected) in [("\\cite{al<caret>, beta", "\\cite{alpha, beta"), ("\\cref{al<caret>, beta", "\\cref{alpha, beta")] {
        let (mut workspace, source, caret) = project.open(marked);
        let set = workspace.completions("main.tex", 1, caret).unwrap();
        assert_eq!(labels(&set), vec!["alpha"]);
        assert_eq!(inserted(&source, &set, "alpha"), expected);
    }
}

#[test]
fn empty_whitespace_list_segments_offer_unused_keys_without_removing_spaces() {
    let project = Project::new(); project.write("labels.tex", "\\label{alpha}\\label{beta}");
    project.write("refs.bib", "@book{alpha, title={A}}\n@book{beta, title={B}}");
    for (marked, chosen, expected) in [
        ("\\cite{alpha, <caret>}", "beta", "\\cite{alpha, beta}"),
        ("\\cite{ <caret> }", "alpha", "\\cite{ alpha }"),
        ("\\cref{alpha,\t<caret> }", "beta", "\\cref{alpha,\tbeta }")
    ] {
        let (mut workspace, source, caret) = project.open(marked);
        let set = workspace.completions("main.tex", 1, caret).unwrap();
        assert!(labels(&set).contains(&chosen), "{marked}");
        assert_eq!(set.replace_length_utf16, 0);
        assert_eq!(inserted(&source, &set, chosen), expected);
        if marked.contains("alpha,") { assert!(!labels(&set).contains(&"alpha")); }
    }
}

#[test]
fn near_limit_malformed_source_cannot_repeat_overlapping_group_lookahead() {
    let project = Project::new(); project.write("refs.bib", "@book{alpha, title={A}}");
    let mut slow = Vec::new();
    for template in ["\\begin{", "\\newcommand{", "\\newcommand\\foo[", "\\NewDocumentCommand{\\foo}{", "\\def\\foo#1"] {
        let marked = template.repeat(900_000 / template.len()) + "\\cite{al<caret>";
        let (mut workspace, _, caret) = project.open(&marked);
        let start = std::time::Instant::now(); let set = workspace.completions("main.tex", 1, caret).unwrap();
        let elapsed = start.elapsed(); assert!(set.items.len() <= 20);
        println!("MALFORMED_COMPLETION template={template:?} bytes~=900000 elapsed_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
        // Generous UI regression guard, not a release-performance benchmark.
        if elapsed >= std::time::Duration::from_secs(1) { slow.push((template, elapsed)); }
    }
    assert!(slow.is_empty(), "Synchronous malformed-source queries stalled: {slow:?}");
}
