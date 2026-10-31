# Editor intelligence and native assistant

Read `<repo>/docs/IDE_PRODUCT_DIRECTION.md`, the 2026-10-31 intelligence spec/plan and `<repo>/docs/intelligence-verification.md`. Command completion is shared Rust; its popup and the initial Codex assistant are implemented on Mac. Windows/Linux native versions remain future work, even though shared Rust type-checks for MSVC x64.

## Keyboard ownership

Mac Cmd+Z/Shift+Cmd+Z use the logical character, not a US physical key code. Measured Hungarian layout: CUA `super+z` delivered logical `y`/keyCode6; `super+y` delivered logical `z`/keyCode16. This is a tool/input-layout observation, not an application remapping instruction. Do not change users' layouts or add a global Cmd+Y undo alias. Native tests exercise logical `z` with both physical positions and reject extra Option/Control modifiers.

SourceTextView handles equivalents only when it owns focus. App menu commands call `WorkspaceStore.undoCommand/redoCommand`: an active native field/editor such as the assistant composer or Find owns its native UndoManager, while source/other workspace actions use Rust history. Agent save does not clear that history. Preserve IME and the private session/revision contract.

## Completion wire contract

`get_completions {document_id,expected_revision,caret_utf16}` borrows current authoritative text, including dirty edits. Reply fields: document_id/revision, replace_start_utf16/replace_length_utf16, prefix, items. Each item has label/detail/insert_text and selection_start_utf16/selection_length_utf16 relative to inserted text. Up to20 items; existing1MiB source limit. All positions are zero-based UTF-16 and scalar aligned.

The original command catalog is now extended by [project-completion.md](project-completion.md): own macros, ref labels and BibTeX keys from a bounded dirty-aware project index. It remains lexical rather than a complete language server. Replace the whole current command/key, preserving arguments/stars/list neighbors. Native acceptance also forces project freshness; an identical insertion consumes the key without editing/history. Stale Return/Tab dismisses and falls through. Keep marked text/selection/focus guards, release the popup's hosting hierarchy, and persist the completion preference. Color/theme changes do not edit source.

Accepting Enter closes the visible chooser and retains source focus; it must not require a subsequent Escape. Cover Return and keypad Enter with actual native key events, delayed refreshes and the ordinary second Enter. Mac `CompletionKeyboardTests` retain the popup and assert invisible/detached state in addition to text/history. Keep target-specific keyboard tests when porting; current Mac results are in `<repo>/docs/completion-enter-verification.md`.

## Codex adapter

Mac source: `apps/macos/Sources/LaTeXIDEKit/Assistant/`. The user explicitly selected native panel + installed Codex CLI. It uses bounded nonblocking JSONL stdio `codex app-server --listen stdio://`; no UI web runtime. Resolve existing executable/allow native selection; start only on explicit Connect. Connect sends initialize/initialized only. Send creates an ephemeral conversation, reads effective config, disables inherited MCP servers/apps/shell/web features, adds only the required IDE MCP bridge, and verifies a read-only policy before a turn. Thread sandbox enum is `read-only`; turn policy type is `readOnly`.

Private descriptor validation binds protocol/owner/mode/regular file/canonical project before launch. Do not emit its token. Reads/edits/build/save of project documents go through the existing MCP session. Keep explicit-save and shared-undo boundaries; never bypass with raw file patches. No global Codex config/auth changes. Model comes from the user's existing provider settings; a local CLI process is not necessarily offline inference.

Actual model validation exposed that `auto` is not an unconditional MCP grant under `never`. Use the exact known IDE tool allowlist (`workspace_info`, `list_project_files`, `read_document`, `apply_edits`, `save_document`, `get_diagnostics`, `request_build`, `get_operation`, `cancel_build`) and per-tool `approval_mode=approve`; unknown-tool default stays `prompt`. Never grant arbitrary inherited tools or change global settings to fix this. Keep `enabled_tools` and the grants aligned with the packaged bridge. The scoped approval regression and actual read/edit→keyboard undo workflow are recorded separately from fixtures/handshake.

AssistantSession owns connection, bounded messages, draft text and attachments, outside disposable PDF/Assistant views. Panel/hide roundtrips retain draft. Project generation disconnects and drops late events; explicit new conversation/project reset clears old context. Window disappearance/session deinit stops the child. Handle cancellation before a turn id, EOF/partial frames/timeouts, malformed policy responses and unexpected server requests without hanging. Never fabricate model output or connection state.

Tests use actual controlled subprocesses plus an opt-in installed-CLI initialization check. Read the verification document for whether a model/MCP bootstrap turn actually ran; fixtures and initialization alone do not establish that. The inspected installed CLI is0.166.0-alpha.5.1, an experimental integration boundary. Generated exact schemas live under `.local/codex-app-server-schema` and must be regenerated/reviewed when qualifying another CLI version.

Future Windows/Linux developers should port these state/framing/focus/UTF-16 contracts and tests with their actual native controls/process APIs. Darwin F_SETNOSIGPIPE and AppKit popup ownership are Mac procedures, not portable implementations. Keep target runtime/keyboard/IME/provider qualification explicit in Markdown.
