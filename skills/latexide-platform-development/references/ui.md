# Shared native developer UI contract

Read `<repo>/docs/DEVELOPER_HANDOFF.md`, `<repo>/docs/ui-verification.md` and the accepted developer-UI design/plan before changing presentation. Mac is an implemented native AppKit/SwiftUI/PDFKit workspace; WinUI source is implemented but unbuilt on target; Linux has no chosen frontend.

The2026-10-31 increment adds shared command completion, Mac native popup/Codex inspector, persistent pane dimensions and focused-field undo. Read [intelligence.md](intelligence.md) and `<repo>/docs/IDE_PRODUCT_DIRECTION.md` for those contracts; do not infer Windows/Linux native parity from the shared API.

## Palette assets and appearance

Canonical resource: `<repo>/shared/ide-theme/Sources/IDEThemeAssets/Resources/editor-themes.json`, schema1. Four families (graphite/ocean/dune/contrast), each light/dark. `syntax_kind_roles` defines wire kind ordering; semantic brushes have no platform-specific alternate values. System/Light/Dark mode is independent of palette. Persist both, release native appearance overrides for System, and react to live system changes. No theme download.

Mac uses the local SwiftPM IDEThemeAssets product. Stage its `.bundle` under app Resources; missing/invalid assets fail visibly rather than read the developer checkout. Windows stages the same JSON beside its executable. Preferences use UserDefaults on Mac and the known LocalAppData folder on Windows. Keep contrast checks for normal/secondary/selection and syntax text in all eight variants.

## Syntax and native editing

`get_syntax {document_id,expected_revision,offset_utf16,length_utf16}` returns absolute sorted nonoverlapping `[start,length,kind]` spans. Windows are at most32768 UTF-16 units; complete intersecting tokens may extend past the window, scalar aligned. Prefix scanning recovers math/verbatim context under the1MiB document bound; borrow core text, do not request a snapshot per key. Stale revisions are quietly discarded. Kind0 is ordinary prose; kinds1–10 follow the JSON map.

The lexer is lexical, not macro-expanding. Native highlighters apply foreground attributes only, debounced on ordinary input/scroll, immediate on initial/theme presentation, and deferred for marked text. Preserve text, core revision/history, selection, typing font/paragraph defaults and viewport. Save plain source and use plain clipboard insertion. RichEdit must prove exact full/range text round trips before trusting TOM positions; a mismatch keeps editing read-only. Mac native Find is separate from the project filename filter.

Cache UTF-16 line offsets across accepted native edits; same-length edits without line breaks must not rescan a whole document. Rebuild on stale state, rejection, composition or external history refresh. During marked text the ruler and status caret share display-only line geometry from the transient view, while core text/history stay unchanged. After accepted CRLF input or composition, align the view and mapped UTF-16 selection to normalized store.text; update cached offsets from the normalized replacement. Detect representation changes from the replacement alone so ordinary keystrokes do not compare a whole document. Test subsequent emoji edits and one shared undo, not only a standalone index. Query already-laid-out visible glyph ranges after visible-layout requests rather than total glyph counts per redraw. Avoid forcing recursive layout when invalidating selection/color display. Keep real hosted viewport restoration tests: unconditional noncontiguous layout displaced scrolling; enable it only for a completely validated single ASCII row. The representable may update before attaching to a window or receiving its size; defer guarded restoration through hosting layout and cancel on actual native input/navigation. Native ABI timing, packaged editor timing and hardware presentation are separate; record file layout, sample count, profile, agreed target and every failed attempt. PR68 met the retained 16.7 ms p95 in all four controlled 40-sample cases, including 1 MiB single line; this is not broad release qualification. See `<repo>/docs/recovery-latency-review.md`. These Mac changes do not prove Windows/Linux layout or IME behavior.

Mac SourceTypesetter keeps supported fixed-pitch ASCII fragments at most2048 glyphs on one visual row, with complete native text and native fallback for unsupported text/attributes/IME. A complete uniform system-monospaced row may reuse geometry only for bounded same-length printable-ASCII edits with unchanged validated font/style/allowed attributes; invalidate the certificate otherwise. Compare real glyph IDs/properties/character indices and geometry with native controls, including shared storage. Preserve logical hard-line navigation, full selection/clipboard, Unicode scalar offsets and native Find. Syntax foreground colors are temporary layout-manager drawing attributes; they must not regenerate source glyphs.

Mac SourceViewportSurface is owned by its SourceTextView. Draw synchronously into the actual visible viewport using the window profile/BGRA layout, align damage to full backing pixels and preserve prior images through copy-on-write. Idle updates reuse pixels; explicit full invalidation repaints. Native insert/marked/unmark must publish actual changed pixels before returning, verified against fresh native draws and the real layer composite. Repeated unchanged-caret notifications must not add a whole-viewport redraw; changed selection and composition retain invalidation. Keep image-publication assertions in packaged timing.

WorkspaceStore uses field-specific Observation. UI tab projections exclude revision churn but update dirty/origin/title independently of revision, including same-revision saves. Core revision-bearing metadata remains authoritative. Verify actual NSHostingView external NFC/NFD refresh, subsequent UTF-16 edit/undo, and hide/destroy/recreate selection/scroll behavior rather than only manually calling synchronize.

## Workspace presentation

Selection and scroll state belong to persistent per-project presentation, not a disposable editor coordinator. Restore both range endpoints safely; scroll to the caret for explicit navigation. Mac `EditorViewportTests` exercises real view update and last-tab destruction/recreation. Project switches reset presentation and choose a main file from the new project's files.

Visual hiding retains Rust text, dirty state and undo, including the final tab; it does not free the ten-retained-document budget. Refresh dirty metadata independently of revision, including MCP saves. Keep current/old/failed PDF generations distinct. MCP available is not a connected-client count. Only expose real actions and actual status, not fake Git/terminal/agent panels.

## Verification and packaging

Run Rust, native attribute/history/IME/tab/viewport/theme tests and actual platform builds. The Mac render test is opt-in (`LATEXIDE_RENDER_PREVIEWS=1`); it uses actual native content and vector-draws the real compiled PDFPage because off-screen caching excludes GPU tiles. Label images as controlled renders, not live screenshots.

Mac packaging hashes Cargo's external static archive into a generated Swift source stamp to force relink. Stage/sign/verify a complete bundle, then replace the old bundle by rename with a rollback copy; never truncate a running executable. Canonical Run refuses an existing process to preserve unsaved edits.

Windows source/portable checks do not compile WinUI. Follow `<repo>/docs/windows-ui-port.md` for target checks and partial Quick Open/command/PDF parity. A future Linux adapter must consume these contracts and run its own native/IME/accessibility tests.


Project macro/ref/BibTeX completion: [project-completion.md](project-completion.md). The shared index is implemented; the new native popup/acceptance remains Mac-only. Port `refresh_project:true` acceptance validation, typed candidates/partial status, whitespace/list/scalar handling and no-op acceptance without history. Shared MSVC type-check does not establish native Windows/Linux behavior.
