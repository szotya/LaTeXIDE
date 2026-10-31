use latexide_core::syntax::syntax_spans;

fn offset(text: &str, needle: &str) -> u32 { text[..text.find(needle).unwrap()].encode_utf16().count() as u32 }
fn kind_at(text: &str, needle: &str) -> Option<u32> {
    let position = offset(text, needle);
    syntax_spans(text, 0, 32768).into_iter().find(|span| span[0] <= position && position < span[0] + span[1]).map(|span| span[2])
}
#[test]
fn prose_remains_plain_while_commands_groups_and_comments_are_distinct() {
    let text = "Árvíz 😀 \\section{Bevezetés} normál \\textbf{szöveg} % \\ignored";
    assert_eq!(kind_at(text, "Árvíz"), None);
    assert_eq!(kind_at(text, "😀"), None);
    assert_eq!(kind_at(text, "\\section"), Some(1));
    assert_eq!(kind_at(text, "{Bevezetés"), Some(3));
    assert_eq!(kind_at(text, "Bevezetés"), None);
    assert_eq!(kind_at(text, "szöveg"), None);
    assert_eq!(kind_at(text, "\\ignored"), Some(4));
}
#[test]
fn escaped_percent_and_dollar_do_not_start_comment_or_math() {
    let text = r"Költség \$5 és \% jel % valódi komment";
    assert_eq!(kind_at(text, r"\$"), Some(1));
    assert_eq!(kind_at(text, r"\%"), Some(1));
    assert_eq!(kind_at(text, "jel"), None);
    assert_eq!(kind_at(text, "valódi"), Some(4));
}
#[test]
fn math_and_environment_context_survives_newlines() {
    let text = "Előtte \\[\nx_1 = 3.14 + \\alpha\n\\] utána\n\\begin{align}\ny &= 2\\\\\n\\end{align}";
    assert_eq!(kind_at(text, "x"), Some(5));
    assert_eq!(kind_at(text, "_"), Some(9));
    assert_eq!(kind_at(text, "3.14"), Some(8));
    assert_eq!(kind_at(text, "\\alpha"), Some(1));
    assert_eq!(kind_at(text, "utána"), None);
    assert_eq!(kind_at(text, "align"), Some(2));
    assert_eq!(kind_at(text, "y &"), Some(5));
}
#[test]
fn literal_arguments_and_option_keys_are_not_colored_as_prose() {
    let text = r"\documentclass[a4paper,11pt]{article}\usepackage{fontspec}\setmainfont{font.otf}[BoldFont=bold.otf]\input{chapters/intro.tex}\section{Saját cím}";
    assert_eq!(kind_at(text, "article"), Some(7));
    assert_eq!(kind_at(text, "fontspec"), Some(7));
    assert_eq!(kind_at(text, "font.otf"), Some(7));
    assert_eq!(kind_at(text, "BoldFont"), Some(10));
    assert_eq!(kind_at(text, "chapters/"), Some(7));
    assert_eq!(kind_at(text, "Saját"), None);
}
#[test]
fn verbatim_does_not_interpret_tex_comments_or_commands() {
    let text = r"\verb|% $ \section{x}| próza \begin{verbatim}% \notcommand \end{verbatim}";
    assert_eq!(kind_at(text, "% $"), Some(7));
    assert_eq!(kind_at(text, "próza"), None);
    assert_eq!(kind_at(text, "\\notcommand"), Some(7));
}
#[test]
fn visible_window_keeps_context_and_unicode_scalar_boundaries() {
    let text = "😀 próza\n\\begin{equation}\nx^2 + y^2 = 1\n\\end{equation}\n";
    let start = offset(text, "y^2");
    let spans = syntax_spans(text, start, 8);
    assert!(spans.iter().any(|span| span[0] <= start && span[0] + span[1] > start && span[2] == 5));
    let units: Vec<_> = text.encode_utf16().collect();
    let boundary = |offset: usize| offset == 0 || offset == units.len() || !(0xD800..=0xDBFF).contains(&units[offset-1]) || !(0xDC00..=0xDFFF).contains(&units[offset]);
    for span in &spans { assert!(boundary(span[0] as usize)); assert!(boundary((span[0]+span[1]) as usize)); }
    for pair in spans.windows(2) { assert!(pair[0][0] + pair[0][1] <= pair[1][0]); }
    let dense = "{".repeat(1024*1024);
    let bounded = syntax_spans(&dense, 0, u32::MAX);
    assert!(bounded.iter().all(|span| span[0] + span[1] <= 32768));
}

#[test]
fn optional_title_delimiters_and_text_inside_math_keep_prose_plain() {
    let text = r"\section[Rövid]{Saját cím} \(x + \text{mért érték}\)";
    assert_eq!(kind_at(text, "[Rövid"), Some(3));
    assert_eq!(kind_at(text, "Rövid"), None);
    assert_eq!(kind_at(text, "mért"), None);
    assert_eq!(kind_at(text, "\\text"), Some(1));
}

#[test]
fn late_window_in_a_large_source_stays_bounded() {
    let line = "Magyar próza 😀 \\section{Cím} $x_1=2$\n";
    let text = line.repeat(20000);
    let length = text.encode_utf16().count() as u32;
    let start = std::time::Instant::now();
    let spans = syntax_spans(&text, length - 1000, 500);
    eprintln!("large-source syntax: {} bytes, tail window500 UTF16, {:?}", text.len(), start.elapsed());
    assert!(!spans.is_empty());
    assert!(spans.len() < 500);
    assert!(spans.iter().all(|span| span[0] < length - 500 && span[0] + span[1] > length - 1000));
}
