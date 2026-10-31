# Developer UI final review — 2026-10-30

One fresh-context read-only whole-change review covered shared syntax/protocol/palettes, native Mac editor/workspace/packaging, and Windows source. No Git repository exists; current scoped files, design/plan and evidence were the review package. Initial verdict: request changes, two P2 findings; no P0/P1 found. This does not establish absence of every bug. The author fixed both in one pass; no second review is claimed.

| Finding and trigger | Disposition and evidence | Platform boundary |
| --- | --- | --- |
| Mac tab switch collapsed a nonempty selection and scrolled to the caret; hiding the last tab discarded coordinator-owned presentation. | Fixed. `EditorViewState` now lives in `WorkspaceStore`, stores both range endpoints and viewport, and survives editor destruction. Selection/scroll callbacks and dismantle capture state; restore clamps UTF-16 endpoints at scalar boundaries. Only explicit navigation scrolls to the caret. `EditorViewportTests` uses the actual native creation/update/dismantle paths. After exposing those paths, `viewport-behavior-red.log` records `{4,10}` becoming `{4,0}` and y850 becoming y0, plus last-tab state loss; the final native suite passes both new tests. | AppKit regression, not a hardware keyboard/IME acceptance test. State is per-project and cleared on project switch. |
| Windows project without `main.tex` opened its first `.tex`, but Build retained the old main-file field. | Fixed. `ProjectFiles.h` selects `main.tex` or first `.tex` independently for each project; OpenFolder assigns that value before Load. The portable helper covers `thesis.tex`, main-file preference and an empty/non-TeX project. | Finding is source-provable; helper executes on Mac. No native Windows reproduction/build is claimed. |

Reviewer strengths: borrowed authoritative syntax with revisions and scalar-safe spans; attribute-only/IME-aware coloring; retained history and metadata-only dirty refresh; coherent compact native render; staged package/archive stamp and canonical assets; guarded plain-text Windows paste and RichEdit round-trip checks; explicit evidence limits.

## Author rulings on the reviewer's set-aside scopes

1. Full macro expansion/semantic parsing stays outside this lexical highlighter. Cost: custom macro meanings and every TeX grammar extension are not inferred.
2. Core document eviction stays deferred. Cost: hiding retains text/history and still occupies the ten-document lifetime budget; the UI names the action **Fül elrejtése** and exposes hidden documents.
3. Windows fuzzy Quick Open popup, searchable command list and PDF text-selection/search parity stay partial. Cost: Windows currently has a filename filter/action menu and page rendering; do not claim full Mac feature parity.
4. Windows native visual/input qualification remains unrun without a runner. Mac live AX subsequently observed project open, real bundled compile/PDF, palette/mode clicks, Quick Open and Find; ScreenCaptureKit later failed again. Cost: controlled native images and tests supply narrower evidence, not a complete live interaction acceptance or readable live screenshot. The final presentation-state fix is covered by native tests and the refreshed controlled render.

Current verification and commands: [ui-verification.md](ui-verification.md). The earlier foundation review remains [foundation-review.md](foundation-review.md).

Follow-up2026-10-31: documentation closure and the final app's real MCP edit/save → visible native-menu undo → explicit save → successful runtime PDF build were verified without source changes or a second review. ScreenCaptureKit failed again before the final PDFKit observation; hardware shortcut acceptance and Windows qualification were not promoted to PASS.
