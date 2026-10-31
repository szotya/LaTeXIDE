//! Borrowed lexical TeX tokens. No expansion, catcode changes or compiler execution.
use std::ops::Range;

pub(crate) struct Command<'a> { pub name: &'a str, pub start: usize, pub end: usize }
pub(crate) enum Token<'a> { Command(Command<'a>), Ignored(Range<usize>) }
pub(crate) struct Tokens<'a> { text: &'a str, byte: usize }
impl<'a> Tokens<'a> {
    pub fn new(text: &'a str) -> Self { Self { text, byte: 0 } }
    fn advance(&mut self, end: usize) { self.byte = end; }
}
impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        while self.byte < self.text.len() {
            let start = self.byte;
            match self.text.as_bytes()[start] {
                b'%' => {
                    let end = self.text[start..].find('\n').map_or(self.text.len(), |n| start + n);
                    self.advance(end); return Some(Token::Ignored(start..end));
                }
                b'\\' => {
                    let mut end = start + 1;
                    while end < self.text.len() && is_letter(self.text.as_bytes()[end]) { end += 1; }
                    if end == start + 1 && end < self.text.len() { end += self.text[end..].chars().next().unwrap().len_utf8(); }
                    let name = &self.text[start + 1..end];
                    if name == "verb" {
                        let mut p = end; if self.text.as_bytes().get(p) == Some(&b'*') { p += 1; }
                        if let Some(delimiter) = self.text[p..].chars().next().filter(|c| !c.is_whitespace()) {
                            p += delimiter.len_utf8();
                            let tail = &self.text[p..];
                            let stop = tail.find([delimiter, '\n']).map_or(self.text.len(), |n| p + n);
                            let finish = if self.text[stop..].starts_with(delimiter) { stop + delimiter.len_utf8() } else { stop };
                            self.advance(finish); return Some(Token::Ignored(start..finish));
                        }
                    }
                    if name == "begin" {
                        // Only a short literal environment name matters here. Do not
                        // rescan a malformed document-sized group for each \begin.
                        if let Some(g) = group_limited(self.text, end, b'{', b'}', 64) {
                            let environment = self.text[g.content.clone()].trim();
                            if g.closed && matches!(environment, "verbatim" | "verbatim*" | "Verbatim" | "lstlisting" | "minted" | "comment") {
                                let closing = format!("\\end{{{environment}}}");
                                let finish = self.text[g.end..].find(&closing).map_or(self.text.len(), |n| g.end + n + closing.len());
                                self.advance(finish); return Some(Token::Ignored(start..finish));
                            }
                        }
                    }
                    self.advance(end); return Some(Token::Command(Command { name, start, end }));
                }
                _ => {
                    // Prose cannot start a control sequence/comment: skip it in one borrowed slice.
                    let end = self.text[start..].find(['\\', '%']).map_or(self.text.len(), |n| start + n);
                    self.advance(end);
                }
            }
        }
        None
    }
}
pub(crate) fn is_letter(byte: u8) -> bool { byte.is_ascii_alphabetic() || byte == b'@' }
pub(crate) fn skip_space(text: &str, mut p: usize) -> usize {
    while p < text.len() {
        let ch = text[p..].chars().next().unwrap();
        if ch.is_whitespace() { p += ch.len_utf8(); }
        else if ch == '%' { p = text[p..].find('\n').map_or(text.len(), |n| p + n); }
        else { break; }
    }
    p
}
pub(crate) struct Group { pub content: Range<usize>, pub end: usize, pub closed: bool }
/// Bounded lookahead for definitions/options/keys; incomplete groups retain their range.
pub(crate) fn group(text: &str, p: usize, open: u8, close: u8) -> Option<Group> {
    group_limited(text, p, open, close, 16_384)
}
pub(crate) fn group_limited(text: &str, p: usize, open: u8, close: u8, maximum_bytes: usize) -> Option<Group> {
    let p = skip_space(text, p);
    if text.as_bytes().get(p) != Some(&open) { return None; }
    let start = p + 1; let mut cursor = start; let mut depth = 1; let mut braces: usize = 0;
    let limit = text.len().min(start.saturating_add(maximum_bytes));
    while cursor < limit {
        let ch = text.as_bytes()[cursor];
        if ch == b'\\' { cursor += 1; if cursor < text.len() { cursor += text[cursor..].chars().next().unwrap().len_utf8(); } continue; }
        if ch == b'%' { cursor = text[cursor..].find('\n').map_or(text.len(), |n| cursor + n); continue; }
        if open == b'[' && ch == b'{' { braces += 1; }
        else if open == b'[' && ch == b'}' { braces = braces.saturating_sub(1); }
        else if braces == 0 && ch == open { depth += 1; }
        else if braces == 0 && ch == close {
            depth -= 1;
            if depth == 0 { return Some(Group { content: start..cursor, end: cursor + 1, closed: true }); }
        }
        cursor += text[cursor..].chars().next().unwrap().len_utf8();
    }
    Some(Group { content: start..cursor.min(text.len()), end: cursor.min(text.len()), closed: false })
}
pub(crate) fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 256 && key.chars().all(|c| c.is_alphanumeric() || "_:-./+!?@".contains(c))
}
