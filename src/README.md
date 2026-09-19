# Renderer source

The renderer is organized by responsibility rather than by technical layer.

- `app/` composes core application state with the developer workbench, subscriptions, shortcuts, and layout.
- `components/` contains focused graph, source, activity-bar, runtime/API, evidence, and shell components.
- `contracts/` adapts generated IPC schemas for presentation code.
- `data/` supplies the deterministic browser-demo fixture.
- `features/` owns Getting Started onboarding, the deterministic execution Flow/Map, complete indexed API catalog, workspace search, editor/settings/themes/tabs, Project Agent, local source control, the integrated terminal, the AI-tool configuration hub, and the real-time WebSocket console.
- `hooks/` contains reusable React behavior.
- `lib/` owns narrow core/onboarding/search/editor/terminal/developer-tool Tauri bridges and pure presentation helpers.
- `styles/` is the ordered Tailwind/CSS design system.
- `test/` owns shared Vitest setup.
- `generated/ipc/` is reproducible OpenAPI Generator output.

Handwritten TypeScript, TSX, and CSS files are limited to 500 physical lines by
`npm run structure`. Generated contract code is still kept below the same limit
by the selected generator layout.

The renderer is not an authority boundary. It may buffer editable text and raw
terminal display bytes, but Rust selects and contains workspaces, validates and
atomically saves files, owns formatter/PTY/Git/configuration execution, retains
secrets, and requires native consent where documented. Browser-only development
uses deterministic simulations instead of performing those privileged actions.

Workspace/query generations guard Explorer, workspace Search, API inventory, Git, Project Agent, and tool-configuration
responses against late async completion. Editor saves/formats additionally bind
requests to the backend-issued workspace ID and results to submitted content, so
a workspace switch or typing during an operation cannot overwrite the newer
view/buffer. Runtime events enter an ordered 100 ms renderer batch, while
ordinary stdout/stderr without a source node stay out of graph projection so
log-only traffic does not recompute the static execution map.

Save/format owns a synchronous mutation counter that stops folder opening before
discard consent or picker invocation. After dirty-document leave consent, the
shared `openingWorkspace` state is set
before picker IPC and remains through scan/load success or cancellation. Editor
mutation/save/format/close, new source opens, Git mutations, and tool-config
create/open are disabled and handler-guarded during that transaction.
Before normal ready state, a separate startup guard blocks the folder picker
until the initial snapshot and both workspace graph snapshots can no longer
race a new selection.
