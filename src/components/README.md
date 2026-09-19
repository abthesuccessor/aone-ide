# Components

These files render one product surface each: the graph facade, editable source,
activity bar, evidence, runtime/API/terminal tools, workspace navigation, title
bar, and small interaction primitives.
Components receive typed data and callbacks; privileged work remains in the Rust
commands exposed through `src/lib/bridge.ts`.

Workspace-facing components receive a shared busy flag while folder selection,
scan, and load are in flight. Explorer/source controls and tab close stop
old-workspace actions during that interval; feature panels independently guard
their mutation handlers so disabled presentation is not the only check.

On macOS desktop, `Titlebar` reserves the native traffic-light region, omits the
redundant Aone/workspace identity block, and centers the command field against
the complete window. Workspace controls return after a workspace opens. Its
Scan action keeps a stable label while accessible progress lives in the shell's
two-pixel indicator, and completion text belongs only to the side toast.

`WorkspaceExplorer` renders at most 5,000 initial paths, then debounces non-empty
queries through the same Rust file-list command so any safe indexed file is
discoverable without mounting all 20,000 possible rows. Renderer and Rust bound
the control-free query to 256 characters; Rust searches the complete index and
returns at most 200 case-insensitive substring matches. Query/workspace
generations reject stale responses, and the live footer states whether it is
showing the initial window, searching the full index, or displaying capped
results. A path-keyed `Map` builds the bounded tree without repeated sibling
scans.

`GraphCanvas` delegates its Flow and Map views to the deterministic execution
renderer in `src/features/graph`. It and `WorkspaceExplorer` use one roving
tab stop per composite. Arrow, Home, and End keys navigate; graph Enter/Space selects,
Command/Ctrl+Enter opens source, and tree Left/Right collapse or descend. If a
filter/lens hides the focused path or selected graph node, the active or first
visible item becomes the fallback tab stop. The canvas horizontally fits all
six stages near 0.9 scale; Fit also shrinks tall content into view.
Selecting a card creates a visually hidden live region listing incoming and
outgoing neighbor, edge-kind, and evidence details, so SVG geometry is not the
only relationship representation.
Map uses bounded group/card disclosure and never starts a force simulation.
The shell's Focus action hides surrounding panels and the console while
preserving graph state; Escape restores the prior layout and invoking focus.

API + Data does not use `GraphCanvas`. The shell lazy-loads the dedicated center-
workbench catalog so its complete paged inventory has enough horizontal space.

The activity bar exposes Search beside Explorer. Its panel is lazy-loaded and
reuses the app controller's source-open path, so a content, symbol, API, or
event-listener result reveals the exact returned range in Monaco.

Keep data loading and long-lived subscriptions in `src/app`, keep graph-specific
layout and classification in `src/features/graph`, and split a component before it exceeds 500 lines.
Stateful editor, terminal, Git, and MCP/CLI panels belong in their respective
`src/features/` folder; components must not construct paths, commands, or
executables that bypass the Rust bridge.
