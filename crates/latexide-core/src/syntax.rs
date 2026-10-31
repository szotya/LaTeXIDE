//! Bounded lexical LaTeX coloring. Offsets are absolute UTF-16 code units.
//! Kind IDs are a native presentation contract: 1 command, 2 environment,
//! 3 delimiter, 4 comment, 5 math, 6 parameter, 7 literal, 8 number,
//! 9 operator, 10 option key. Prose has no span.

const WINDOW_LIMIT: u32 = 32768;
struct Cursor<'a> { text: &'a str, byte: usize, utf16: u32 }
impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<char> { self.text[self.byte..].chars().next() }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?; self.byte += ch.len_utf8(); self.utf16 += ch.len_utf16() as u32; Some(ch)
    }
    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&predicate) { self.bump(); }
    }
}
fn literal_command(name: &str) -> bool {
    matches!(name.trim_end_matches('*'), "documentclass" | "usepackage" | "RequirePackage" |
        "input" | "include" | "includegraphics" | "bibliography" | "bibliographystyle" |
        "addbibresource" | "setmainfont" | "setsansfont" | "setmonofont" |
        "label" | "ref" | "eqref" | "autoref" | "pageref" | "cite" | "citep" | "citet" |
        "url" | "href" | "newcommand" | "renewcommand" | "providecommand")
}
fn math_environment(name: &str) -> bool {
    matches!(name.trim_end_matches('*'), "math" | "displaymath" | "equation" | "align" |
        "aligned" | "gather" | "gathered" | "multline" | "flalign" | "alignat" |
        "eqnarray" | "cases" | "split" | "matrix" | "pmatrix" | "bmatrix" | "vmatrix")
}
fn emit(spans: &mut Vec<[u32; 3]>, start: u32, end: u32, kind: u32, window: (u32, u32)) {
    if start == end || end <= window.0 || start >= window.1 { return; }
    if let Some(last) = spans.last_mut() {
        if last[2] == kind && last[0] + last[1] == start { last[1] = end - last[0]; return; }
    }
    spans.push([start, end - start, kind]);
}

/// Recover context from the document prefix, return only intersecting tokens.
/// The workspace bounds source size to 1 MiB; no text copies or per-char arrays.
/// Complete tokens may extend outside the requested window (e.g. a comment).
pub fn syntax_spans(text: &str, offset_utf16: u32, length_utf16: u32) -> Vec<[u32; 3]> {
    let window = (offset_utf16, offset_utf16.saturating_add(length_utf16.min(WINDOW_LIMIT)));
    if window.0 == window.1 { return Vec::new(); }
    let mut cursor = Cursor { text, byte: 0, utf16: 0 };
    let mut spans = Vec::new();
    let mut inline_math = false;
    let mut math_environments = 0u32;
    let mut braces = 0u32;
    let mut literal_brace = None;
    let mut pending_literal = false;
    let mut pending_prose = false;
    let mut prose_brace = None;
    let mut last_command = false;
    let mut optional_prose = 0u32;
    let mut last_literal = false;
    let mut optional_literal = 0u32;
    let mut verbatim: Option<&str> = None;
    while cursor.byte < text.len() && cursor.utf16 < window.1 {
        if let Some(environment) = verbatim {
            let closing = if environment == "verbatim" { "\\end{verbatim}" } else { "\\end{lstlisting}" };
            if !text[cursor.byte..].starts_with(closing) {
                let until = text[cursor.byte..].find(closing).map_or(text.len(), |offset| cursor.byte + offset);
                let start = cursor.utf16;
                while cursor.byte < until { cursor.bump(); }
                emit(&mut spans, start, cursor.utf16, 7, window);
                continue;
            }
            verbatim = None;
        }
        let start = cursor.utf16;
        let ch = cursor.bump().unwrap();
        let math = (inline_math || math_environments > 0) && prose_brace.is_none();
        match ch {
            '%' => {
                cursor.consume_while(|ch| ch != '\n');
                emit(&mut spans, start, cursor.utf16, 4, window);
            }
            '\\' => {
                let command_start = cursor.byte;
                if cursor.peek().is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '@') {
                    cursor.consume_while(|ch| ch.is_ascii_alphabetic() || ch == '@');
                    if cursor.peek() == Some('*') { cursor.bump(); }
                    let command = &text[command_start..cursor.byte];
                    emit(&mut spans, start, cursor.utf16, 1, window);
                    pending_literal = literal_command(command); last_literal = pending_literal; last_command = true;
                    pending_prose = math && matches!(command.trim_end_matches('*'), "text" | "mbox" | "textrm" | "textnormal" | "textbf" | "textit" | "textsf" | "texttt");
                    if command == "begin" || command == "end" {
                        cursor.consume_while(char::is_whitespace);
                        if cursor.peek() == Some('{') {
                            let open = cursor.utf16; cursor.bump(); emit(&mut spans, open, cursor.utf16, 3, window);
                            let environment_start = cursor.byte; let name_start = cursor.utf16;
                            cursor.consume_while(|ch| ch != '}' && ch != '\n');
                            let environment = &text[environment_start..cursor.byte];
                            emit(&mut spans, name_start, cursor.utf16, 2, window);
                            if cursor.peek() == Some('}') {
                                let close = cursor.utf16; cursor.bump(); emit(&mut spans, close, cursor.utf16, 3, window);
                                if math_environment(environment) {
                                    if command == "begin" { math_environments += 1; }
                                    else { math_environments = math_environments.saturating_sub(1); }
                                }
                                if command == "begin" && matches!(environment, "verbatim" | "lstlisting") { verbatim = Some(environment); }
                            }
                        }
                    } else if command.trim_end_matches('*') == "verb" {
                        if let Some(delimiter) = cursor.peek().filter(|ch| !ch.is_whitespace()) {
                            let content = cursor.utf16; cursor.bump();
                            cursor.consume_while(|ch| ch != delimiter && ch != '\n');
                            if cursor.peek() == Some(delimiter) { cursor.bump(); }
                            emit(&mut spans, content, cursor.utf16, 7, window);
                        }
                    }
                } else if let Some(symbol) = cursor.bump() {
                    let kind = if matches!(symbol, '[' | ']' | '(' | ')') {
                        inline_math = matches!(symbol, '[' | '('); 3
                    } else { 1 };
                    emit(&mut spans, start, cursor.utf16, kind, window);
                } else { emit(&mut spans, start, cursor.utf16, 1, window); }
            }
            '{' => {
                braces += 1; last_command = false;
                if pending_prose { prose_brace = Some(braces); pending_prose = false; }
                if pending_literal { literal_brace = Some(braces); pending_literal = false; }
                emit(&mut spans, start, cursor.utf16, 3, window);
            }
            '}' => {
                if literal_brace == Some(braces) { literal_brace = None; }
                if prose_brace == Some(braces) { prose_brace = None; }
                braces = braces.saturating_sub(1);
                emit(&mut spans, start, cursor.utf16, 3, window);
            }
            '[' if math || pending_literal || last_literal || last_command || optional_literal > 0 || optional_prose > 0 => {
                if !math { if last_literal || optional_literal > 0 { optional_literal += 1; } else { optional_prose += 1; } }
                last_command = false; emit(&mut spans, start, cursor.utf16, 3, window);
            }
            ']' if math || optional_literal > 0 || optional_prose > 0 => {
                if optional_literal > 0 { optional_literal -= 1; } else { optional_prose = optional_prose.saturating_sub(1); }
                emit(&mut spans, start, cursor.utf16, 3, window);
            }
            '$' => {
                if cursor.peek() == Some('$') { cursor.bump(); }
                inline_math = !inline_math; emit(&mut spans, start, cursor.utf16, 3, window);
            }
            '#' => { cursor.consume_while(|ch| ch.is_ascii_digit()); emit(&mut spans, start, cursor.utf16, 6, window); }
            '^' | '_' | '&' | '~' => { emit(&mut spans, start, cursor.utf16, 9, window); }
            '=' | ',' if optional_literal > 0 || math => { emit(&mut spans, start, cursor.utf16, 9, window); }
            ch if optional_literal > 0 && ch.is_ascii_alphabetic() => {
                cursor.consume_while(|ch| ch.is_ascii_alphanumeric() || ch == '_');
                let tail = text[cursor.byte..].trim_start();
                emit(&mut spans, start, cursor.utf16, if tail.starts_with('=') { 10 } else { 7 }, window);
            }
            ch if math && ch.is_ascii_digit() => {
                cursor.consume_while(|ch| ch.is_ascii_digit() || ch == '.'); emit(&mut spans, start, cursor.utf16, 8, window);
            }
            _ => {
                if literal_brace.is_some() || optional_literal > 0 { emit(&mut spans, start, cursor.utf16, 7, window); }
                else if math { emit(&mut spans, start, cursor.utf16, 5, window); }
                else if !ch.is_whitespace() { pending_literal = false; pending_prose = false; last_literal = false; last_command = false; }
            }
        }
    }
    spans
}
