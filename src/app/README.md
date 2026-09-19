# Application composition

This folder connects the existing feature components to the local Tauri bridge.
It owns application-level orchestration, not graph rendering, API authoring, or
workspace-tree presentation; those remain in `src/components`.

## Responsibilities

- `useAppController.ts` owns application state, derived graph state, guarded
  asynchronous actions, and the narrow view model consumed by the shell.
- `useRuntimeSubscriptions.ts` owns desktop event subscription, bounded
  fixed-cadence event batching, readiness gating, terminal-run reconciliation,
  and debounced workspace reloads.
- `useAppShortcuts.ts` owns global keyboard commands and their cleanup.
- `useGraphFocus.ts` owns full-width graph focus, Escape restoration, and focus
  return without changing graph selection or zoom state.
- `useWorkspaceGraphs.ts` owns overview, dependency, and rooted execution-flow
  snapshots; it guards same-workspace trace races and cursor-page merges.
- `useDeveloperWorkbench.ts` composes editor preferences, open documents, and
  the active VS Code-style activity view without growing the core controller.
- `runtimeEvents.ts` creates browser-demo runtime evidence without mixing demo
  record construction into the controller actions.
- `AppShell.tsx` renders the IDE layout and delegates domain UI to components.
- `model.ts` contains application-only state types and immutable defaults.

`src/App.tsx` is intentionally a small composition entry: create the controller,
then pass it to the presentation shell.

## Data flow

Tauri bridge snapshot -> curated overview, anchored detail, and rooted execution-flow queries -> controller state ->
runtime graph projection -> `AppShell` props -> existing feature components.
`systemOverview` loads first (depth 2, at most 60 nodes); its sorted IDs, capped
at 50, root the depth-4 / 500-node neighborhood. Architecture reads the curated
overview; Dependencies reads the anchored neighborhood; System and Runtime use
the backend `executionFlow` projection. API + Data can query that projection by
the catalog operation's canonical node ID, then page root relationships by its
opaque cursor. All results share workspace and same-workspace request guards.
User interactions travel in
the opposite direction through controller actions, which call the bridge and
commit only results belonging to the current workspace generation.

Workspace changes increment a generation token before state is cleared. A
separate monotonically increasing source-request token also prevents an older
same-workspace file read from replacing a newer selection. Dirty editor state
gates the shared folder-opening action before the native picker is invoked.
Startup also holds `initializingRef` and a non-ready phase until the initial
snapshot and matching-generation workspace payloads are committed; Open Folder
returns before picker IPC while that guard is set.
Open Folder and explicit rescan share a synchronous operation ref, so a second
renderer action cannot enter either bridge call while the first is active.
Late file, graph, and source responses are ignored when their tokens no longer
match. Runtime events retain the most recent 500 entries, and a process
that exits before its start response is reconciled through the terminal-run set.
Incoming runtime events are committed to React state once per 100 ms batch—at
most 10 timeline updates per second. Both the pending buffer and committed
timeline keep the newest 500 events in callback order. Terminal events reconcile
`activeRun` immediately, before their timeline batch is painted. Cleanup cancels
any scheduled flush and clears its pending references. Opening a different
workspace cancels the pending batch before the old timeline is reset, preventing
late logs from crossing the workspace boundary.

Folder opening keeps progress transient: the shell shows the scan as a thin
progress line, clears it after success, cancellation, or failure, and reports a
successful committed index once through the side toast. Native Tauri IPC may
reject with a string rather than a JavaScript `Error`; the controller normalizes
that bounded text so a useful backend failure reaches the toast instead of being
replaced by a generic workspace error. The desktop subscription accepts scan
events only while the shared workspace-operation ref is active; `complete`
clears progress, and a late terminal-phase event after `finally` cannot restore
the line.

## Conventions

- Keep bridge calls and race guards in the controller, DOM-only effects in a
  focused hook, and markup in the shell or an existing feature component.
- Do not duplicate backend security decisions in presentation code. Renderer
  checks improve guidance only; Rust remains authoritative.
- Preserve evidence labels (`static`, `observed`, `inferred`) exactly.
- Keep handwritten TypeScript and TSX files below 500 lines. Extract a cohesive
  action/effect/presentation boundary before adding another large branch.
- Prefer immutable array updates and bounded tails for live event collections.
- High-rate event callbacks enqueue into the shared 100 ms batch rather than
  calling a React state setter once per event.

## Accessibility

The shell preserves semantic `main`, `header`, and `footer` regions, button and
tab controls, `aria-busy`, polite scan/source status announcements, graph lens
pressed states, and keyboard focus behavior. Global shortcuts must prevent the
browser default only when Aone handles the command, and every effect must remove
its listener or subscription during cleanup.

## Tests

`src/App.test.tsx` is the integration contract for this folder. It exercises
initialization, graph-to-source synchronization, environment isolation, REST and
GraphQL flows, inferred AI evidence, Observe mode, and Command-R execution.
`workspaceOpening.test.tsx` locks the startup picker guard, overview-to-sorted-
roots query order, Dependencies-only neighborhood selection, folder-open cancellation,
generation guards, Open Folder/rescan serialization, transient progress cleanup,
single completion notification, and native string error presentation.
`useRuntimeSubscriptions.test.tsx` proves listener readiness is not exposed
until asynchronous subscription installation completes, ignores late scan
progress after an operation ends, and locks the 100 ms / 500-event batch limits,
immediate terminal reconciliation, and timer cleanup.
`useGraphFocus.test.tsx` locks Escape exit, focus return, and listener cleanup.

```bash
npm test -- src/App.test.tsx
npm run build
npm run size
```
