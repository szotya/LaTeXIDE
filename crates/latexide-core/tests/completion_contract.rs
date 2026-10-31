use latexide_core::completion::complete;

#[test]
fn command_candidates_replace_the_whole_word_and_place_the_first_argument() {
    let set = complete("Árvíz 😀 \\section", 13).unwrap(); // caret after \\sec, before tion
    assert_eq!(set.prefix, "sec");
    assert_eq!((set.replace_start_utf16, set.replace_length_utf16), (9, 8));
    let item = set.items.iter().find(|item| item.label == "\\section").unwrap();
    assert_eq!(item.insert_text, "\\section{}");
    assert_eq!((item.selection_start_utf16, item.selection_length_utf16), (9, 0));
}

#[test]
fn comments_verbatim_and_escaped_backslash_do_not_offer_commands() {
    for text in ["% \\sec", "\\begin{verbatim}\n\\sec", "\\verb|\\sec", "\\\\sec", "plain prose", "\\begin{lstlisting}\n\\sec"] {
        assert!(complete(text, text.encode_utf16().count() as u32).unwrap().items.is_empty(), "{text}");
    }
    let source = "\\% \\sec";
    assert!(complete(source, source.len() as u32).unwrap().items.iter().any(|i| i.label == "\\section"));
}

#[test]
fn candidates_are_bounded_and_unrelated_prefix_is_empty() {
    assert!(complete("\\", 1).unwrap().items.len() <= 20);
    assert!(complete("\\", 1).unwrap().items.len() > 3);
    assert!(complete("\\zzzzunlikely", 13).unwrap().items.is_empty());
    let set = complete("$ \\fra", 6).unwrap();
    let frac = set.items.iter().find(|item| item.label == "\\frac").unwrap();
    assert_eq!(frac.insert_text, "\\frac{}{}");
    assert_eq!(frac.selection_start_utf16, 6);
    assert_eq!(set.items[0].label, "\\frac");
}

#[test]
fn invalid_scalar_caret_is_rejected_without_splitting_emoji() {
    assert!(complete("😀\\sec", 1).is_err());
    assert!(complete("\\sec", 99).is_err());
    let source = "😀\\sec";
    let set = complete(source, 6).unwrap();
    assert_eq!((set.replace_start_utf16, set.replace_length_utf16), (2, 4));
}

#[test]
fn completion_preserves_arguments_the_user_already_wrote() {
    let set = complete("\\sec{Saját cím}", 4).unwrap();
    let section = set.items.iter().find(|item| item.label == "\\section").unwrap();
    assert_eq!(section.insert_text, "\\section");
    assert_eq!(section.selection_start_utf16, 8);
}

#[test]
fn starred_commands_keep_the_existing_star_and_arguments() {
    for source in ["\\section*{Title}", "\\section*[Short]{Title}", "\\section *{Title}", "\\section*"] {
        let set = complete(source, 4).unwrap();
        let item = set.items.iter().find(|item| item.label == "\\section").unwrap();
        assert_eq!(item.insert_text, "\\section", "{source}");
        assert_eq!((set.replace_start_utf16,set.replace_length_utf16),(0,8));
    }
}
