# LaTeX IDE development context

Mac is the authoritative accepted product/workflow reference. Before Windows/Linux work, read `docs/development/PLATFORM_DEVELOPMENT.md` and the target app's AGENTS.md/README.md. P21 owns the complete Windows native tree and P22 the complete Linux native tree; shared runtime/contracts/workflows and central handoff/ledger/skill changes remain P00 integration work. Use the generated ownership registry, not an older blanket restriction on native files.

Read `docs/DEVELOPER_HANDOFF.md` before changing this project. Use the project skill at `skills/latexide-platform-development/SKILL.md` for shared Rust, macOS, Windows, compiler, MCP, or future Linux work.

The user explicitly asked that substantive Mac development be maintained in Markdown and in a reusable developer skill for Windows and later Linux contributors. After a material change, update the handoff's current state and the implementation ledger with actual commands/results and platform implications. Update skill references when a reusable contract or platform procedure changes; keep chronological detail in the Markdown logs.

Keep implemented, tested, partial, planned, and unrun states distinct. Follow the user's accepted architecture and direct-agent undo contract. A Mac build or Rust cross-check is not a Windows/Linux runtime result. Use repository-relative references in reusable guidance rather than the current developer's absolute paths.

UI work must follow `docs/IDE_PRODUCT_DIRECTION.md`: one coherent research/developer workspace, persistent customization and unsent work, correct focus/keyboard ownership and actual assistant/tool states. Do not equate added panels or visual similarity with a completed IDE; record workflow-level evidence and remaining capability gaps.
