# Developer IDE interface and LaTeX syntax themes

The user explicitly requested a much closer JetBrains + VS Code experience: polished, premium, developer-friendly. They then specified several prepared palettes, system-following or fixed appearance, and distinct syntax colors for LaTeX constructs while ordinary prose remains readable. This is an authorized refinement of the existing native IDE, retaining its Rust authority, native controls, compiler and MCP contracts.

## Chosen direction and alternatives

Use the user's requested hybrid: JetBrains-style compact chrome/tool windows and VS Code-style activity rail, project tree and file tabs. A conventional macOS sidebar (current UI) does not provide the requested density. A web/Monaco replacement would change the accepted native architecture and memory profile; keep AppKit/SwiftUI and WinUI instead. This decision follows the user's named references; no additional architecture approval is needed for this reversible UI refinement.

Primary references: [JetBrains UI](https://www.jetbrains.com/help/idea/guided-tour-around-the-user-interface.html), [VS Code UI](https://code.visualstudio.com/docs/editing/getting-started/userinterface).

## Layout

- Compact native window chrome: project selector left, command/file search center, main-file/run controls right; preserve traffic lights, dragging and resize behavior.
- Narrow activity rail for working Project, File search and local Agent panels, with appearance at the bottom. No fake Git, terminal, debugger or AI chat controls.
- Resizable project tool window, hierarchical directories and selected file state; compact file rows, useful type colors, context actions.
- Scrollable document tabs, dirty markers and safe close/hide semantics. Closing a visual tab must not destroy Rust undo or unsaved content.
- Central code editor with native line-number gutter, current-line indication, monospaced source, breadcrumb and native Find. Source–PDF divider resizable, preview hide/show and useful PDF zoom/page controls.
- Bottom tool window with real diagnostics and compiler output, hide/show, clear prior-result labels. Persistent status bar with actual caret, UTF-8, actual LF/CRLF and build/agent state.
- Quick Open (Cmd+P), command palette (Cmd+Shift+P), Find (Cmd+F), build/save/history shortcuts, sidebar/PDF/output toggles and appearance popover.
- Empty state is a compact editor start surface with actionable keyboard hints, not a marketing page.

## Palettes and appearance

Four bundled palette families: Graphite, Ocean, Dune, High Contrast. Each has light and dark values. Appearance mode independently selects System / Light / Dark. Persist both choices locally. Palette data is shared JSON so Windows and later Linux can use the same semantic values. No network theme download or new external runtime.

Semantic colors: chrome, sidebar, editor, panel, borders, selected/hover rows, active tab, current line, selection, foreground/secondary, accent/success/warning/error. Syntax: command, environment, delimiter, comment, math, parameter, literal argument, number, operator, option key. Ordinary prose keeps the foreground color. Check text contrast for all palette variants.

## Syntax contract

One bounded Rust lexical highlighter, shared by native frontends. Additive native protocol request returns compact UTF-16 span triples `[start, length, kind]` for a revision-protected requested window, max 32768 UTF-16 units. No source serialization per keystroke and no second document model. Preserve escaped percent/dollar, multiline math, environment names, literal package/path arguments, comments, verbatim and Unicode boundaries. This is lexical coloring, not a macro-expanding TeX parser.

Native coloring changes attributes only, never document text/revision/history. Skip marked text and color after commit; debounce normal edits and color visible content on scroll. Native controls keep core undo/redo. A theme change must not create an edit. Normal source remains plain text on clipboard/save.

## Platform and verification boundaries

Mac: implement, build and inspect the real native interface. Preserve the user's existing sample edits. The current CUA screenshot is visibly distorted; use AX behavior plus a high-resolution native render artifact if needed and distinguish that artifact from live GUI evidence.

Windows: carry shared palette assets, native layout/theme/highlight mapping into the WinUI source. No runner exists; XML/source/pure-function checks are not Windows linking/runtime proof. Linux: document reusable layout/color/syntax contracts; no toolkit selection.

Regression focus: Unicode token ranges; comments/escaped syntax; math/verbatim context; themes changing attributes only; IME/undo preserved; switching/closing tabs retains dirty state; saved metadata updates without revision changes; no inaccessible/fake actions. Readability and actual rendered inspection are required beyond build success.

## Builder check resolutions

One fresh read-only builder check identified practical implementation contracts; all are settled from the existing architecture and user request, without new product questions.

- Span starts are document-absolute UTF-16, sorted/non-overlapping. Return complete scalar-aligned tokens intersecting the requested window; adjacent same-kind spans merge. Syntax precedence is verbatim > escaped command/comment > command/delimiter > literal/math content. Requests scan preceding text for lexical state, bounded by the existing1MiB document limit; use a borrowed workspace document, not a whole-document snapshot copy per color request. Debounce edits and restrict returned windows; record large-source timing rather than claim performance without measurement.
- Native responses are applied only to the same document/revision/window. Theme colors resolve at application time. Stale revision responses are quietly discarded and rescheduled by the normal refresh path; no modal dialog for coloring races.
- Expose actual BOM/EOL in the existing document read response. Refresh metadata independently of text revisions for every retained document, including agent saves.
- Visual tab hiding preserves Rust state/history and still occupies the existing ten-document budget. Name the action "Fül elrejtése"; provide a retained/hidden-tab indicator and Quick Open recovery. Do not silently evict dirty text or imply that hiding frees the core budget. Full core close/eviction remains a separately designed feature.
- Stage the local SwiftPM resource bundle inside the `.app` and shared JSON beside the Windows executable. No checkout-relative product fallback. Palette System mode releases native overrides and follows live system changes.
- Preserve selection/scroll/typing attributes on recolor and tab changes; no recolor while marked text exists. Windows pure text extraction/ranges and plain-text paste need actual target validation even if source uses the shared offsets.
- Agent panel says local MCP is available, not that a client is connected. Filename filtering is labeled as such; editor Find is separate. New icons only expose working actions.
