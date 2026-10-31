# Native IDE intelligence and interaction quality

User requirements: reliably undo agent edits with Mac Cmd+Z; a deliberately designed, customizable premium IDE for researchers and developers, inspired by JetBrains and VS Code; native assistant connected to an existing local agent. The user selected **Codex CLI** as the first adapter and requested live LaTeX command-completion candidates. This extends the accepted native/shared-Rust architecture. Windows/Linux portability remains maintained in Markdown/skill; target runtime depends on external machines.

## Product structure and quality contract

One coherent workspace: project/navigation left, authoritative source center, right inspector for PDF or Assistant, diagnostics/output below. The right inspector switches between PDF and Assistant rather than permanently squeezing three narrow content columns into the minimum window. Tool windows share the same spacing, semantic palette, typography, focus conventions and resize behavior. Persist appearance, font, completion preference and useful pane dimensions. No ornamental dead controls or simulated assistant replies.

Researchers need structure/citations/equations/source–PDF navigation and reproducible builds; developers need keyboard navigation/completion/diagnostics/project operations and explainable agent changes. Existing implementation covers local projects, native source/PDF, builds, syntax, tabs and history. This increment delivers command completion and real assistant transport. Semantic project indexing, citation completion, SyncTeX navigation, Git integration, workspace profiles and extension SDK remain named future work, not checked features. Product readiness is judged by complete workflows, not a collection of panels.

## Keyboard finding and acceptance

The initial failed CUA `super+z` was measured in a separate native probe: current input source is Hungarian, keyCode6 generated `charactersIgnoringModifiers=y`, while CUA `super+y` generated keyCode16/`z`. The IDE correctly follows characters rather than a hardcoded US key position. Actual Cmd+Z/Shift+Cmd+Z already restored/reapplied a real saved MCP transaction, with exact core revisions and clean/dirty state. Preserve this behavior; do not add a second physical-key undo alias. Add a focused characterization for both layouts and keep local text-entry undo distinct from document history.

## Command completion

Shared Rust pure completion query against borrowed authoritative source at a revision and zero-based UTF-16 caret. Return a replacement range, prefix and bounded candidates with command name, short Hungarian description, insertion text and first-argument selection. Max20 candidates, document1MiB, bounded backward prefix; no snapshot serialization per keystroke. Reject non-scalar carets; no automatic candidates in comments/verbatim, escaped backslash, selection or IME. Matching is deterministic prefix ranking with exact command first; a bundled authored catalog covers common document/structure/math/font/reference/package commands and environments. No downloaded database or external dependency.

Native AppKit completion popup anchors below the caret, follows the active palette and preserves source focus. Show command/signature/detail; Up/Down selects, Tab/Return accepts only a visible current proposal, Escape dismisses. Acceptance is exactly one core edit/undo transaction and selects the first argument; stale document/revision/caret must not apply. Re-query after ordinary typing, dismiss on tab/project/focus/composition change, and allow completion preference off. Standard Tab/Return behavior remains when no proposal exists. Argument/label/citation semantic completion is future work.

## Native Codex assistant

The user explicitly chose an existing local Codex CLI agent. Use the official stdio app-server protocol as a separate process; no browser/Node UI and no mandatory model runtime inside the IDE. Resolve an executable from user configuration/PATH/common official app locations; let the user choose a different executable with a native file picker. Start only on explicit Connect/Send, never on app launch. No auth token or global Codex config changes. Default model comes from the user's existing Codex configuration, not a hardcoded model recommendation.

Session configuration binds project root, the existing private IDE session descriptor and the packaged MCP bridge. Codex's raw filesystem is read-only; developer instructions require all document reads/edits/saves through the IDE MCP tools, so dirty content and shared undo remain authoritative. Configure the project's MCP server per session. No direct apply_patch/shell writes or approval escalation. A source-changing action is direct and undoable per the accepted contract; Save remains explicit. MCP tools already enforce project/revision constraints.

Native panel: connection state/error/retry, transcript with streaming assistant text, visible tool activity, prompt field, Send/Stop, context chips for project/active file/revision and optional explicitly added selection/diagnostics. New conversation resets the assistant conversation, not document undo. Switching project disconnects/cancels old process and discards late events. No fabricated connected status or output. Explain locally that Codex runs as a local agent process and uses its configured model service; this is not a claim of offline inference.

Protocol implementation must bound frames/transcript, handle split UTF-8/newlines, process EOF, request timeouts, stale events and cancellation-before-turn-id. Unexpected server approval/elicitation requests receive explicit refusal/error rather than hanging. Tests drive a real controlled subprocess speaking protocol fixtures; real CLI handshake can be checked without any model request. A live model turn is a separate result, not inferred from fixtures.

Primary sources: [Apple key events](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingKeyEvents/HandlingKeyEvents.html), [OpenAI app-server](https://developers.openai.com/codex/app-server). Installed CLI JSON schemas are generated under `.local/codex-app-server-schema`; use its exact `read-only` enum rather than a docs-version spelling assumption. Record experimental integration status and version.

## Verification

Keyboard: actual MCP save → Cmd+Z → Shift+Cmd+Z → Cmd+Z/save, exact runtime/source proof. Completion: Unicode/context/range/limit contracts plus native insertion/selection/stale/IME/undo and popup interaction. Assistant: real subprocess framing/lifecycle/state tests, actual optional CLI handshake, no fabricated LLM response. Final native build/signature and rendered inspection; fresh independent integrated review once. Preserve all user files/settings and distinguish controlled renders, live AX and target-platform tests.
