//! Small authored command catalog; authoritative source and wire ranges are UTF-16.
use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use crate::{project_index::ProjectIndex, tex_scan::{self, Token, Tokens}};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionKind { #[default] Command, Macro, Reference, Citation }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionIndexStatus { pub indexed_files: u32, pub truncated: bool, pub skipped_files: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    #[serde(default)]
    pub kind: CompletionKind,
    pub label: String,
    pub detail: String,
    pub insert_text: String,
    pub selection_start_utf16: u32,
    pub selection_length_utf16: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionSet {
    pub replace_start_utf16: u32,
    pub replace_length_utf16: u32,
    pub prefix: String,
    pub items: Vec<CompletionItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_status: Option<CompletionIndexStatus>,
}
struct Command { name: &'static str, detail: &'static str, insertion: &'static str, selection: u32, length: u32 }
macro_rules! command {
    ($name:literal, $detail:literal, $insertion:literal, $selection:literal) => {
        Command { name: $name, detail: $detail, insertion: $insertion, selection: $selection, length: 0 }
    };
}
static COMMANDS: &[Command] = &[
    command!("section", "Fejezetcím · {cím}", "\\section{}", 9),
    command!("subsection", "Alfejezet · {cím}", "\\subsection{}", 12),
    command!("subsubsection", "Harmadszintű fejezet · {cím}", "\\subsubsection{}", 15),
    command!("chapter", "Könyv fejezete · {cím}", "\\chapter{}", 9),
    command!("paragraph", "Bekezdéscím · {cím}", "\\paragraph{}", 11),
    command!("title", "Dokumentum címe · {cím}", "\\title{}", 7),
    command!("author", "Szerző · {név}", "\\author{}", 8),
    command!("date", "Dátum · {dátum}", "\\date{}", 6),
    command!("maketitle", "Címoldal megjelenítése", "\\maketitle", 10),
    command!("documentclass", "Dokumentumosztály · {osztály}", "\\documentclass{}", 15),
    command!("usepackage", "Csomag betöltése · {csomag}", "\\usepackage{}", 12),
    command!("input", "Forrásfájl beillesztése · {útvonal}", "\\input{}", 7),
    command!("include", "Fejezetfájl beillesztése · {útvonal}", "\\include{}", 9),
    command!("label", "Hivatkozási címke · {kulcs}", "\\label{}", 7),
    command!("ref", "Hivatkozás címkére · {kulcs}", "\\ref{}", 5),
    command!("eqref", "Egyenletszám · {kulcs} · amsmath", "\\eqref{}", 7),
    command!("autoref", "Típusos hivatkozás · {kulcs} · hyperref", "\\autoref{}", 9),
    command!("pageref", "Hivatkozás oldalszámra · {kulcs}", "\\pageref{}", 9),
    command!("cite", "Irodalmi hivatkozás · {kulcs}", "\\cite{}", 6),
    command!("citep", "Zárójeles hivatkozás · {kulcs} · natbib", "\\citep{}", 7),
    command!("citet", "Szöveges hivatkozás · {kulcs} · natbib", "\\citet{}", 7),
    command!("bibliography", "Irodalomjegyzék fájlja · {fájl}", "\\bibliography{}", 14),
    command!("addbibresource", "Irodalmi adatbázis · {fájl} · biblatex", "\\addbibresource{}", 16),
    command!("printbibliography", "Irodalomjegyzék kiírása · biblatex", "\\printbibliography", 18),
    command!("begin", "Egyenlet-környezetpár · equation", "\\begin{equation}\n  \n\\end{equation}", 19),
    command!("end", "Környezet lezárása · {név}", "\\end{}", 5),
    command!("item", "Lista következő eleme", "\\item ", 6),
    command!("textbf", "Félkövér szöveg · {szöveg}", "\\textbf{}", 8),
    command!("textit", "Dőlt szöveg · {szöveg}", "\\textit{}", 8),
    command!("texttt", "Monospace szöveg · {szöveg}", "\\texttt{}", 8),
    command!("emph", "Szöveg kiemelése · {szöveg}", "\\emph{}", 6),
    command!("underline", "Aláhúzás · {szöveg}", "\\underline{}", 11),
    command!("footnote", "Lábjegyzet · {szöveg}", "\\footnote{}", 10),
    command!("url", "Webcím · {URL} · hyperref/url", "\\url{}", 5),
    command!("href", "Hivatkozás · {URL}{szöveg} · hyperref", "\\href{}{}", 6),
    command!("includegraphics", "Ábra beillesztése · {fájl} · graphicx", "\\includegraphics{}", 17),
    command!("caption", "Ábra- vagy táblázatfelirat · {szöveg}", "\\caption{}", 9),
    command!("centering", "Középre igazítás", "\\centering", 10),
    command!("frac", "Tört · {számláló}{nevező}", "\\frac{}{}", 6),
    command!("sqrt", "Négyzetgyök · {kifejezés}", "\\sqrt{}", 6),
    command!("sum", "Összeg · alsó és felső határral", "\\sum_{}^{}", 6),
    command!("int", "Integrál · alsó és felső határral", "\\int_{}^{}", 6),
    command!("prod", "Szorzat · alsó és felső határral", "\\prod_{}^{}", 7),
    command!("lim", "Határérték · {feltétel}", "\\lim_{}", 6),
    command!("sin", "Szinusz", "\\sin", 4), command!("cos", "Koszinusz", "\\cos", 4),
    command!("log", "Logaritmus", "\\log", 4), command!("exp", "Exponenciális függvény", "\\exp", 4),
    command!("left", "Automatikus bal zárójel", "\\left(", 6), command!("right", "Automatikus jobb zárójel", "\\right)", 7),
    command!("alpha", "Görög alfa", "\\alpha", 6), command!("beta", "Görög béta", "\\beta", 5),
    command!("gamma", "Görög gamma", "\\gamma", 6), command!("delta", "Görög delta", "\\delta", 6),
    command!("epsilon", "Görög epszilon", "\\epsilon", 8), command!("theta", "Görög théta", "\\theta", 6),
    command!("lambda", "Görög lambda", "\\lambda", 7), command!("mu", "Görög mű", "\\mu", 3),
    command!("pi", "Görög pi", "\\pi", 3), command!("rho", "Görög ró", "\\rho", 4),
    command!("sigma", "Görög szigma", "\\sigma", 6), command!("tau", "Görög tau", "\\tau", 4),
    command!("phi", "Görög fi", "\\phi", 4), command!("omega", "Görög ómega", "\\omega", 6),
    command!("mathbb", "Duplaszárú matematikai betű · amsfonts", "\\mathbb{}", 8),
    command!("mathcal", "Kalligrafikus matematikai betű", "\\mathcal{}", 9),
    command!("mathrm", "Álló matematikai szöveg", "\\mathrm{}", 8),
    command!("mathbf", "Félkövér matematikai betű", "\\mathbf{}", 8),
    command!("text", "Szöveg matematikán belül · amsmath", "\\text{}", 6),
    Command { name: "newcommand", detail: "Saját parancs · {\\név}{definíció}", insertion: "\\newcommand{\\name}{}", selection: 13, length: 4 },
];

pub(crate) enum Mode { None, Command { bare: bool }, Key { kind: CompletionKind, closing: bool, used: BTreeSet<String> } }
pub(crate) struct Context { pub set: CompletionSet, pub mode: Mode }
impl Context { pub fn needs_index(&self) -> bool { !matches!(self.mode, Mode::None) } }

pub(crate) fn byte_at(text: &str, caret: u32) -> Result<usize> {
    let mut byte = 0; let mut offset = 0;
    for ch in text.chars() {
        if offset == caret { break; }
        offset += ch.len_utf16() as u32; byte += ch.len_utf8();
        if offset > caret { return Err(CoreError::new("invalid_position", "A kurzor nem UTF-16 karakterhatáron áll.")); }
    }
    if offset != caret { return Err(CoreError::new("invalid_position", "A kurzor a dokumentumon kívül áll.")); }
    Ok(byte)
}
pub(crate) fn context(text: &str, caret: u32) -> Result<Context> {
    let byte = byte_at(text, caret)?;
    let mut set = CompletionSet { replace_start_utf16: caret, replace_length_utf16: 0, prefix: String::new(), items: vec![], index_status: None };
    let mut command_here = None; let mut key_command = None;
    // One prefix scan: the latest eligible reference/citation command owns its first argument.
    for token in Tokens::new(text) {
        match token {
            Token::Ignored(range) if range.start <= byte && byte <= range.end => return Ok(Context { set, mode: Mode::None }),
            Token::Command(command) => {
                if command.start > byte { break; }
                if command.start < byte && byte <= command.end && command.name.bytes().all(tex_scan::is_letter) { command_here = Some((command.start, command.end)); }
                if command.end <= byte {
                    let kind = match command.name {
                        "ref" | "eqref" | "pageref" | "autoref" | "nameref" | "vref" | "cref" | "Cref" => Some(CompletionKind::Reference),
                        "cite" | "citep" | "citet" | "Citep" | "Citet" | "autocite" | "parencite" | "textcite" | "footcite" | "smartcite" | "supercite" | "nocite" => Some(CompletionKind::Citation),
                        _ => None,
                    };
                    if let Some(kind) = kind { key_command = Some((kind, command.end)); }
                }
            }
            _ => {}
        }
    }
    if let Some((start, end)) = command_here {
        if byte - start - 1 > 64 { return Ok(Context { set, mode: Mode::None }); }
        set.replace_start_utf16 = text[..start].encode_utf16().count() as u32;
        set.replace_length_utf16 = (end - start) as u32;
        set.prefix = text[start + 1..byte].to_owned();
        let suffix = text[end..].trim_start();
        let bare = suffix.starts_with('*') || suffix.starts_with(['{', '[']);
        return Ok(Context { set, mode: Mode::Command { bare } });
    }
    if let Some((kind, mut p)) = key_command {
        p = tex_scan::skip_space(text, p);
        if text.as_bytes().get(p) == Some(&b'*') { p += 1; }
        while let Some(g) = tex_scan::group(text, p, b'[', b']').filter(|g| g.closed) { p = g.end; }
        if let Some(g) = tex_scan::group(text, p, b'{', b'}') {
            if !g.closed && g.end < text.len() { return Ok(Context { set, mode: Mode::None }); }
            if g.content.start <= byte && byte <= g.content.end {
                let left = text[g.content.start..byte].rfind(',').map_or(g.content.start, |n| g.content.start + n + 1);
                let right = text[byte..g.content.end].find(',').map_or(g.content.end, |n| byte + n);
                let segment = &text[left..right];
                let (start, end) = if segment.trim().is_empty() { (byte, byte) }
                    else { (left + segment.len() - segment.trim_start().len(), right - segment.len() + segment.trim_end().len()) };
                if start <= byte && byte <= end && (start == end || tex_scan::valid_key(&text[start..end])) {
                    let prefix = &text[start..byte];
                    let mut used = BTreeSet::new();
                    for (a, b) in [(g.content.start, left), (right, g.content.end)] {
                        for key in text[a..b].split(',').map(str::trim).filter(|key| tex_scan::valid_key(key)) { used.insert(key.to_owned()); }
                    }
                    set.prefix = prefix.into(); set.replace_start_utf16 = text[..start].encode_utf16().count() as u32;
                    set.replace_length_utf16 = text[start..end].encode_utf16().count() as u32;
                    // A nonfinal segment must not close an unfinished list before its later keys.
                    return Ok(Context { set, mode: Mode::Key { kind, closing: !g.closed && right == g.content.end, used } });
                }
            }
        }
    }
    Ok(Context { set, mode: Mode::None })
}
fn catalog(prefix: &str, bare: bool) -> Vec<CompletionItem> {
    let mut candidates: Vec<_> = COMMANDS.iter().filter(|command| command.name.starts_with(prefix)).collect();
    candidates.sort_by_key(|command| command.name != prefix);
    candidates.into_iter().take(20).map(|command| {
        let label = format!("\\{}", command.name);
        let bare = bare && command.insertion.contains('{');
        CompletionItem {
            kind: CompletionKind::Command,
            insert_text: if bare { label.clone() } else { command.insertion.into() },
            selection_start_utf16: if bare { label.len() as u32 } else { command.selection },
            selection_length_utf16: if bare { 0 } else { command.length }, label, detail: command.detail.into(),
        }
    }).collect()
}
pub fn complete(text: &str, caret_utf16: u32) -> Result<CompletionSet> {
    let mut context = context(text, caret_utf16)?;
    if let Mode::Command { bare } = context.mode { context.set.items = catalog(&context.set.prefix, bare); }
    Ok(context.set)
}
pub(crate) fn with_index(mut context: Context, index: &ProjectIndex) -> CompletionSet {
    context.set.index_status = Some(index.status.clone());
    let mut unique = BTreeMap::new();
    for symbol in index.symbols() {
        let eligible = match &context.mode {
            Mode::Command { .. } => symbol.kind == CompletionKind::Macro,
            Mode::Key { kind, used, .. } => symbol.kind == *kind && !used.contains(&symbol.name),
            Mode::None => false,
        };
        if eligible && symbol.name.starts_with(&context.set.prefix) { unique.entry(symbol.name.as_str()).or_insert(symbol); }
    }
    let mut symbols: Vec<_> = unique.into_values().collect();
    symbols.sort_by_key(|symbol| (symbol.name != context.set.prefix, symbol.name.as_str()));
    let mut items = Vec::new();
    if let Mode::Command { bare } = context.mode {
        if context.set.prefix.is_empty() { items = catalog("", bare); }
    }
    for symbol in symbols.into_iter().take(20) {
        let label = if symbol.kind == CompletionKind::Macro { format!("\\{}", symbol.name) } else { symbol.name.clone() };
        let mut insertion = label.clone(); let mut selection = insertion.encode_utf16().count() as u32;
        match &context.mode {
            Mode::Command { bare: false } if symbol.arguments.is_some_and(|n| n > 0) => { insertion.push_str(&"{}".repeat(symbol.arguments.unwrap() as usize)); selection += 1; }
            Mode::Key { closing: true, .. } => { insertion.push('}'); selection += 1; }
            _ => {}
        }
        items.push(CompletionItem { kind: symbol.kind, label, detail: format!("{} · {}:{}", symbol.description, symbol.path, symbol.line), insert_text: insertion, selection_start_utf16: selection, selection_length_utf16: 0 });
    }
    if let Mode::Command { bare } = context.mode {
        if !context.set.prefix.is_empty() { items.extend(catalog(&context.set.prefix, bare)); }
    }
    let mut seen = BTreeSet::new(); items.retain(|item| seen.insert(item.label.clone())); items.truncate(20);
    context.set.items = items; context.set
}
