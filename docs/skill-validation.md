# Project skill validation — 2026-10-30

The user explicitly requested a maintained Markdown handoff and skill for Windows and future Linux developers. The skill is a project-local reference, not an installation into the developer's global memory or skill directory.

Before writing it, an isolated read-only baseline investigation recovered the current contracts, but found stale README/ledger statements and the Mac-specific `/private/tmp` Unix assumption. It did not demonstrate an unsafe behavioral failure; this is a documented discoverability/drift finding, not a fabricated RED test.

After writing it, a second isolated read-only agent used the skill to describe Windows onboarding and the future Linux boundary. It correctly identified TeX Live as the primary basis, source vs type-check vs runtime evidence, direct revision-protected edits with undo after save, and the Linux runtime-directory gap. It reported no broken reference. It found stale Task4 and runtime rows, which were corrected, and requested explicit backend attribution in future compiler test logs. Windows source/build commands now exist in `scripts/build_windows.ps1` and `docs/windows-validation.md`; target restore/MSBuild/link/runtime remain unrun.

Formal validation command, from the repository root:

```sh
PYTHONPATH="$PWD/.local/skill-validation" python3 /Users/szotya/.codex/skills/.system/skill-creator/scripts/quick_validate.py skills/latexide-platform-development
```

Actual result: `Skill is valid!`, exit 0. PyYAML was unavailable in both existing Python runtimes; the validator-only dependency was installed under `.local/skill-validation`, with acquisition output in `evidence/skill-validator-dependency.log`. The absolute validator path describes this Mac environment, not a portable product dependency.

This check validates skill structure and onboarding behavior. It does not validate Windows/Linux builds or certify the product's security.

The developer-UI extension adds `references/ui.md` for shared palette/syntax/native presentation and packaging contracts, plus updated Windows/Linux references. Formal validation after this extension on2026-10-30 also returned `Skill is valid!`. The2026-10-31 documentation closure checked17Markdown documents/51relative targets with0broken, and a fresh formal validator run again returned `Skill is valid!`. Reports are `evidence/ui-redesign/resume-2026-10-31/{markdown-links.json,skill-validation.log}`. No new onboarding agents or repeated code-review cycle are implied.

The later2026-10-31 intelligence increment adds `references/intelligence.md` and pointers from UI/Windows/Linux references. It preserves logical-character/focused-field undo, UTF-16/revision completion and stale-key fallback, popup ownership, session-owned drafts and the real Codex/MCP approval/process lifecycle. Formal validation again returned `Skill is valid!`. The expanded local link check covered28 Markdown files/75relative targets,0broken; scope excludes web URLs/anchors. Current reports: `evidence/intelligence-2026-10-31/{skill-validation.log,markdown-links.json}`. Actual Codex model/MCP/edit and Mac keyboard observations are recorded in the separate current verification; the skill checks do not establish Windows/Linux runtime or full product readiness. No global skill or memory was installed/updated.


Project-aware completion extension2026-10-31: added `references/project-completion.md`, routed from the skill/UI/intelligence/Windows/Linux references. It documents dirty-buffer precedence, cache/content refresh, parser/work budgets, key-list/whitespace/no-op acceptance and actual target boundaries. Formal validator again returned `Skill is valid!`;33Markdown documents/92relative targets,0broken. Current reports: `evidence/project-completion-2026-10-31/{skill-validation.log,markdown-links.json}`. No global installation or memory update; actual implementation/runtime evidence is separate from skill validation.

Capability audit extension2026-10-31: added a narrow `references/audit.md` route for source/test/artifact identity, current product/plugin lifecycle, prototype limits, planned parity requirements and platform qualification. Formal quick validator returned `Skill is valid!`, exit0; the audit data/link validator checked226items,50comparison rows,51sources and39Markdown documents/262relative targets/0broken at closure. Evidence: `evidence/ide-audit-2026-10-31/{skill-validation.log,artifact-validation.json}`. No independent onboarding-agent scenario or new Windows/Linux runtime qualification is claimed. The final guard found concurrent definition-navigation source changes after the earlier green suites; this is explicitly recorded as a new source-only/unrun boundary in the dated audit and `snapshot-delta.json`.
# Parallel-package reference extension — 2026-10-31

Added `references/parallel-development.md` and its route for exclusive paths, the planned contracts gate, integration ownership and private-repository onboarding. The formal project-skill validator returned `Skill is valid!` (exit0); `evidence/repository-2026-10-31/skill-validation.log`. The package validator checks all226requirements/44plans and does not claim service implementation. No global skill or memory update was performed.

