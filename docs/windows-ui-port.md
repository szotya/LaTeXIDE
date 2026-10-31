# Windows developer UI port — source and qualification boundary

2026-10-30. This is implemented **Windows source**, with portable/structural checks run on Mac. No Windows runner exists. It is not a native Windows build, launch or rendered acceptance result. See [the foundation qualification procedure](windows-validation.md) and [the accepted UI design](superpowers/specs/2026-10-30-developer-ui-design.md).

## Implemented source

- `apps/windows/MainWindow.xaml`: compact command row, activity rail, hierarchical native project tree, filename filter, native tabs and breadcrumb, gutter/current-line layer, source Find, resizable source/PDF/sidebar/output regions, real diagnostics/compiler output, caret/encoding/EOL/compiler/MCP status and the appearance flyout. The OS still owns title-bar dragging, resize and caption buttons; title colors follow the palette.
- `apps/windows/MainWindow.Presentation.cpp`: native presentation state, palette application, project-tree selection, keyboard filename filtering, panels and visual-tab hiding. The action is explicitly named **Fül elrejtése**. Hidden documents still use Rust's ten-retained-document budget and remain available through the tree/Ctrl+P; no close, eviction or history reset command is sent. Dirty metadata refreshes independently of source revisions, including agent saves.
- `apps/windows/ThemeCatalog.{h,cpp}`: the shared schema-1 JSON catalog, four palette families with light/dark values, and separate System/Light/Dark preferences. Preferences use `%LOCALAPPDATA%/LaTeXIDE/appearance.json`. System selects native `ElementTheme::Default`, with `ActualThemeChanged` updating semantic brushes. Existing brush objects are changed so their native consumers retain the references.
- `apps/windows/MainWindow.Editor.cpp`: 120 ms syntax debounce, viewport-bounded scalar-safe requests, attribute-only foreground changes and native gutter/current-line geometry. Requests use absolute UTF-16 spans and the shared kind ordering; stale revisions retry quietly. Composition defers coloring, native undo is disabled, Ctrl+Z/Y uses Rust, clipboard copying is plain text, the Paste event inserts text only and rich drops/formatting controls are suppressed. AltGr/Ctrl+Alt input is not treated as an IDE shortcut.
- `apps/windows/MainWindow.xaml.cpp` and `NativeCore.{h,cpp}`: source/EOL/BOM snapshots, common history/save/compiler/MCP connections, composition rollback and project-generation checks for asynchronous PDF work. A rejected native composition begin remains marked locally until its end event, then reloads acknowledged Rust text. A rejected end releases composition and reloads acknowledged text.
- `apps/windows/LaTeXIDE.vcxproj` and `scripts/build_windows.ps1`: stage the canonical `shared/ide-theme/Sources/IDEThemeAssets/Resources/editor-themes.json` **beside the executable**. Runtime reads that file only; there is no checkout-relative asset fallback. The per-user settings location uses the Windows known-folder API and adds only the system `shell32.lib` link dependency.

There is no instantiated embedded browser, Node or .NET runtime, extra UI package, fake Git/terminal panel, or claimed connected-agent count. “MCP elérhető” means the local session is available.

## Exact local checks and results

Run from the repository root on Mac:

```sh
python3 apps/windows/tests/check_ui_source.py
clang++ -std=c++20 -Wall -Wextra -Werror apps/windows/tests/editor_helpers.cpp -o /private/tmp/latexide-windows-editor-helpers
/private/tmp/latexide-windows-editor-helpers
clang++ -std=c++20 -Wall -Wextra -Werror tests/windows_text_adapter.cpp -o /private/tmp/latexide-windows-text-adapter
/private/tmp/latexide-windows-text-adapter
```

Actual results, exit 0:

- Structural checker: **5 XML files, 42 event handlers**, unique XAML names, referenced semantic brushes and native source/asset references pass. This is XML parsing and source consistency, not WinUI XAML schema generation or C++ compilation.
- New portable helper harness: scalar-safe bounded syntax windows, logical-line starts and CR/CRLF paragraph normalization pass, including exhaustive first/last/cap combinations over small Unicode/empty samples. The first run failed with eight missing helper identifiers; the implementation made it pass.
- Existing portable adapter harness: UTF-16 delta, surrogate rejection and paragraph normalization pass.
- `pwsh` is not installed in this Mac environment. PowerShell parser execution, package restore, MSBuild, native C++/WinRT projections and Windows launch remain **unrun**.

The binaries above live only in the local temporary directory. No user project/sample source was changed by this port.

## Decisions and remaining Windows work

Ruling: retain the native system title bar and color it, with a compact 44-pixel command row below — this preserves OS drag/caption behavior without claiming untested custom hit-test regions. Cost: title-bar density is not yet visually qualified against the requested reference UI.

Ruling: Ctrl+P focuses a clearly labeled filename filter in the project tool window; Enter opens the first matching file and Down moves to the native tree. Ctrl+Shift+P opens a menu of implemented actions. Cost: this Windows source does not yet provide a fuzzy-index popup or searchable command list.

Ruling: fail closed if the full native text round trip or the `GetRange(0, sourceUTF16Length)` text differs from the acknowledged Rust source. The editor remains read-only in that case. Every returned syntax range is also compared with its expected text before coloring. Cost: a real RichEdit convention mismatch can block Windows editing until its adapter is corrected; it cannot silently color or edit the wrong model range.

Required target checks, all **unrun**:

1. Restore/build/link with the pinned Windows tools, including XAML generation and the `Microsoft.UI.Text` projection types, `GetDefaultCharacterFormat`, `UndoLimit`, `DisabledFormattingAccelerators`, TabView/TreeView event types and title-bar colors. These API names were reconciled with Microsoft documentation; no linked/native compilation is claimed.
2. Prove RichEdit's implicit final paragraph, `TextGetOptions::None`, TOM UTF-16 positions, `GetRangeFromPoint`/`GetRect` client geometry and scroll-view discovery on Windows. The Mac helper harness cannot prove these. Check empty text, final newline, LF/CRLF files, emoji, combining marks and very long logical lines.
3. Verify all eight palette variants; System must follow live OS changes after fixed Light/Dark, and both choices must survive restart. Check selection, native-control hover/focus, caption buttons, disabled states, high contrast, Narrator, DPI scaling, long paths and narrow windows. Native layout has not been rendered or visually accepted here.
4. Recolor and theme switch with nonempty/backward selections, scroll offsets and insertion formatting; prove source/revision/shared undo unchanged. Exercise IME commit once, rejected begin/end, an agent revision race and subsequent ordinary typing.
5. Test plain-text paste from HTML/RTF, keyboard and context cut/copy/paste, international AltGr, native formatting shortcuts, undo/redo and Tab insertion. Async paste must not apply to a different document/revision/selection after awaiting the clipboard.
6. Hide/reopen dirty tabs, all tabs hidden, metadata-only agent saves and the ten-retained-document limit. No hidden content or undo may be evicted.
7. Exercise project open → native Unicode edit → direct MCP edit/save → GUI undo/save → compile → fresh PDF. Check cancellation/timeout/failure/stale builds keep the prior PDF correctly labeled. PDF rendering is one-page Windows.Data.Pdf with zoom/navigation; text selection/search parity with PDFKit is not implemented.

The parent implementation integrated this port into the central handoff/ledger and the shared UI/Windows/Linux skill references. The final source-review fix also assigns each new project's own main-file choice before loading it (`ProjectFiles.h`); the Mac-executed helper covers a project containing only `thesis.tex`, preference for `main.tex`, and a project without TeX files. Native Windows execution remains unrun.

## Primary API references consulted

- [RichEditBox.Document and its Microsoft.UI.Text document](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.richeditbox.document)
- [RichEditTextDocument methods and UndoLimit](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.text.richedittextdocument)
- [Native formatting-accelerator suppression](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.richeditbox.disabledformattingaccelerators)
- [RichEdit selection brush](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.richeditbox.selectionhighlightcolor)
- [TreeView node invocation](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/tree-view)
- [System-owned title-bar color customization](https://learn.microsoft.com/en-us/windows/apps/develop/title-bar)

Documentation consultation is source guidance, not a Windows runtime result.
