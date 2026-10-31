# IDE intelligence — independent review and fix record

Date:2026-10-31. One fresh, read-only integrated review covered the new Rust completion contract, native keyboard/popup integration, preferences, Codex subprocess/session and inspector integration. It ran after implementation and initial verification. No second review is claimed. The earlier foundation/UI reviews concern their respective increments.

The reviewer reported0Critical,3Important and1Minor before fixes. The author regraded the popup retain cycle to **Important** because repeated popup creation retained native hosting hierarchies, directly conflicting with the user's RAM requirement. All four findings were fixed in one integrated fix pass, with reproductions before changes and green tests afterward.

| Finding | Reproduction and final change | Evidence |
| --- | --- | --- |
| Unsent assistant prompt and attachments disappeared when the disposable panel was recreated. Important. | `AssistantDraftTests.testUnsentPromptSurvivesPDFAndHiddenInspectorRoundTrips` inserted text into an actual native TextEditor, switched PDF/Assistant and hid/reopened the inspector. It reproduced the lost prompt. AssistantSession now owns draft/attachments outside that view; explicit project reset/new conversation still clears context. | `review-native-red.log`; final native suite. |
| Completing inside an existing starred command could produce `\section{}*{Title}`. Important. | `starred_commands_keep_the_existing_star_and_arguments` reproduced the wrong insertion, including whitespace/star/options. Shared Rust completion now preserves the existing star/arguments and replaces the complete command word. | `review-star-red.log`; six completion contracts in `rust-tests.log`. |
| A stale proposal could consume Return/Tab after acceptance rejected it. Important. | Actual native Tab case reproduced the missing tab; authoritative-revision Return case reproduced the consumed event. The controller now dismisses a stale proposal and returns false, allowing ordinary native editing. Tests separately cover stale revision, moved caret and no unwanted mutation. | `review-completion-red.log`, `review-revision-red.log`; final CompletionTests. |
| Controller → panel → hosting view → observed controller retained the popup after editor removal. Important (author regrade). | A real NSWindow/NSHostingView fixture's weak controller remained alive. Dismiss now removes the child window, clears contentView and releases the panel. The deallocation test passes. | `review-completion-red.log`; `testPopupReleasesItsControllerWhenEditorIsRemoved`. |

Evidence above is under `evidence/intelligence-2026-10-31/`. The first native review log contains several cases; the exact revision-only Return reproduction is recorded separately because typing/auto-scroll could legitimately dismiss the popup before that path was reached.

## Additional live integration finding

The subsequent actual Codex model turn initialized and streamed, but `read_document` failed: MCP approval was required and the thread policy was `never`. The prior `default_tools_approval_mode = auto` was not an unconditional grant. This was an observed product failure, not an inferred security warning.

The fix grants `approve` only to the nine existing IDE tool names, sets the same `enabled_tools` allowlist and leaves the server's unknown-tool default at `prompt`. Other inherited servers/apps/shell/web stay disabled and raw filesystem access stays read-only. No global configuration or authentication was changed. This implements the user's accepted direct-agent edit contract without adding per-edit prompts. Explicit save remains its own MCP operation; the assistant's developer instructions require a user save request.

`testIDEOnlyApprovalGrantAllowsMCPWithoutEnablingUnlistedTools` failed first with the approval error, then passed together with all assistant regressions. Actual upstream semantics were checked against the installed generated schema, the [configuration reference](https://developers.openai.com/codex/config-reference) and [OpenAI's MCP approval resolver](https://github.com/openai/codex/blob/main/codex-rs/codex-mcp/src/mcp/mod.rs). The same actual-model read then completed and returned the correct title. Exact live editing/history results belong to [verification](intelligence-verification.md).

## Author rulings on scopes set aside by review

| Scope | Ruling and practical cost |
| --- | --- |
| General TeX semantics, packages and user-defined commands | Keep the bounded authored catalog. This increment does not claim an LSP, arbitrary macro expansion, reference/citation completion or semantic project index; these are explicit next product capabilities. |
| Windows/Linux native ports | Shared Rust query/type-check evidence is portable input, not a native popup/process implementation. Actual target UI, keyboard, IME and provider tests remain unrun. |
| Model/provider/authentication and CLI stability | Separate controlled subprocesses, initialize-only handshake and actual model turns. The installed0.166.0-alpha.5.1 integration is experimental; a successful small own-project cycle does not qualify all providers/accounts/CLI releases. |
| Actual physical keyboard and accessibility | Preserve the measured logical-key result and CUA live actions. No broad hardware-layout, VoiceOver or full accessibility qualification is claimed. |
| Private/global Codex configuration | Do not inspect or rewrite credentials/global settings to satisfy a test. Use effective app-server config only for transient isolation, without logging private values. Managed-policy incompatibility should remain an explicit error. |
| Release RAM/performance | Fix the concrete retained popup. No new release RSS/long-session benchmark is inferred from the weak-reference regression or previous170MiB debug snapshot. |
| Documentation and product completeness | The author owns the handoff/ledger/skill update and capability matrix. Added panels are not evidence of a finished research/developer IDE; lifecycle, semantic navigation, SyncTeX, citations, Git and recovery remain tracked. |

The final source/tests, staged package and maintained docs stay local. No Git repository, merge target, commit, publication or repeated review cycle was added.
