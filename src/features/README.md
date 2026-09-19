# Renderer features

Cross-cutting product capabilities that own state, presentation, and focused
tests live below this folder. A feature may use the shared Tauri bridge and
contracts, but it must not gain native authority or duplicate backend payload
definitions.

- `editor/` owns Monaco tabs, dirty buffers, theme/font/wrap preferences, and
  workspace-ID/generation-bound save and format orchestration.
- `onboarding/` owns the initial Getting Started workbench, guarded Open/Clone/Create
  adoption, provider templates, permission entry points, and manual Git workflow guide.
- `terminal/` renders the single Rust-owned PTY with xterm.js; input uses a
  64-message/256 KiB outstanding budget and output a 32-by-16 KiB bounded pump.
- `source-control/` presents bounded local-only Git with per-file `--no-renames`
  diff and staged-path/content-fingerprint confirmation.
- `search/` presents workspace-bound SQLite FTS and AST-key matches as a lazy,
  grouped, keyboard-navigable result tree that opens exact Monaco ranges.
- `tool-config/` presents a static eleven-entry AI-tool catalog, separately
  consented metadata inspection, and consented external create/open actions for
  allowlisted MCP/CLI, agent, instruction, rule, and skill locations.
- `project-environment/` defines permission-gated toolchain evidence retained
  by the native session for the active workspace.
- `agent-setup/` is the single Project Agent surface for readiness, approval,
  and bounded provider-consented engineering questions. Its action firewall can
  select only an exact registered IDE run profile; model prose, Docker,
  installers, and shell text are never executed.
- `realtime/` owns bounded WebSocket connection and transcript state.
- `graph/` owns the deterministic six-stage execution Flow/Map, nested source
  groups, progressive disclosure/search, safe frontend leakage filters, and
  observed-boundary presentation rules.
- `api-catalog/` owns the center-workbench complete indexed operation inventory,
  full-inventory search, deterministic role/service/route grouping, bounded row
  reveal, and exact source navigation.

There is no extension host. Features receive opaque IDs and workspace-relative
paths; the Rust core remains responsible for containment, native confirmation,
process lifetime, secrets, and local filesystem changes.

Long-running renderer actions must bind completion to the workspace/request or
submitted-content generation that started them. Terminal Open additionally
waits for event subscription, buffers only 32 pre-open events with a visible
overflow notice, surfaces rejected write/resize IPC promises, and best-effort
closes a session that rejects write/resize before clearing stale local state.

The onboarding, editor, source-control, and tool-config features also receive the
shared workspace-mutation lock. Once leave consent succeeds they refuse old-workspace
edit/save/format/close, Git mutation, and config create/open until picker plus
scan/load completion or cancellation; source navigation is guarded in `src/app`.
Before that boundary, the editor's synchronous mutation counter prevents folder
opening from crossing an active save/format, even before busy state renders.
