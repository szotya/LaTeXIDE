# Project-aware LaTeX completion

The user explicitly requested continuing the existing IDE with own macros, reference labels and BibTeX citation keys, while maintaining Markdown and the developer skill. This is an authorized extension of the shared Rust query/native popup, not a new UI architecture or approval flow.

## Approach and scope

Extend the existing Rust completion with a bounded, cached lexical project index, and use the existing native candidate popup. A full TeX language server would add process/dependency/configuration and a much larger semantic contract; reading every file on each keystroke would harm native responsiveness. The chosen incremental Rust approach is the implementer's routine choice under the accepted architecture. It does not execute TeX, expand arbitrary macros or infer a compiler's include order.

Index project-local `.tex`, `.ltx`, `.sty`, `.cls`, `.bib` recursively. Ignore hidden/artifact directories and symlinks; keep traversal, reads and symbol retention bounded. Open authoritative documents override disk snapshots immediately, including dirty agent edits and undo. Indexing does not open editor tabs, consume the ten-document budget, save, change revisions or add history. Cached disk discovery/metadata refresh is at most once per second during typing; accepting a candidate forces a bounded fresh disk check. New/removed external files appear after refresh; already-open documents remain authoritative according to the existing external-change contract.

Initial budgets:128 indexed files,8MiB aggregate source,1MiB/file,8192 retained symbols,10000 visited directory entries. Candidate limit stays20. Reply reports partial/skipped index state; the popup makes partial coverage visible. The first query may scan within those budgets; record actual cold/warm timings, not a release-performance claim.

## What users get

- After `\my`, offer project-defined control words with source path/line and a conservative argument snippet. Recognize new/renew/providecommand, DeclareRobustCommand, DeclareMathOperator, def/gdef/edef/xdef and the New/Renew/Provide/DeclareDocumentCommand family. Plain `#1#2` and common `m/o/O/s` argument specs get mandatory braces; unknown/delimited signatures insert only the command. Existing arguments/stars survive. Project definitions can override builtin suggestions; duplicates are deterministic: active document, other open documents sorted, remaining files sorted; last definition within a file.
- Inside ref/eqref/pageref/autoref/nameref/vref/cref/Cref arguments, offer static `\label{key}` values from project source. For key lists replace just the current comma-delimited key; preserve whitespace, other keys, optional arguments and braces. Exclude keys already used in the other list segments.
- Inside cite/citep/citet and common biblatex citation forms (autocite/parencite/textcite/footcite/smartcite/supercite/nocite), offer `.bib` entry keys with type/source detail. Support brace/parenthesis entries, nested fields and quoted text; skip string/preamble/comment pseudoentries. No bibliography download or compiler run is needed. If an incomplete argument has no closing brace, acceptance adds one only for its final segment. Completing an earlier segment leaves the unfinished list intact; whitespace-only segments are valid empty keys.
- Ignore commented/verbatim examples both while indexing and while typing. Avoid dynamic keys containing TeX commands/parameters. Matching stays deterministic and case-sensitive. This is project-aware lexical coverage, not full TeX semantics, package catalogs or citation resolution.

## Wire and native behavior

Keep existing document/revision/UTF-16 replacement/selection fields. Add item `kind` and optional set `index_status { indexed_files, truncated, skipped_files }`. Add optional request `refresh_project` default false. Old native consumers may ignore new fields. Swift decodes optional metadata and shows a compact type badge, source detail and partial-index hint in the existing themed popup.

Acceptance re-queries with `refresh_project=true` and verifies the chosen insertion/range still exists, catching a definition changed in another file even when the active document's revision stayed fixed. Stale Enter/Tab dismisses and falls through. One insertion remains one Rust transaction; cursor/IME/focus/undo safeguards remain.

An already identical key is accepted without editing/history or ordinary Tab fallthrough. Context groups are limited to16KiB. Literal environment names use short lookahead, macro names/counts use lexical reads and document-signature parsing has an aggregate work budget; repeated malformed groups must not multiply into multi-second synchronous work. These corrections follow the recorded review's reproductions.

## Evidence and boundaries

Real filesystem/core tests cover dirty cross-file edits/undo, labels/citations with Unicode and mid-key replacement, malformed/comment/verbatim/BibTeX cases, duplicates, limits and project confinement. IPC exercises the actual dirty-project query and forced refresh. Native tests exercise actual key insertion/caret/undo and cross-file stale fallback; rendered/live checks use a separate own fixture. Run the full affected Rust/native suites, native package/signature, MSVC x64 type-check and one fresh integrated review. Windows/Linux native execution remains unrun; shared behavior/port contracts go into the skill.

Primary syntax references: [LaTeX author guide](https://www.latex-project.org/help/documentation/usrguide.pdf), [historic definitions guide](https://www.latex-project.org/help/documentation/usrguide-historic.pdf), [BibTeXing](https://tug.ctan.org/biblio/bibtex/base/btxdoc.pdf), [natbib manual](https://tug.ctan.org/macros/latex/contrib/natbib/natbib.pdf). The parser deliberately implements the documented subset above, not those specifications in full.
