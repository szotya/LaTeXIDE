# Shared native developer UI contract

Read `<repo>/docs/DEVELOPER_HANDOFF.md`, `<repo>/docs/ui-verification.md` and the accepted developer-UI design/plan before changing presentation. Mac is an implemented native AppKit/SwiftUI/PDFKit workspace; WinUI source is implemented but unbuilt on target; Linux has no chosen frontend.

The2026-10-31 increment adds shared command completion, Mac native popup/Codex inspector, persistent pane dimensions and focused-field undo. Read [intelligence.md](intelligence.md) and `<repo>/docs/IDE_PRODUCT_DIRECTION.md` for those contracts; do not infer Windows/Linux native parity from the shared API.

## Palette assets and appearance

Canonical resource: `<repo>/shared/ide-theme/Sources/IDEThemeAssets/Resources/editor-themes.json`, schema1. Four families (graphite/ocean/dune/contrast), each light/dark. `syntax_kind_roles` defines wire kind ordering; semantic brushes have no platform-specific alternate values. System/Light/Dark mode is independent of palette. Persist both, release native appearance overrides for System, and react to live system changes. No theme download.

Mac uses the local SwiftPM IDEThemeAssets product. Stage its `.bundle` under app Resources; missing/invalid assets fail visibly rather than read the developer checkout. Windows stages the same JSON beside its executable. Preferences use UserDefaults on Mac and the known LocalAppData folder on Windows. Keep contrast checks for normal/secondary/selection and syntax text in all eight variants.

## Syntax and native editing

`get_syntax {document_id,expected_revision,offset_utf16,length_utf16}` returns absolute sorted nonoverlapping `[start,length,kind]` spans. Windows are at most32768 UTF-16 units; complete intersecting tokens may extend past the window, scalar aligned. Prefix scanning recovers math/verbatim context under the1MiB document bound; borrow core text, do not request a snapshot per key. Stale revisions are quietly discarded. Kind0 is ordinary prose; kinds1–10 follow the JSON map.

The lexer is lexical, not macro-expanding. Native highlighters apply foreground attributes only, debounced on ordinary input/scroll, immediate on initial/theme presentation, and deferred for marked text. Preserve text, core revision/history, selection, typing font/paragraph defaults and viewport. Save plain source and use plain clipboard insertion. RichEdit must prove exact full/range text round trips before trusting TOM positions; a mismatch keeps editing read-only. Mac native Find is separate from the project filename filter.

## Workspace presentation

Selection and scroll state belong to persistent per-project presentation, not a disposable editor coordinator. Restore both range endpoints safely; scroll to the caret for explicit navigation. Mac `EditorViewportTests` exercises real view update and last-tab destruction/recreation. Project switches reset presentation and choose a main file from the new project's files.

Visual hiding retains Rust text, dirty state and undo, including the final tab; it does not free the ten-retained-document budget. Refresh dirty metadata independently of revision, including MCP saves. Keep current/old/failed PDF generations distinct. MCP available is not a connected-client count. Only expose real actions and actual status, not fake Git/terminal/agent panels.

## Verification and packaging

Run Rust, native attribute/history/IME/tab/viewport/theme tests and actual platform builds. The Mac render test is opt-in (`LATEXIDE_RENDER_PREVIEWS=1`); it uses actual native content and vector-draws the real compiled PDFPage because off-screen caching excludes GPU tiles. Label images as controlled renders, not live screenshots.

Mac packaging hashes Cargo's external static archive into a generated Swift source stamp to force relink. Stage/sign/verify a complete bundle, then replace the old bundle by rename with a rollback copy; never truncate a running executable. Canonical Run refuses an existing process to preserve unsaved edits.

Windows source/portable checks do not compile WinUI. Follow `<repo>/docs/windows-ui-port.md` for target checks and partial Quick Open/command/PDF parity. A future Linux adapter must consume these contracts and run its own native/IME/accessibility tests.


Project macro/ref/BibTeX completion: [project-completion.md](project-completion.md). The shared index is implemented; the new native popup/acceptance remains Mac-only. Port `refresh_project:true` acceptance validation, typed candidates/partial status, whitespace/list/scalar handling and no-op acceptance without history. Shared MSVC type-check does not establish native Windows/Linux behavior.
