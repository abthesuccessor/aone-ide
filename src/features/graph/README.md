# Relationship graph feature

This feature presents bounded workspace and code-path facts as a D3 bubble and
link canvas. Flow-stage metadata still colors Frontend / Trigger, API / Event,
Backend, Service / Agent, Repository / Data, and External nodes.

- `ForceRelationshipGraph.tsx` owns the bounded D3 force simulation, node drag,
  graph-local pan and zoom, Fit, source opening, labels, and selection focus.
- `relationshipView.ts` projects All, API, Logic, Data, Code path, and changed
  Git-file categories without mutating backend evidence. Changes synthesizes
  one bounded marker for every Git status entry and links it to every loaded
  exact-path source fact, so a changed file remains selectable even when no
  semantic node is present in the current graph page.
- `ExecutionFlowGraph.tsx` remains the deterministic card renderer used by
  specialized execution and debugger surfaces.
- `flowModel.ts` honors backend `flowStage` and `flowGroup` metadata before
  applying legacy/demo heuristics. It owns trace reachability, loaded-node
  search, and the rule that only nodes whose own evidence is observed gain
  observed card presentation. An observed edge never upgrades a static card.
- `flowLayout.ts` is a pure, memoized layout. It mounts at most seven groups per
  stage and three cards per collapsed group (18 when expanded). A selected
  hidden result is forced into its trace and centered, so every loaded node is
  reachable without mounting the complete large graph. SVG relationships have
  a deterministic 600-edge budget that prioritizes selected and observed facts
  and reports the omitted visible-card relationship count.
- The force simulation is recreated only when graph data or canvas size
  changes, stops after settling, and is not restarted for ordinary selection.
- `model.ts` owns lens filtering and sensitive-node defenses. Sanitized observed
  HTTP/WebSocket boundaries and explicit trace spans may appear in System;
  ordinary stdout/stderr, credentials, `.env`, tests, raw analysis, and resolver
  intermediates remain excluded.

The initial transform fits all six columns horizontally near 0.9 scale and
leaves tall groups pannable. Fit is the explicit whole-map action. Graph Focus
continues to provide the full-window view without resetting selection.

Runtime semantics are intentionally strict. An observed request envelope does
not make internal handlers, services, or repositories observed. Internal cards
retain declared, resolved, or inferred evidence. A process-reported source
path/line candidate and URL/template correlation both create inferred edges.
Only a trusted backend-authored `sourceNodeId` can create an observed link to
static code; the static node still keeps its original evidence.

Source opening always uses the backend-provided UTF-16 range. A bounded root
with a cursor exposes a working Load next action; loaded-node search does not
claim access to backend facts that have not been paged in.
