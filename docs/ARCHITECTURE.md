# Aone v0.1 architecture

## Product boundary

The first release implements one complete path:

```text
start in Getting Started: open, clone public GitHub, or create in Documents
  -> index source
  -> search or choose a document, entry, or API in a bounded graph snapshot
  -> open, edit, format, and safely save indexed text
  -> run a local service
  -> send an HTTP request or open a WebSocket session
  -> use one bounded local PTY or local-only Git workflow
  -> inspect or restore a permissioned local Project Agent report
  -> inspect or open an allowlisted AI-tool configuration externally
  -> correlate evidence
  -> navigate to source
  -> request an optional explanation
```

AI setup is optional: opening, scanning, indexing, graph navigation, and export
remain available through **Continue code-only**. This is a production-shaped
proof of concept, not proof of an installed, packaged, deployed, or live system.

## Source architecture

The repository uses feature folders so authority, contracts, tests, and documentation stay close to the behavior they describe.

```text
src/
  app/          React orchestration, workbench state, subscriptions, shortcuts, and shell
  components/   graph, source, activity, runtime, API, evidence, and shell views
  contracts/    handwritten adapters over generated IPC schemas
  data/         deterministic browser-demo data
  features/     onboarding, graph/search/artifacts, editor/settings, terminal, source control, project environment, tool configuration, and realtime features
  generated/    reproducible OpenAPI Generator output
  hooks/        reusable React behavior
  lib/          Tauri bridge and pure presentation helpers
  styles/       ordered feature CSS
  test/         shared Vitest setup

src-tauri/src/
  ai/ analyzer/ commands/ domain/ environment/ error/ onboarding/
  editor/ git/ graph_algorithms/ http_client/ network_policy/ project_environment/ run_profiles/
  runner/ otlp/ terminal/ tool_config/ websocket/
  scanner/ search/ state/ store/ watcher/
```

`src-tauri/src/lib.rs` is the Rust composition root. Inside each Rust feature folder, `mod.rs` contains
module declarations and public re-exports only; behavior lives in named implementation files. Each source module directory that directly contains checked source has a `README.md` covering responsibility, data flow, invariants, limits, and tests.

`npm run structure` enforces a 500-physical-line ceiling for Rust, TypeScript, TSX, JavaScript module,
and CSS files under `src/`, `src-tauri/src/`, and `scripts/`. It also parses every Rust `mod.rs` to
reject implementations and requires module READMEs. Generated TypeScript is exempt from the README
rule, but its files still remain under the line ceiling. The limit is a reviewability guard, not a
substitute for cohesion or tests; a file should be split earlier when it has more than one reason to
change.

## Trust boundary

The React WebView is unprivileged. All authority is concentrated in a narrow Rust broker.

| Capability | React | Rust |
| --- | --- | --- |
| Render graph and edit source | owns presentation and buffers | supplies bounded indexed text; validates and atomically saves |
| Search the workspace | debounces and groups exact matches; holds no source index | validates the workspace, queries document keys, securely verifies source, and returns bounded ranges |
| Drag custom titlebar | marks the native drag region | main-window-only `window.start_dragging`; no broader window-control grant |
| Select/read local files | no | native-selected workspace, contained indexed source/run env, and native-picked external `.env` files |
| Bootstrap a project | supplies a bounded public GitHub URL or safe child name | confirms, creates/clones one direct Documents child, then scans and installs it |
| Spawn and stop processes | no | explicit Run/Debug action over an exact backend-detected profile plus immediate identity revalidation |
| Open an interactive terminal | xterm.js renders/forwards bounded bytes | discovers the shell, confirms, owns the PTY and cleanup |
| Run Git | presents status/per-file diff and fixed actions | fixed local binary, exact repository root, contained non-sensitive literal paths, consent for mutations |
| Inspect Git onboarding | requests permission and renders sanitized metadata plus a manual checklist | consented bounded Git/public-key metadata only; no private key, SSH, network, or mutation |
| Open tool configuration | shows allowlisted metadata only | resolves/creates/opens the allowlisted file after consent; never returns contents |
| Inspect project environment | requests an explicit report and renders evidence | performs consented bounded metadata plus names-only build/config hint discovery and fixed version probing; never installs or edits |
| Hold environment secrets | no | separate provider/run stores in memory only |
| Send API/provider traffic | no | validated, bounded, direct, and natively confirmed at the authority boundary |
| Persist structural graph | no | SQLite in Application Support |
| Hold runtime evidence | renders a bounded projection | bounded memory queue in Rust |
| Receive local traces | shows lifecycle/configuration/replay only | consented loopback listener, token admission, bounded OTLP decoding |

No application login or role system is present in v0.1. This does not remove operating-system, filesystem, process, or IPC safety checks.

## Evidence model

Every node and edge has one provenance class:

| Evidence | Meaning | Example |
| --- | --- | --- |
| `declared` | Directly present in source or supported text structure | import, function, route, heading, sentence |
| `resolved` | Deterministically linked by analysis | uniquely resolved relative import target |
| `observed` | Captured during execution | process output, API request, response timing |
| `inferred` | Heuristic or model-generated | likely route match, architecture explanation |

Inference may reference facts and observations. It may never overwrite them.

## Modules

| Module | Responsibility |
| --- | --- |
| domain | Serialized workspace, search, graph, editor, terminal, Git, tool-configuration, project-environment, runtime, network, AI, and WebSocket DTOs plus provenance enums |
| scanner | Rust-owned discovery, ignore policy, hashing, resource budgets, and containment |
| analyzer | Tree-sitter code adapters, exact Markdown/MDX/TXT/RST/AsciiDoc structure, iterative bounded traversal, and framework heuristics |
| store | SQLite migrations, atomic file replacement, contentless FTS5 projections, neighborhood queries, and the bounded system-overview projection |
| search | Workspace-bound request validation, indexed candidate retrieval, hash-verified exact matching, source previews, and search budgets |
| state / watcher | Installed workspace lifecycle, serialized analysis, bounded change batches, and incremental refresh |
| run_profiles / runner | Structured manifest detection, explicit-action exact-profile launch, identity revalidation, process supervision, and output events |
| environment | Single-open `O_NOFOLLOW` identity-checked parsing of backend-selected `.env` files without expansion or execution |
| commands | Narrow renderer-facing orchestration for workspace, graph, environment, and profile operations |
| network_policy | Shared IPv4/IPv6 and cloud-metadata destination classification for outbound transports |
| HTTP client | REST/GraphQL-style requests, IP-literal-loopback explicit Send or bounded native confirmation, destination validation/DNS pinning, limits, redaction, evidence |
| OTLP | Opt-in loopback HTTP/JSON receiver, ephemeral capability token, semantic allowlist, workspace lifecycle, and observed runtime conversion |
| WebSocket | Direct `ws`/`wss` sessions, destination validation, per-connect native consent, bounded messaging, typed events |
| editor | Indexed UTF-8 validation, pre-commit analysis, optimistic hashes, atomic saves, recoverable derived graph refresh, safe normalization, and consented formatter allowlist |
| terminal | Backend shell profiles, one clean-environment PTY, bounded raw-byte transport, resize/input validation, and process-group cleanup |
| Git | Exact-root status/per-file diff plus consented init/stage/unstage/commit using fixed `/usr/bin/git`; no remote or destructive surface |
| onboarding | Permission-first Documents create/public GitHub clone, standard workspace installation, and sanitized Git/public-key metadata inspection |
| tool_config | Permissioned metadata-only allowlist for eleven MCP/CLI/agent/skill/instruction/rule locations, private template creation, and external text-editor opening |
| project_environment | Two-stage permissioned stack/tool discovery, silent current-workspace report restore, bounded names-only configuration hints, canonical fixed version probes, deterministic recommendations, and workspace/report-bound state |
| AI | Optional hosted OpenAI/Anthropic, loopback Ollama, and audited Codex CLI transports for Rust-owned tasks over bounded graph or Project Agent evidence, per-call native consent, single-call reservation |
| graph_algorithms | Deterministic SCC and structural-community analysis over bounded in-memory projections |
| Tauri | Small command/event surface and application lifecycle |

Service behavior, privileged state, serialized DTOs, and presentation controllers live in cohesive,
named files inside those features. Aone intentionally avoids one-implementation `IService`-style
traits: a trait is added only when there is a real substitution, test-double, or plugin boundary.
Adding an interface in front of every concrete service would add navigation and indirection without
changing authority or testability. Feature modules are compile-time boundaries, not a public plugin
API. Arbitrary native dynamic libraries are deliberately excluded. A later plugin SDK should use
explicit capability contracts, with WASI as the current direction.

## Contract pipeline

`openapi/aone-ipc.openapi.yaml` is the OpenAPI 3.1 source of truth for renderer-visible Tauri
commands. Its POST paths are documentation surrogates carrying `x-aone-tauri-command`; they are not
network endpoints, and Aone does not start an HTTP server for IPC. The React bridge still calls
Tauri `invoke`, so filesystem, process, secret, and network authority remains in Rust.

`npm run openapi:generate` runs the official OpenAPI Generator 7.24.0 `typescript-fetch` generator
from a Docker image pinned by SHA-256 digest. Only models and supporting TypeScript are retained; an
HTTP client is not used as the desktop transport. Handwritten types in `src/contracts/` adapt those
generated schemas for presentation without duplicating payload definitions. The project compiler is
fixed at TypeScript 7.0.2, and the normal production build compiles the generated output with it.

`npm run openapi:check` verifies source and generated tree hashes, generator identity, the exact
TypeScript 7.0.2 package/manifest version, and parity between `x-aone-tauri-command` values and the commands registered in
Tauri's `generate_handler!` macro. `npm run openapi:docs` starts
`swaggerapi/swagger-ui:v5.32.11` locally with submit methods disabled.
The OpenAPI tree documents 56 registered commands. Onboarding, search, developer-workbench, API inventory, Project Agent, debugger, observability, and AI
paths/schemas are split into focused files under `openapi/components/` to preserve the 500-line
source limit.
`asyncapi/aone-events.asyncapi.json` documents the five Rust-originated Tauri event
channels as AsyncAPI 3.1, reusing OpenAPI schemas; `npm run asyncapi:check` verifies listener parity.
Neither contract advertises a public network control plane.

## Storage

SQLite with WAL mode is the canonical store because it is embedded, transactional, crash tolerant, queryable, and tiny. The workspace is never used as a cache directory.

SQLite data in v0.1:

- file fingerprints and language capability
- graph nodes, graph edges, source spans, and provenance
- declared document headings, sentences, `contains`, and `precedes` facts
- contentless trigram source and selective semantic search projections keyed back to file and node rows
- a read-only API-operation projection over indexed endpoint facts

The Explorer initially requests a deterministic path-ordered window of at most 5,000 files. A
non-empty query takes a separate complete-index path: a parameterized case-insensitive substring
search accepts at most 256 non-control characters and returns at most 200 rows. Renderer debounce
and generation checks keep an older query or workspace response from replacing current state.

Run profiles, Project Agent reports, run/HTTP/OTLP events, run environment values, WebSocket session controls, terminal bytes, and AI explanations are bounded in memory. Editor theme/font/layout preferences are the only WebView-local state and contain no source or secrets. Project Agent retains only the latest approved report and may restore it without a new prompt only when it belongs to the current workspace; frontend generations discard stale results. Run profiles, reports, runtime events, run environment state, the active OTLP receiver/token, and editor buffers clear on workspace switch and application exit; a switch is refused until an active terminal is closed. WebSocket sessions are application-session network state and are closed on application exit; their typed events are not persisted. The selected hosted key, Ollama endpoint/model, or attested Codex adapter is application-session state: it remains across workspace switches until replaced or the app exits. Direct keys briefly traverse the masked WebView field and typed IPC request before zeroizing native retention; imported keys never return to the WebView. Git state remains in the opened repository, and tool configuration remains in its external allowlisted file. Broader runtime persistence remains later work.

Workspace opening has both renderer and Rust transaction boundaries. Save/format counters block a
same-tick switch; after dirty-document consent, one generation-bound guard covers native selection,
scan, graph/file loading, and cancellation. Old-workspace edits, source opens, Git mutations, and
configuration actions are guarded while Rust workspace IDs and path validation remain authoritative.

Open, Clone, Create, and rescan share serialized workspace installation. Clone/Create accept no
absolute destination, act only after native approval, refuse overwrite/identity changes, and feed
the same scanner/store/watcher path. Same-root rescan reuses its context and SQLite handle.

Progress enters `committing` before atomic graph/search replacement and reaches `complete` only after
the same-generation workspace state is installed. Late events are rejected; bounded native errors
remain actionable without exposing arbitrary object serialization.

`petgraph` is used only for bounded projections and algorithms. This avoids loading a repository-scale graph into memory and keeps the storage engine replaceable.

`systemOverview` is a curated query, not another database. Fixed quotas seed configuration, entry,
interface/job, application, data, and external candidates before conservative depth-two expansion,
capped at 60 nodes/120 edges. At most 50 sorted overview IDs anchor a depth-four/500-node dependency
neighborhood. Architecture uses the curated snapshot; Dependencies may expose broader inferred/test
evidence; System and Runtime use rooted `executionFlow`.

Manifest/analyzer facts support exact placement. Conventional paths and unverified absolute non-local
HTTP(S) targets remain inferred with a human-readable basis; no provider/cloud identity is claimed.
The overview excludes tests/E2E, secret-like paths, raw call-target/module nodes, and inferred
resolution noise. It never reads configuration contents or synthesizes a provider, webhook,
schedule, or executed flow, and a visible `truncated` flag means the view is incomplete.

PGlite and a separate graph server were evaluated and rejected for v0.1. The
Rust-owned SQLite store already supplies the required single-host transactions,
FTS5 candidates, and indexed adjacency rows without adding a WebAssembly/
JavaScript persistence boundary or a daemon. Tree-sitter facts, persistent
SQLite nodes/edges, bounded `petgraph` algorithms, and D3 presentation form the
implemented AST/graph pipeline. See [Search and storage decision](SEARCH_STORAGE_DECISION.md)
for primary-source links, measured probes, and explicit revisit criteria.

## Algorithms and data structures

### Initial scan

- Algorithm: ignored directory walk followed by deterministic per-file parsing and content hashing.
- Complexity: `O(F + B)`, where `F` is visited file count and `B` is bytes hashed for changed candidates.
- Memory: discovered paths plus per-file parse results; file reads are individually capped at 2 MiB.
- Workspace budgets: fail the scan instead of silently returning a partial repository after 20,000 qualifying files, 256 MiB of aggregate candidate source, 500,000 extracted facts, or 120 seconds.
- Parser budgets: AST and identifier searches use iterative stacks with independent depth/visit caps, and each file can emit at most 5,000 facts. A limited traversal marks the file as having parse/analysis errors instead of claiming complete analysis.
- Trade-off: hashing costs I/O but makes incremental invalidation deterministic.

### Incremental scan

- Algorithm: debounce filesystem events, hash and reparse changed files, then atomically replace facts owned by each changed file.
- Complexity: changed-file parsing is proportional to changed content, but v0.1 then rebuilds workspace-level resolution projections over stored declarations, calls, and imports.
- Safety: watcher-triggered full rescans use the same workspace and parser budgets as the initial scan.
- Trade-off: body changes always re-run extraction because unchanged signatures can still hide changed calls or SQL; update cost can include a repository-wide stored-graph pass.

### Stable identity

- Structure: a path-derived workspace ID plus normalized relative path, language, node kind, qualified label, signature, owner, and deterministic semantic-occurrence ordinal for named symbols. The occurrence prevents repeated same-name/same-signature declarations in separate test or implementation scopes from colliding in SQLite. Edit-sensitive AST facts also include the file content hash, owner, AST kind, ordinal, and byte range without hashing or retaining an entire subtree.
- Trade-off: named IDs usually survive unrelated body edits but can change when signatures or the order of equivalent declarations changes. Anonymous IDs are intentionally more edit-sensitive.

### Document extraction

- Input: `.md`, `.mdx`, `.txt`, `.rst`, `.adoc`, and `.asciidoc` safe UTF-8
  files use the document parser while retaining the `textOnly` capability.
- Output: exact `heading` and `sentence` nodes plus declared `contains` and
  `precedes` edges. All ranges use one-based Monaco UTF-16 columns and exclusive
  ends; document edges carry no invented confidence.
- Syntax: Markdown ATX/Setext, RST underline, and AsciiDoc `=` headings are
  recognized. Markdown-style fences in Markdown/MDX/TXT/RST and AsciiDoc
  `[source]` blocks are skipped.
- Bound: at most 250,000 visited lines and 5,000 facts per file. Parser/format,
  segment order/hash, counts, and explicit line/fact truncation reasons remain
  in metadata.
- Boundary: binary PDF, image, audio, video, and other media are unsupported and
  are not automatically uploaded.

### Neighborhood query

- Algorithm: bounded breadth-first search for equal-cost hops.
- Complexity: `O(Vb + Eb)` within the requested bound.
- Memory: `O(Vb)` visited set and queue.
- Why BFS: a readable focus-plus-one-or-two-hop lens is more useful than rendering the full graph.

The relationship-search control accepts one control-free label/path query of at
most 256 characters and requests a depth-two neighborhood capped at 500 nodes.
Rust also bounds node-kind and edge-kind filter lists. Search returns stored
facts only and exposes truncation rather than claiming a full-graph result.

### Structural communities

- Algorithm: deterministic bounded label propagation over the returned graph;
  edges are treated as undirected and deduplicated, dangling edges are ignored,
  and isolates form singleton communities.
- Metadata: `community:<32 hex>` ID, returned-snapshot member count,
  `deterministicLabelPropagationV1` algorithm, `boundedGraphSnapshot` basis, and
  `communityComplete` equal to the inverse of snapshot truncation.
- Boundary: communities describe current bounded topology. They are not
  semantic topics, teams, services, ownership, or observed runtime clusters.

### System-overview query

- Algorithm: deterministic per-layer candidate queries, fixed quota round-robin,
  then bounded relationship expansion over real stored nodes and edges.
- Default/maximum: depth 2, 60 nodes, 120 edges. User-supplied overview limits
  may reduce but never raise those ceilings.
- Ordering: non-test connected candidates first, then case-folded label and
  stable ID; final nodes sort by system layer, label, and ID.
- Truth boundary: `systemLayerEvidence` describes only presentation placement as
  `exact` or `inferred`. It never changes declared/resolved/observed/inferred
  provenance and never asserts that a conventional job path actually runs.
- Diversity: fixed quotas prevent alphabetically early disconnected file rows
  from consuming the complete overview budget.

### Execution-flow query

- Input: exactly one existing graph/API root, an optional opaque query-bound
  cursor, depth at most four, response limit at most 500 nodes, and test
  inclusion off in the product request.
- Algorithm: deterministic bounded breadth-first expansion over selected
  execution relations. The root page contains at most 100 relationships;
  descendants contribute at most 40 each. Traversal stops at 500 expansions,
  20,000 scanned links, 500 nodes, or 2,000 edges and reports truncation rather
  than implying completeness.
- API bridge: a catalog operation may join to a unique producer/handler only
  when the stored service/protocol/method/path evidence supports it. Selecting
  **Trace flow** roots this query; it does not transform the complete API
  catalog into one canvas.
- Resolution truth: a `calls` plus unique `resolvesTo` pair may collapse into a
  readable hop while retaining both source edge IDs. Inferred global-name
  resolution is never followed across languages. A same-language inferred
  match remains a non-recursive candidate because lexical binding was not
  proven.
- Privacy and production scope: scanner-hard-denied paths are excluded before
  paging or counts, and tests/E2E are excluded unless a non-product caller
  explicitly requests them. Root totals, root omissions, per-node child totals,
  and cursor continuation remain visible.
- The renderer merges no more than 1,000 nodes, 2,000 edges, or ten pages for a
  selected flow. Reaching that client ceiling disables continuation and asks
  the engineer to choose a narrower root.

### Graph presentation

- The ready workbench composes a left API/file Knowledge Navigator, a nested
  source-plus-D3 center, the evidence inspector, and a vertically resizable
  terminal/runtime console. Accessible `react-resizable-panels` separators keep
  these surfaces simultaneously available; resizing changes presentation only.
- The active bounded snapshot is a D3 force-directed node/link canvas. Pan,
  zoom, drag, selection, and **Fit** change presentation only.
- One selected indexed graph/API root can drive a bounded `executionFlow` query.
  A mapped runtime card may appear in the same view, but process-reported source
  correlation remains inferred unless the backend supplied stronger evidence.
- Backend graph search can replace the active snapshot with a bounded depth-two
  neighborhood. Empty search restores the controller's normal projection.
- Selection reveals the trace corridor and relation labels without changing
  stored topology or upgrading declared/resolved/inferred evidence to observed.
  The same relationships populate an `aria-live` incoming/outgoing summary.
- Repository/Data cards describe indexed code boundaries. They do not prove a
  live row, SQL statement, or fetched payload. An accepted `database` trace can
  show only the process-reported boundary/status/duration; the protocol accepts
  no SQL parameters, bodies, arbitrary attributes, or user data.
- Selecting a node synchronizes the evidence inspector and exact source range.
  The force simulation and structural-community annotation never upgrade
  declared/resolved/inferred evidence to observed.
- **Export current graph view** serializes only this active bounded snapshot as
  `graph.json`, `GRAPH_REPORT.md`, and a standalone offline `graph.html`. It is
  not a complete-index export; its truncation state and potentially sensitive
  labels, metadata, and relative paths remain visible.

### API inventory
- Rust searches the complete indexed HTTP-operation projection and returns an
  exact filtered `total`, exact unfiltered `indexedTotal`, and stable opaque
  keyset pages capped at 200 rows. The renderer follows pages to completion
  within its explicit 10,000-operation safety ceiling.
- OpenAPI 3.x/Swagger 2 JSON or YAML, Rust Utoipa, and static TypeScript/
  JavaScript server/client facts merge only by service, protocol, method, and
  normalized path. The catalog recognizes CONNECT evidence; the separate runtime
  sender still rejects CONNECT. Coverage counts remain distinct.
- Server-side filtering covers the full projection. The center workbench groups
  results by role, service or source, and route branch, with exact indexed source
  opening for each operation.
- OpenAPI descriptions, examples, schemas, security fields, bodies, server
  defaults, and remote references do not enter the catalog. External HTTP(S)
  targets are accepted only after credentials, query, and fragment are removed.
- Existing databases need one manual **Scan** to replace legacy unknown-role facts.

### Explorer inventory and search

- The normal tree renders only the first 5,000 deterministic indexed paths, with a status message that states the visible and total counts; it does not pretend the initial window is the entire workspace.
- A single `Map` keyed by relative path builds that bounded tree without repeatedly scanning sibling arrays. The tree uses one roving tab stop, falling back from a hidden focus/active path to the first visible item.
- After a non-empty query is debounced, Rust searches every indexed path with case-insensitive substring matching. Renderer input removes control characters and stops at 256 characters; Rust independently rejects invalid input and caps results at 200.
- Until the complete-index response arrives, the tree may show matches from the loaded window and announces that the full index is still being searched. Search and workspace generations discard late success/error results, and the live status distinguishes initial, searching, capped, empty, and failed states.

### Workspace code search

- **Shift+Command+F** on macOS or **Shift+Control+F** elsewhere selects the lazy Search activity and focuses its input. React waits 170 ms and binds each request to the current workspace ID plus workspace/request generations.
- SQLite contentless FTS5 trigram tables provide source document-key and selective AST-fact candidates for queries of at least three characters. One- and two-character queries use a deterministic fallback capped at 4,000 source files because a trigram index cannot represent them.
- Candidate ceilings are 2,000 source files, 200 paths, and 1,000 semantic nodes. Rust re-opens candidates through the scanner's canonical containment/no-symlink path and accepts source only when its BLAKE3 hash matches the committed index row; the contentless projection is never treated as current source.
- Exact literal verification is ASCII case-insensitive and non-ASCII case-exact. The response distinguishes `path`, `content`, `symbol`, `endpoint`, and `event`, uses Monaco-compatible 1-based UTF-16/end-exclusive source ranges, and caps output at 500 matches, 50 occurrences per file, 64 MiB of verified reads, and 750 ms. Reaching any candidate or execution ceiling sets `truncated`.
- File groups and match rows form one roving keyboard tree. Arrow, Home, End, Left, Right, and Enter operate the visible result set, and activation reuses the normal Monaco source-opening path.

### Static event semantics

- TypeScript/JavaScript extraction recognizes exact literal names for `addEventListener`, `on`, `once`, `addListener`, and Tauri-style `listen` subscriptions plus `emit`, `dispatch`, `publish`, and `dispatchEvent(new Event|CustomEvent(...))` publications.
- Each inferred `event` node records its exact literal range, API family, subscription/publication direction, operation, `staticEventName`, and confidence. Its owning code node receives a `listensTo` or `emits` edge. Dynamic, interpolated, escaped, control-containing, oversized, or handlerless names stay ordinary inferred call targets, so static analysis is not presented as observed runtime behavior.

### Source editor and formatting

- Monaco opens multiple safe indexed UTF-8 files in workbench tabs. Dirty buffers remain renderer state until the engineer saves or closes them; closing a dirty tab requires confirmation, and cancelling that confirmation keeps the final source tab/view open.
- Folder opening is refused while save/format is active, so a mutation can finish and update the saved baseline/hash before leave consent is considered. If the engineer then confirms opening another folder, existing documents become read-only immediately and cannot save, format, close, or accept a new source read until the picker/scan/load completes or cancels. The same `openingWorkspace` interval blocks Git mutations and configuration create/open, preventing the old workspace from changing after leave consent.
- Both save and format carry the backend-issued workspace ID from the read that produced the buffer; Rust rejects an empty, stale, or different ID, and the renderer also ignores completions from a superseded workspace generation. Rust accepts only an existing indexed relative path, at most 2 MiB, with its last-read BLAKE3 content hash. While holding the analysis reservation it analyzes the proposed text before disk mutation, writes and fsyncs a same-directory `create_new` temporary file, then revalidates the original identity/hash. A current-workspace read lock spans the final check and atomic rename, so installing another workspace cannot cross the save commit section.
- Rename is the save commit point. Any earlier failure returns an error with the old target intact. After rename, the returned `SourceFile` always contains the committed request text and new BLAKE3 hash. Parent-directory fsync, immediate transactional GraphStore replacement, or summary-refresh failure is logged without source text and does not turn the committed save into a false IPC failure; the watcher or **Rescan Workspace** recovers derived state.
- Async UI results are content-aware: typing during a save preserves the newer buffer as dirty over the returned committed baseline, and a formatter result is applied only if the buffer still matches the submitted text. A fresh source read never overwrites an already dirty tab.
- Display word wrap has `off`, viewport, and bounded modes. The configured wrap column is also passed as a print-width hint to compatible formatters, but display wrapping alone never modifies source.
- The always-available Aone normalizer changes only line endings, trailing horizontal whitespace, and final-newline shape. Its static catalog reads no `PATH`; one native approval permits bounded allowlisted discovery and a second binds the resolved executable, fixed arguments, and target before execution. Missing or rejected external tools leave the built-in result available.
- `dark-modern`, labeled **VS Code Dark Modern**, is the immutable default. Light Modern, High Contrast, Cursor Dark, Tokyo Night, and Catppuccin Mocha share one typed semantic-token registry that supplies workbench CSS, Monaco, and xterm colors. Font/layout/terminal preferences are versioned, bounded, non-secret local settings. No VS Code extension host, settings-file compatibility layer, or LSP is implied.
- Settings is an `aria-modal` dialog. It moves focus to Close, loops Tab/Shift+Tab between its first and last controls, restores the connected invoking element, closes on Escape, and uses a capture-phase handler to prevent Cmd/Ctrl+K, R, 1, 2, Enter and Shift+F5 from reaching workbench commands while the modal is open.

### Integrated terminal authority and backpressure

- Rust publishes only detected, canonical shell profiles. React selects an opaque profile ID and dimensions; it cannot provide an executable, argument list, working directory, environment, or shell command.
- One reservation spans native consent and the live PTY. The shell and workspace identities are checked again after consent. The child starts at the canonical workspace root with a cleared, small usability environment that excludes loaded `.env` values, credentials, and loader hooks.
- Unix terminal launch sets `O_NONBLOCK` on the PTY master before portable-pty clones its reader and writer descriptors; non-Unix launch fails explicitly until interruptible PTY I/O exists. Input is capped at 64 KiB per call and handed to a dedicated writer through non-blocking enqueue. Its budget includes the in-progress write and permits at most 64 outstanding messages or 256 KiB; saturation returns a retryable error while a closed/finished/missing session or disconnected writer returns `{ accepted: false }`. Partial-write/flush, read, and full-output-queue retry loops check the shared close signal at most every 5 ms. Dimensions are clamped. The reader emits at most 16 KiB per data event through a 32-chunk synchronous queue and spaces data events by at least 16 ms (about 62.5/s). Backpressure preserves byte order instead of dropping/reordering arbitrary bytes inside ANSI sequences. Arbitrary output is base64 in IPC and decoded losslessly by xterm.js.
- React subscribes before enabling **New**. While native consent/open is in flight it retains the newest 32 early events, binds only events for the returned session ID, and prints a visible omission count if that small pre-open buffer overflows. Rejected write/resize IPC promises become visible terminal errors. An `{ accepted: false }` write or resize triggers a best-effort native close before the renderer clears that stale session and reports it inactive; teardown removes the listener/observer/xterm instance and closes an active session.
- A writer failure shuts down the registered session. A reader failure first queues the typed terminal error, then shuts it down; a child-wait failure also closes it. Natural EOF retains the child-driven completion path. Explicit close, application teardown, and state drop atomically remove the session, signal HUP, invoke the child killer, send SIGKILL to the original Unix process group, and close master handles without joining workers. A real Unix PTY regression deliberately keeps its slave open and unread, then proves the non-blocking cloned reader/writer descriptors observe cancellation and are dropped within the test deadline.
- A workspace switch is refused while the terminal is active. Terminal bytes are not stored or promoted to graph/AI evidence.

### Local Git authority

- The workspace must be the exact Git worktree root with an in-place non-symlink `.git` directory. Linked-worktree `.git` files, `commondir`, symlinked object directories, and local/HTTP object alternates are rejected so a selected folder cannot redirect inspection into unrelated metadata. Read-only status and bounded patch diff use fixed `/usr/bin/git`, literal pathspecs, argv only, and `--no-renames`. Each inspection copies the bounded no-follow index into a private temporary Git directory, writes a detached HEAD plus empty attribute tree, pins the validated original object database only as an alternate, and supplies no repository/global/system configuration. It disables replacements, lazy fetch, external diff/textconv, fsmonitor, hooks, credentials, and submodule traversal, so merely opening Source Control cannot execute a configured helper. Diff requires one validated workspace-relative file; scanner-hard-denied paths, directories, symlinks, escapes, and unsafe display characters are rejected.
- Initialization, staging, unstaging, and commit are serialized and each requires native consent. Paths remain workspace-relative, bounded, non-sensitive, and non-symlinked. Commit snapshots the validated path list between two matching raw-index reads, hashes the bounded `git diff --cached --raw -z --full-index --no-renames` bytes with BLAKE3, and repeats the same snapshot after consent; the commit is rejected when either paths or staged content changed, even when filenames stayed identical.
- Hooks, prompts, pagers, fsmonitor, GPG signing, and remote operations are disabled in Source Control. It exposes no push, pull, fetch, remote clone, checkout, reset, discard, file deletion, or arbitrary Git arguments; the separate bootstrap clone accepts only credential-free public GitHub HTTPS into a new Documents child.
- A file with both index and working-tree changes has separate staged and unstaged rows/diffs. Workspace/request generation guards clear state and ignore late status, diff, or mutation completions after the active workspace or newer request changes.
- A separate native-approved Git onboarding inspection parses bounded regular Git configuration/HEAD files and up to 16 regular non-symlink `.pub` files. It strips unquoted inline `#`/`;` configuration comments, returns sanitized identity labels/remotes and public-key fingerprints, never opens or stats private keys, and does not execute Git/SSH/GitHub CLI, contact a network, or mutate forks, branches, remotes, or pushes. The UI keeps those development steps as a manual checklist.

### AI-tool configuration hub

- Rust owns eleven fixed home/workspace locations: Codex/Claude Desktop/Cursor/Gemini/VS Code MCP or CLI configuration plus `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, a Cursor project rule, Copilot instructions, and a portable `.agents/skills/` project skill.
- The initial catalog is pure static data with `notInspected` states. Only **Inspect AI tools** triggers native permission for bounded file/application/PATH metadata checks; no content is read and no executable is run. React receives labels, company, kind, scope, path hint, and three-state file/CLI status only.
- Before consent, Rust attests the canonical trusted root, the filesystem identity of every existing parent directory, and any existing target file. After consent it confirms that workspace/path and complete identity chain are unchanged; a final adjacent revalidation ensures fixed `/usr/bin/open -t` receives only the approved canonical regular file.
- A missing target is created only when requested and approved, using private `0700` parent directories, an empty documented root template, atomic create-without-overwrite behavior, symlink rejection, and `0600` file permissions. A file that appears during consent is rejected for review instead of silently opened or overwritten.
- Renderer refresh/open operations are generation-guarded so a late result from another workspace cannot repopulate or clear the current panel.
- This is a configuration launcher, not an MCP client, server manager, agent runtime, credential vault, or compatibility guarantee for future provider schema changes.

### Project Agent and runtime discovery

- Project Agent may read the in-memory report already approved for the current workspace when its panel mounts; that restore performs no discovery, file/PATH read, process execution, or consent dialog. One global reservation covers an explicit new inspection's first native approval and discovery. Before that approval, Rust reads no language summary, run-profile marker, build/config content, PATH/home-tool directory, or executable path.
- The fixed catalog contains 39 runtime/build tools. Discovery admits at most 64 inherited absolute PATH entries and 128 total inherited/standard/home-relative directories, keeps four canonical regular executable candidates per tool, and rejects workspace-contained paths. After consent it also reads a fixed allowlist of already-indexed build/config sources (24 files maximum, 64 KiB each, 512 KiB total) through the scanner's containment, symlink, and hard-deny checks. Only recognized environment names and controlled PostgreSQL/Redis dependency hints survive; values do not, and the UI labels every result as an inference rather than a required-service/readiness claim. Dedicated env files and known credential paths remain excluded. Discovery does not launch a shell or evaluate startup files.
- A second native dialog shows only relevant primary canonical paths and their fixed version arguments. Approval may execute at most 16 probes with concurrency four, six-second timeouts, a clean environment, `/` working directory, filesystem-identity revalidation, bounded 12 KiB stdout/stderr drains and 240-byte printable version lines, and process-group cleanup. Decline returns the same paths as `unverified`; omitted probes do not run.
- The report always includes all 39 tools, evidence-backed stack confidence, and deterministic run-profile/tool/environment recommendations. A detected stack without a supported run profile is a warning. `Dockerfile` confirms container packaging evidence, but never becomes an executable profile; without a detected Compose manifest the report explicitly says that no Compose run profile exists in the opened root. **Use run profile** selects an existing Rust-detected profile; it does not synthesize argv, write configuration, install a runtime, or execute automatically.

### Execution paths

v0.1 exposes bounded neighborhood expansion, not a shortest-path product API. Weighted execution paths that distinguish inferred, declared, resolved, and observed cost remain future work.

### Local process authority

- Package-manifest discovery reads at most 1 MiB per `package.json`; malformed or oversized manifests are skipped independently.
- A start request must name a profile currently registered by Rust. It carries no renderer-provided executable or argument fields; Rust resolves both from that registered profile.
- Rust resolves the backend-owned profile source, executable target, argument vector, and working directory, then builds a cleared and allowlisted environment. The explicit Run/Debug action authorizes the launch without a second native dialog; Rust revalidates each filesystem identity immediately before spawn. Runtime audit metadata reveals selected environment names, never values.
- A reservation covers both start preparation and execution, so only one process may be starting or running. `stop_run` terminates the process group.

### API request authority

- API Explorer accepts HTTP(S) and JSON request bodies only. URL, header count/bytes, body, timeout, response, and emitted evidence are bounded.
- Rust grants explicit-Send authorization when the URL host is an IP-literal `DestinationScope::Loopback`, including IPv4 `127.0.0.0/8` and IPv6 `::1`; no IDE-managed run is required. `localhost`, every other DNS name, and all public/private non-loopback IP literals retain a Rust-owned native confirmation that times out closed after 120 seconds and occurs before DNS or other network activity.
- After authorization Rust resolves all addresses; shared `network_policy` rejects unspecified, link-local, multicast, broadcast, metadata, reserved, deprecated, and other special-use destinations. Confirmed literal private non-loopback targets receive a conspicuous warning; IP-literal loopback proceeds directly from the engineer's explicit Send action.
- Validated DNS results are pinned into a fresh direct client for that request. Ambient proxies and redirects are disabled, preventing the validated destination from changing after consent.
- When required, the confirmation shows method, canonical origin, and query names with values hidden. Headers and body values are not displayed.
- Runtime request/response evidence retains only method, a URL with every query value hidden, status/duration, header names, body byte counts, and truncation; no header or body value enters runtime history.

### WebSocket authority and backpressure

- WebSocket uses the shared destination policy, direct validated sockets, native
  per-connect consent, bounded/redacted previews, eight session permits and
  eight-item outbound queues. Message rate overflow becomes an explicit dropped
  summary. Exact byte/time ceilings are centralized in [IPC contract](IPC_CONTRACT.md).

### AI request authority

- AI is optional; code-only startup retains workspace, index, search, graph,
  source-navigation, and export behavior.
- An explicit selected-node explanation builds a deterministic undirected
  two-hop corridor from the active bounded snapshot, selected node first, with
  at most 16 node IDs. Rust includes those nodes and eligible internal edges in
  a provider-safe pack capped at 24 items and 32 KiB.
- Rust owns the fixed task, rechecks workspace authority before evidence
  selection, after native consent, and after response, and permits only one
  pending or active call. Every explanation has separate consent.
- AI discourse relationships remain inferred unless supplied evidence already
  declares or observes them. Project Agent chat accepts one bounded untrusted
  engineer question at a time, binds it to the retained report, redacts it, and
  obtains separate provider consent; keys stay backend-only, redirects and
  ambient proxies are disabled.

### Cycle analysis

- Kosaraju SCC through `petgraph` runs in `O(V + E)` over a bounded projection.
  It annotates import, dependency, recursive-call, and debugger-layout cycles;
  v0.1 has no separate cycle-analysis screen.

### Runtime stream

- A 2,000-item `VecDeque` retains ordered session events. Stdout and stderr share
  a 50 events/s gate; React commits 100 ms batches and keeps the newest 500.
  Ordinary output stays timeline-only. **Observe** admits strict nonce-bound
  `AONE_TRACE_V1` spans. **Debug** reserves child stdin and admits strict
  `AONE_DEBUG_V1` cooperative safe points into a bounded session registry.
  Static source evidence is never upgraded by process-reported correlation.
  Browser activity and uninstrumented internals remain unobserved. Full debugger
  authority and bounds are in [Instrumented execution debugger](INSTRUMENTED_DEBUGGER.md).

WebSocket events use a separate ephemeral channel, frame-batched 300-event tail,
and 120-event transcript; they never enter SQLite, structural graph, or AI evidence.

See [Performance and data-structure policy](PERFORMANCE.md) for measured boundaries.

## Language capability

Each language reports `textOnly`, `syntaxOnly`, or `semantic`. Runtime evidence
is separate and never implies compiler-level coverage. Call-site extraction is
not compiler-resolved dynamic dispatch; future LSP/compiler adapters can enrich
the same graph contract.

## Size strategy

The source configures a 300 MiB DMG size gate, never padding or a cache promise.
Aone uses WKWebView plus native SQLite/Tree-sitter, lazy Monaco/xterm, excluded
source maps, and size-optimized Rust; it bundles no server or toolchain. That
configuration is not proof of a built, signed, notarized, or installed package.
