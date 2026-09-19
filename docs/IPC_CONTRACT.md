# Aone v0.1 IPC contract

The WebView is an unprivileged renderer. Rust owns filesystem, process, secret, database, model-provider, and network access.

All serialized fields use `camelCase`. Commands accept a single object when they need more than one argument.

## Machine-readable contract

`openapi/aone-ipc.openapi.yaml` documents this command surface as OpenAPI 3.1. Each operation has an
`x-aone-tauri-command` extension containing the exact `invoke` name. The documented POST paths and
the `https://tauri.invalid/ipc` server are non-routable documentation surrogates; Aone IPC is **not**
an HTTP server. Its 56 operations correspond to the 56 registered commands below. React calls
Tauri's `invoke`, and Rust remains the privileged implementation.

The official OpenAPI Generator 7.24.0 `typescript-fetch` model generator runs from a Docker image
pinned by digest. Generated models are compiled by the project's fixed TypeScript 7.0.2 compiler and
adapted through `src/contracts/`. This yields one reviewed payload contract without replacing the
Tauri bridge with a generated network client. The current checked contract digest is
`222fa6f4347e1606238c94288cef66ac7e05386b5f68781e61527b602d9e7a21`.

```bash
npm run openapi:check       # no Docker daemon required
npm run openapi:generate    # Docker required; replaces generated output atomically
npm run openapi:docs        # Docker; swaggerapi/swagger-ui:v5.32.11 at 127.0.0.1:8090
```

Swagger UI has submit methods disabled because its value here is schema and operation discovery, not
sending fake HTTP requests. Set `AONE_SWAGGER_PORT` to choose another local port. The drift check
compares contract hashes and verifies exact parity with Tauri's registered command list.

## Commands

| Command | Input | Output |
| --- | --- | --- |
| `get_app_snapshot` | none | `AppSnapshot` |
| `pick_and_open_workspace` | none; Rust opens the native folder picker | `WorkspaceSummary?` (`null` when cancelled) |
| `pick_and_open_file` | none; Rust opens the native file picker and indexes its containing directory | `OpenFileResult?` with the workspace plus relative file path |
| `pick_and_create_workspace_file` | none; Rust opens a native save picker inside the active workspace | new empty indexed `SourceFile?`; cancel returns null and existing files are never overwritten |
| `clone_github_repository` | `{ request: { repositoryUrl, destinationName } }`; credential-free public `https://github.com/<owner>/<repository>` plus matching safe child name | `ProjectBootstrapResult` after native destination/network confirmation and shared scan/index installation |
| `create_documents_project` | `{ request: { projectName, initializeGit } }`; safe single child name only | `ProjectBootstrapResult` after native confirmation, private README creation, optional isolated `git init -b main`, and shared scan/index installation |
| `inspect_git_onboarding` | `{ request: { workspaceId } }`; current opaque workspace ID | sanitized `GitOnboardingReport?` after native metadata permission; `null` when declined |
| `rescan_workspace` | none | `WorkspaceSummary` |
| `get_workspace_files` | `{ query?, limit? }`; optional query is a control-free path substring of at most 256 characters | path-ordered `WorkspaceFile[]`; unfiltered list capped at 5,000, complete-index search capped at 200 |
| `search_workspace` | `{ request: { workspaceId, query, limit } }`; current opaque workspace ID, 1-256 control-free literal characters, and limit 1-500 | `WorkspaceSearchResult` containing exact keyed `path`/`content`/`symbol`/`endpoint`/`event` matches, 1-based UTF-16/end-exclusive ranges, indexed-file count, and truncation state |
| `read_workspace_file` | `{ relativePath }` | `SourceFile` |
| `write_workspace_file` | `{ request: { workspaceId, relativePath, content, expectedContentHash } }` | committed `SourceFile` and new BLAKE3 hash; derived reindex is attempted immediately and recoverable through watcher/rescan |
| `get_formatter_capabilities` | none | static `FormatterCapability[]`; built-in normalizer available, external allowlisted tools uninspected until format-time discovery consent |
| `format_document` | `{ request: { workspaceId, relativePath, content, expectedContentHash, tabSize, insertSpaces, printWidth } }` | in-memory `FormatDocumentResult`; external tools require native confirmation |
| `query_graph` | `{ query: GraphQuery }`; `projection` is `neighborhood` (default), `systemOverview`, or `executionFlow`; neighborhood accepts bounded label/path text plus node/edge-kind filters, while flow requires one `rootId` and accepts opaque `cursor` plus `includeTests` | bounded `GraphSnapshot`; every returned node has snapshot-relative structural-community metadata, overview adds presentation-only system-layer metadata, and flow pages add cursor/count/flow metadata |
| `list_api_endpoints` | `{ request: { workspaceId, query?, cursor?, limit? } }`; current opaque workspace ID, at most 128 query characters, opaque keyset cursor, limit 1-200 | `ApiEndpointPage` with exact filtered `total`, exact unfiltered `indexedTotal`, stable next cursor, role/coverage counts, and indexed source evidence |
| `detect_run_profiles` | none | `RunProfile[]` |
| `inspect_project_environment` | `{ request: { workspaceId } }`; Rust owns both native approvals | workspace/report-bound `ProjectEnvironmentReport?`; `null` when first consent is declined |
| `get_ai_configuration_status` | none | renderer-safe `AiConfigurationStatus` without credentials |
| `configure_hosted_ai` | `{ request: { provider, apiKey, model? } }`; typed key briefly traverses masked WebView/IPC | session-only hosted status; key is never serialized back |
| `configure_ollama` | `{ request: { endpoint, model } }`; loopback HTTP only | status after bounded `/api/tags` reachability and exact-model verification |
| `list_ai_cli_adapters` | none; explicit action runs fixed bounded Codex version/authentication probes only | safe `AiCliAdapterStatus[]`; Claude/Copilot remain unsupported |
| `configure_ai_cli` | `{ request: { adapter: "codex" } }` | status after the audited Codex `0.144.6` executable and authentication are reverified |
| `pick_and_load_env_file` | none; Rust opens the AI-provider env dialog and retains only `AONE_AI_PROVIDER`, OpenAI key/model, or Anthropic key/model fields | names-only `EnvLoadResult?` |
| `pick_and_load_run_env_file` | none; Rust opens the run-process env dialog and filters provider fields | `EnvLoadResult?` |
| `start_run` | `{ request: StartRunRequest }`; exact backend-registered profile ID, selected run-env names, and required `observe`/`debug` booleans; debug requires observe | `{ runId }` after immediate identity revalidation; the explicit Run/Debug action is authorization, Observe injects nonce-bound trace variables, and Debug also binds a cooperative safe-point/control session |
| `stop_run` | `{ request: { runId } }` | `{ stopped }` |
| `list_runtime_events` | `{ request: { limit? } }` | `RuntimeEvent[]` |
| `start_otlp_receiver` | `{ request: { preferredPort? } }`; port 0 requests a system-selected port | `OtlpReceiverSnapshot` after native consent and loopback bind |
| `get_otlp_receiver` | none | current workspace-scoped receiver lifecycle, endpoint, ephemeral token/configuration fields, counts, and limitation |
| `stop_otlp_receiver` | none | stopped `OtlpReceiverSnapshot`; closes the listener and invalidates the token |
| `delete_runtime_trace` | `{ request: { traceId } }` | count of current in-memory runtime events removed for that exact trace |
| `get_debug_session` | `{ request: { runId } }` | bounded current or retained `DebugSessionSnapshot` for that managed run |
| `control_debug_session` | `{ request: { runId, debugSessionId, action, expectedSequence, expectedControlEpoch } }` | `{ accepted, snapshot }`; Pause/Continue/Step are cooperative, while Stop invokes native process-group authority |
| `send_api_request` | `{ request: ApiRequest }` | `ApiResponse` after destination validation; explicit Send authorizes IP-literal loopback without a managed-run prerequisite, otherwise bounded native confirmation is required |
| `connect_websocket` | `{ request: { url, headers, protocols, timeoutMs? } }` | `{ sessionId, protocol? }` after destination validation and native confirmation |
| `send_websocket_message` | `{ request: { sessionId, data, encoding } }`; encoding is `text` or `base64` | `{ accepted }` |
| `disconnect_websocket` | `{ request: { sessionId } }` | `{ disconnected }` |
| `ai_explain` | `{ request: { workspaceId, question, nodeIds, runtimeEventIds: [] } }`; `question` is compatibility-only, non-empty runtime event IDs are rejected, and the product selects at most 16 nodes from a deterministic two-hop active-snapshot corridor | workspace-bound `AiExplanation` over selected nodes and eligible internal edges after per-call native confirmation |
| `ai_explain_project_environment` | `{ request: { workspaceId, reportId } }`; Rust resolves the retained report and fixed task | `AiExplanation` after separate native provider confirmation |
| `list_terminal_profiles` | none | backend-detected `TerminalProfile[]` |
| `open_terminal` | `{ request: { profileId, columns, rows } }` | `TerminalOpenResult` after native confirmation |
| `write_terminal` | `{ request: { sessionId, data, encoding } }`; encoding is `text` or `base64` | `{ accepted }` |
| `resize_terminal` | `{ request: { sessionId, columns, rows } }` | `{ accepted }` |
| `close_terminal` | `{ request: { sessionId } }` | `{ accepted }` |
| `get_git_status` | none | `GitStatusResult` for the exact-root local repository |
| `get_git_diff` | `{ request: { relativePath, staged } }`; `relativePath` is one required validated non-sensitive file | bounded patch plus optional 2 MiB previous/current UTF-8 comparison sides |
| `git_stage` | `{ request: { paths } }` | `GitMutationResult` after native confirmation |
| `git_unstage` | `{ request: { paths } }` | `GitMutationResult` after native confirmation |
| `git_commit` | `{ request: { message } }` | local `GitMutationResult` after native confirmation |
| `git_initialize` | none | local `GitMutationResult` after native confirmation |
| `list_tool_configurations` | none | eleven static `ToolConfiguration[]` descriptors with `notInspected` states; performs no machine lookup |
| `inspect_tool_configurations` | none; Rust opens native inspection consent | eleven metadata-only descriptors with three-state file/CLI status |
| `open_tool_configuration` | `{ request: { toolId, createIfMissing } }` | `ToolConfigurationOpenResult` after native confirmation; never file contents |

The authority-bearing actions are intentionally non-batchable in v0.1:

- An explicit **Run** or **Debug** action authorizes one local process start for the selected backend-registered profile; Aone does not open a redundant native process dialog. `StartRunRequest` carries only the opaque profile ID, selected environment names, and run-mode flags, so the renderer cannot supply executable text or arguments. Rust immediately revalidates the profile source, launch target, and working-directory identities, starts from an empty environment plus its safe allowlist and selected names, never returns environment values, and permits only one starting or running process.
- Clone/Create accept no absolute destination. After the engineer chooses an action, Rust resolves and attests Documents to name the exact target in native consent. The proposed direct child is not probed and no filesystem mutation or network activity occurs before approval. Both commands serialize with workspace operations, refuse overwrite and identity/symlink changes, and install the result through the standard scanner/indexer. Clone uses fixed credential-free `/usr/bin/git` arguments for a shallow single-branch ordinary tracked snapshot; submodules and Git LFS downloads remain disabled.
- `inspect_git_onboarding` is a separate permission and workspace-generation boundary. It reads bounded regular Git metadata and up to 16 regular non-symlink public `.pub` files, strips unquoted inline `#`/`;` configuration comments, and returns only sanitized identity labels/remotes and fingerprints. It never opens or stats private keys, executes Git/SSH/GitHub CLI, contacts a network, or mutates a fork, branch, remote, or push.
- Workspace Search accepts no root or absolute file path. Rust requires the current opaque workspace ID, obtains bounded file/node document-key candidates from parameterized contentless FTS5 tables, then re-opens source through canonical containment/no-symlink checks and requires its committed BLAKE3 hash before returning an exact preview or range. Reaching a candidate, result, per-file, byte, or time budget is visible as `truncated`.
- `systemOverview` accepts the same optional graph text/kind filters and explicit
  root IDs, but its node and edge ceilings cannot be raised above 60 and 120.
  With no roots, Rust selects fixed diverse quotas across six layers and expands
  to depth 2 by default. After that response, the application sorts its node
  IDs, takes at most 50, and requests a rooted `neighborhood` at depth 4 / limit
  500 for Dependencies only. Both responses must pass the same
  workspace-generation commit guard. Architecture retains the curated overview.
  The anchored dependency result may
  contain broader raw/inferred/test evidence and is not an exact architecture
  claim. The overview returns only existing stored nodes/edges, preserves their
  provenance, and reports omitted matches through `truncated`.
- Relationship search submits a control-free label/path query of at most 256
  characters as a depth-two `neighborhood` capped at 500 nodes. Node-kind and
  edge-kind filter lists each contain at most 32 values of at most 64
  control-free characters. Empty search restores the normal controller view.
- Each bounded response is deterministically annotated using returned topology.
  Returned edges are treated as undirected/deduplicated, dangling edges are
  ignored, and isolates are singletons. `communityComplete` is false whenever
  the snapshot is truncated; communities are structural, not semantic claims.
- `executionFlow` requires one existing root and refuses text/kind filters. It
  is bounded to depth four and 500 nodes / 2,000 edges per response. The root
  contributes at most 100 links per cursor page, each descendant contributes at
  most 40, and traversal stops after 500 expansions or 20,000 scanned links.
  Its cursor is bound to root/depth/limit/test-inclusion values. Root totals and
  omissions are cumulative and explicit; scanner-hard-denied paths never enter
  root/child counts, tests are excluded in product requests, and reaching a
  bound sets `truncated`. System and Runtime use this projection. API + Data's
  **Trace flow** roots it from the selected canonical operation ID, with an
  exact handler fallback only when available.
- `list_api_endpoints` is independent of graph-query limits. Rust performs the
  query against the complete indexed HTTP projection, merges duplicates only by
  service/protocol/method/path, and returns deterministic pages of at most 200.
  Server-side search covers the full inventory rather than the loaded page.
  OpenAPI/Swagger JSON or YAML, Rust Utoipa, and static TypeScript/JavaScript
  producer/consumer evidence remain distinguishable. Legacy direction is
  `unknown` until one manual workspace Scan refreshes existing databases.
- An explicit API **Send** authorizes an IP-literal loopback target, including IPv4 `127.0.0.0/8` and IPv6 `::1` under the shared classifier, without requiring an IDE-managed run. This exception performs no DNS lookup. Every other target—including `localhost`, all other DNS names, and public/private non-loopback IP literals—opens a native confirmation with method, canonical origin, and a sanitized URL. Header/body values are hidden, confirmed literal private targets receive an additional warning, and an unanswered confirmation fails closed after 120 seconds.
- Every WebSocket connect opens a native confirmation for the validated destination. It shows the canonical scheme, host, explicit port, path, sorted query-parameter names, and header names; query/header values and message data are not shown. Later sends are restricted to the approved session ID rather than opening arbitrary destinations.
- Every AI call opens a native confirmation with model, evidence-item count/bytes, and the applicable backend-owned task. Evidence contents, Project Agent question text, and credentials are hidden. Only one AI call can be pending or active. A selected-node request uses the renderer's deterministic undirected two-hop corridor (selected node first, at most 16 nodes); Rust rebuilds a provider-safe pack of those nodes and eligible edges, capped at 24 items and 32 KiB. AI discourse relationships are inferred unless supplied evidence already declares or observes them. Project Agent questions are user-authored untrusted data bounded to 1,200 characters/4 KiB, redacted, and bound to a retained report; evidence excludes executable paths, source/configuration contents, arguments, probe errors, and environment values.
- `inspect_project_environment` has two different authority gates. The first occurs before language/manifest/build-config/PATH/home-tool/executable inspection and discloses the fixed names-only hint read: no more than 24 allowlisted already-indexed files, 64 KiB each and 512 KiB total, with scanner containment/hard-deny checks. Dedicated env/credential files remain excluded; values never enter the report, and dependency/environment results are explicitly inferred hints, not proven requirements or readiness. The second lists the canonical executable paths and fixed version arguments for at most 16 relevant probes; declining it returns `unverified` paths and executes nothing. No shell, installer, download, or workspace mutation is exposed.
- Formatter capabilities perform no automatic `PATH` lookup. External formatting first asks native permission for one bounded allowlisted discovery; when a tool resolves, a second confirmation names the fixed executable, arguments, and source path before execution. The source text is sent over bounded stdin and is not shown in either dialog. The in-process safe normalizer needs no process authority.
- Save/format increment a synchronous editor mutation counter before IPC; while nonzero, folder opening returns before discard consent or picker invocation. After dirty-document leave consent, the renderer sets one `openingWorkspace` guard before invoking the picker and retains it through selection, scan, and file/graph load. Old-workspace edit/save/format/close, new source-open, Git mutation, and tool-configuration create/open handlers refuse work until success or cancellation unwinds; backend workspace IDs and validation remain authoritative.
- Save and format bind to the backend-issued `workspaceId` associated with the read buffer. `write_workspace_file` analyzes and validates before mutation and holds the current-workspace read lock through the final check and atomic rename. Rename is the commit point: pre-rename failure returns an error and preserves the old target; after rename it returns the committed content/hash even if directory fsync or derived graph/summary refresh reports a post-commit problem. Such problems are logged without source text and recover through the watcher or **Rescan Workspace**.
- Every terminal open confirms the canonical workspace, detected shell profile, executable, and raw-I/O boundary. Later input/resize/close commands can address only the backend-issued single session.
- Terminal input is decoded to at most 64 KiB per call. A dedicated writer's non-blocking queue counts its in-progress write and permits at most 64 outstanding messages or 256 KiB. Saturation returns a retryable invalid-request error without closing the session; a closed/finished/missing session or disconnected writer returns `{ accepted: false }`.
- React treats `{ accepted: false }` from write or resize as a stale session: it requests best-effort `close_terminal`, clears the matching local session, and displays a lifecycle error even if that cleanup call also fails.
- On Unix the PTY master is `O_NONBLOCK` before reader/writer descriptor clones are created. Partial-write/flush, read, and full-output-queue retries check cancellation at most every 5 ms. Terminal output uses a 32-item synchronous queue of at most 16 KiB chunks; cancellation-aware backpressure preserves ANSI byte order. Writer failure, reader failure after error enqueue, and child-wait failure close the registry session. Shutdown sends HUP and then SIGKILL to the original process group. The renderer installs the event listener before enabling Open and holds at most 32 events that arrive before the session result, with an explicit visible omission count on overflow.
- Git status and per-file diff are read-only and require an in-place non-symlink `.git` directory; linked-worktree files, `commondir`, symlinked objects, and object alternates are rejected. Filesystem-only root/branch/HEAD validation enters a private bounded metadata/index snapshot before the first read-only Git process; empty attributes and no repository, global, or system configuration prevent configured filters, helpers, external diff/textconv, fsmonitor, replacements, lazy fetch, and credentials from executing. Diff uses `--no-renames` and rejects an omitted/repository-wide path, directories, symlinks, escapes, unsafe display characters, and scanner-hard-denied files. Initialize uses isolated configuration; stage, unstage, and commit each open a native confirmation, and commit consent binds the approved staged paths plus a bounded BLAKE3 fingerprint of raw staged-index content. No push, pull, fetch, remote mutation, or destructive Git operation is registered.
- The eleven-entry AI-tool catalog is static until `inspect_tool_configurations` receives native permission to check bounded metadata only. Opening or creating a known MCP/CLI/agent/skill/instruction/rule file is another native confirmation. Rust never reads contents into React, refuses symlinks, never overwrites an existing file, and creates only a minimal private template when explicitly requested. Canonical trusted-root, existing-parent, and target-file identities are captured before consent and revalidated afterward and immediately before fixed external-editor launch.

## Events

- `aone-scan-progress`: `{ phase, completed, total, currentPath? }`
- `aone-runtime-event`: `RuntimeEvent`
- `aone-workspace-changed`: `{ paths }`
- `aone-websocket-event`: `{ sessionId, kind, timestamp, destination, protocol?, data?, encoding?, byteLength?, droppedCount?, truncated, detail? }`, where `kind` is `connecting`, `open`, `message`, `dropped`, `error`, or `closed`, and `encoding` is `text` or `base64`. A `dropped` summary reports messages intentionally omitted by bounded backend event-rate control; it does not close the socket.
- `aone-terminal-event`: `{ sessionId, kind, timestamp, data?, encoding?, byteLength?, exitCode?, detail? }`, where `kind` is `data`, `exit`, or `error`; data is base64 and limited to one bounded PTY chunk.

`asyncapi/aone-events.asyncapi.json` documents all five backend-originated channels as AsyncAPI 3.1
and reuses OpenAPI payload schemas. It describes Tauri's in-process event transport, not a public
broker or a WebSocket endpoint into Aone. `npm run asyncapi:check` validates its channels against the
renderer listener surface.

Runtime events are in memory only. Rust retains at most 2,000. HTTP request events keep method, an all-query-values-hidden URL, header names, body bytes, and timeout; response events keep method, hidden-value URL, status, duration, header names, body bytes, and truncation. Header and body values never enter runtime history, and a request finishing after its initiating workspace changed cannot publish into the new workspace. Process stdout/stderr share one FIFO emission gate capped at 50 events per second; React queues arrivals and flushes them every 100 ms, causing at most ten timeline state commits per second while retaining the latest 500. Ordinary stdout/stderr remain timeline-only and do not change the graph projection. Workspace switches and application exit clear workspace runtime events.

With `observe: true`, native approval precedes injection of
`AONE_TRACE_PROTOCOL=AONE_TRACE_V1` and a fresh per-run nonce. Only that managed
child can submit matching JSONL stdout envelopes. Kinds are limited to
`http.server`, `http.client`, `function`, `database`, `external`, `agent`,
`event`, and `job`; phases are `start`, `end`, and `event`. Unknown fields,
arbitrary attributes, header/body/query values, SQL parameters, and payloads are
rejected. One run accepts at most 8,192 trace records and 4,096 unique spans.
Accepted records are observed runtime facts; valid parent links are observed
process-reported relationships. The reported source range can create only an
inferred candidate correlation to static code, and never changes a static
node's evidence. Ordinary browser refreshes and uninstrumented internals expose
no internal flow. A rejected envelope produces only a bounded
`trace.rejected` timeline diagnostic and is excluded from graph projection.

The separately consented local OTLP receiver binds only `127.0.0.1`, accepts
token-gated OTLP/HTTP JSON `POST /v1/traces`, and converts at most 1,024 spans
per export into the same observed runtime model. It retains only allowlisted
service/code/HTTP/database/messaging identifiers plus timing/status; arbitrary
attributes, span events, links, payloads, statements, and original span names
are not retained. One receiver session admits at most 8,192 unique spans. Its
ephemeral token is invalidated on stop or workspace change. Aone's API client
can inject a W3C `traceparent` only while that receiver is active and only after
the destination is validated as entirely loopback/private. Runtime trace
deletion affects the current bounded memory queue only. See
[Local observability graph](OBSERVABILITY_GRAPH.md).

With `debug: true`, `observe` must also be true. The explicit Debug action additionally
reserves child stdin, injects `AONE_DEBUG_PROTOCOL=AONE_DEBUG_V1`, a fresh nonce,
and a debugger session ID, then registers the run-scoped session before output
admission. Exact `AONE_DEBUG_V1 ` stdout JSON lines carry one serialized
workflow's request/method/line/branch/database/agent/external/response safe
points across Trigger/API/Backend/Service/Data/External/Response stages.
Sequence, control epoch, parent-before-child identity, source, state, and
shape-oriented previews are strict and bounded. The schema has no dedicated
value/body/header/query-value/raw-query/bind field, but semantic strings remain
process-reported and instrumentation authors must keep them data-free. One session retains 2,000 events and
returns its newest 250 per snapshot; running timeline emissions are sampled
10:1 while paused/terminal events are not. `AONE_DEBUG_CONTROL_V1 ` stdin records
support Pause, Continue, Step Into hint, Step Over hint, and cooperative Stop.
Pause binds atomically to Rust's current running sequence so a stale rendered
snapshot cannot make it unusable; Resume and Step require the exact paused
sequence. Debug Stop uses the existing process-group termination authority even
if the child protocol or control pipe is stale. See
[Instrumented execution debugger](INSTRUMENTED_DEBUGGER.md).

WebSocket events are a separate live session stream and are not persisted into SQLite or silently added to structural/AI evidence. React ingests them at most once per animation frame, retains the newest 300, and renders a 120-event selected-session transcript.
Terminal events are raw ephemeral PTY output. Rust does not retain them as runtime evidence, and React does not persist them or add them to graph/AI context.

## Evidence model

- `declared`: directly present in source or configuration.
- `resolved`: deterministically linked by analysis.
- `observed`: captured during a local run or API request.
- `inferred`: heuristic/static correlation or AI output.

`GraphNode` contains `id`, `kind`, `label`, optional source location, `language`, `evidence`, and metadata. `GraphEdge` contains `id`, `source`, `target`, `kind`, `evidence`, and optional confidence. Static JavaScript/TypeScript subscriptions and publications are inferred `event` nodes with exact literal ranges, event API/direction/operation metadata, and `listensTo`/`emits` owner edges. Dynamic, interpolated, escaped, control-containing, oversized, or handlerless names remain unresolved call targets. In a system overview, `systemLayerEvidence: exact|inferred` describes only why the node was placed in a display lane; in an execution flow, `flowStage`, `flowGroup`, candidate, cycle, and child-count metadata are likewise presentation/progressive-disclosure facts. None replaces `GraphNode.evidence`. Repository/model/storage path placement and conventional entry/job placement remain inference. An absolute non-local HTTP(S) label can place a request in External, but the target is unverified and its provider/cloud identity is unknown; requests without that evidence stay in Interfaces. The renderer must never display an inferred relation or placement as observed.

Markdown/MDX/TXT/RST/AsciiDoc extraction adds exact `heading` and `sentence`
nodes with declared `contains` and `precedes` edges, no confidence, and one-based
UTF-16/end-exclusive ranges. File metadata records `parser=aone-document`,
`parserVersion=1`, `documentFormat` (`markdown`, `plainText`,
`reStructuredText`, or `asciiDoc`), `documentLinesVisited`,
`documentSegmentsConsidered`, `factsExtracted`, `analysisTruncated`, and optional
`analysisTruncationReasons` (`document line limit reached` or
`document fact limit reached`). Segment metadata records `contentHash`,
`documentOrder`, and `labelTruncated`; headings add `headingLevel` and
`headingStyle`.

Every returned node also receives `communityId`, `communitySize`,
`communityAlgorithm`, `communityBasis`, and `communityComplete` for its bounded
snapshot. Those fields do not replace evidence or assert semantic meaning.

Graph artifacts are renderer-created downloads, not IPC commands. `graph.json`,
`GRAPH_REPORT.md`, and `graph.html` serialize only the active bounded snapshot,
including its truncation and community basis; they are not complete-index
exports. Binary PDFs, images, audio, video, and other media are not parsed or
automatically uploaded.

## Security invariants

- Workspace authority originates from native Open Folder or the bounded Clone/Create bootstrap commands; the renderer cannot provide an absolute folder path.
- Canonicalize every workspace-relative path and reject escapes.
- Bound unfiltered Explorer inventory to 5,000 rows and complete-index search to 256 control-free query characters/200 matches; parameterize search values and ignore stale renderer results.
- Bind code search to the current workspace ID; accept only a 1-256-character control-free literal, parameterize FTS input, verify candidate source identity/hash before exact matching, and ignore stale renderer generations. ASCII matching is case-insensitive while non-ASCII matching is case-exact.
- Allow writes and formatting only for existing indexed UTF-8 text with a matching content hash and matching backend-issued workspace ID; analyze before mutation, revalidate filesystem identity, hold the current-workspace lock through atomic replacement, and treat rename as the commit point. Keep post-commit derived-projection recovery distinct from disk-save success.
- Never index `.env*`, private keys, certificates, credential stores, `.git`, dependency folders, or build output.
- Keep the API inventory read-only, workspace-bound, parameterized, and
  source-indexed. Return only allowlisted operation metadata and sanitized
  HTTP(S) targets; never return OpenAPI descriptions, examples, schemas,
  security values, bodies, server defaults, remote references, or source bodies.
- Keep the high-level graph evidence-backed: exclude test/E2E, `.env`/credential,
  raw call-target/module, and inferred-resolution noise from `systemOverview`;
  never manufacture cloud, webhook, schedule, or execution claims.
- Keep `executionFlow` root-selected, scanner-hard-deny filtered, production-only
  by default, cursor-bound, and visibly truncated. Never follow an inferred
  global-name resolution across languages or present a same-language candidate
  as proven lexical binding.
- Keep graph text/filter input bounded and treat structural-community metadata
  as snapshot-relative topology, never a semantic, ownership, or runtime claim.
- Never return secret values to React. Env-loading commands return names only; external paths come from Rust-owned native dialogs. Provider and run stores remain separate.
- Reject process-loader and parent-control run variables; bound and deduplicate every renderer-selected environment-name list.
- Launch only the executable and exact argument array owned by a backend-registered profile, never renderer-supplied command text or an interpolated shell command. Treat the explicit Run/Debug action as authorization and immediately revalidate source, executable, and working-directory identities before spawn.
- Treat Observe as explicit child instrumentation only: nonce-bind and bound the
  strict `AONE_TRACE_V1` grammar, reject unknown payload fields and sensitive
  source paths, retain reported source mapping as inference, and never upgrade
  a static card from an observed runtime edge.
- Treat OTLP as explicit local instrumentation only: native-consent the
  loopback listener, require its ephemeral capability token, bound/decompress
  before parsing, retain only semantic allowlists, infer process-reported source
  matches, and stop/zeroize on workspace change. Never advertise it as a remote
  collector, payload inspector, or reverse debugger.
- Treat Debug as explicit cooperative child instrumentation only: bind nonce,
  session, run, workspace, workflow, sequence, epoch, parent, and source; accept
  shape-only previews; fail closed on protocol corruption; and keep native Stop
  independent of child acknowledgement. Never claim OS/thread suspension,
  browser attachment, stack stepping, arbitrary lines, values, or reverse execution.
- Launch a terminal only from a detected opaque shell profile after native consent, with one-session reservation, canonical workspace cwd, a cleared allowlisted environment, Unix non-blocking cloned descriptors, cancel-aware bounded byte-order-preserving PTY-pump backpressure, and failure-driven registry shutdown before base64 event transport.
- Bind local Git to fixed `/usr/bin/git`, the exact workspace repository root, contained literal paths, bounded execution, disabled hooks/prompts/remotes, and native consent for mutations.
- Resolve AI-tool configuration from a backend allowlist only; keep the default catalog non-inspecting, never expose contents, and require distinct inspection/open consent plus parent-chain/target identity revalidation before private template creation or fixed external-editor opening.
- Keep Project Agent inspection single-flight and workspace/report-bound; allow prompt-free retrieval only for the current workspace's already approved in-memory report, perform no new environment inspection before consent, keep build/config hint reads fixed-name/indexed/contained and names-only, label dependency results non-authoritative, use only fixed canonical tool candidates/argv with identity revalidation and clean bounded probes, and never install or mutate automatically.
- Keep HTTP runtime events value-free: method, all-query-values-hidden URL, status/duration, header names, byte counts, and truncation only. Independently redact authorization, cookie, token, key, password, and secret fields in bounded renderer responses.
- Permit explicit-Send HTTP authorization for IP-literal loopback without a managed-run prerequisite. For every other target, obtain bounded native HTTP permission before DNS or other network activity. Then resolve and validate every destination address, pin DNS results into a direct client, prohibit metadata and non-routable special-use destinations, and disable proxies and redirects. Warn explicitly for confirmed literal private requests; keep `localhost` and every other DNS name on the confirmation path.
- Apply equivalent destination validation and direct connection binding to WebSocket handshakes; permit only `ws`/`wss`, reject embedded credentials and fragments, and keep emitted destinations sanitized to their origin.
- Keep AI optional for startup and code-only workflows. Ignore renderer-supplied prompt text, use a fixed Rust-owned task, require consent for every call, and keep selected-node plus internal-edge context bounded and redacted.
- Limit HTTP bodies, AI evidence, runtime history, graph query size, scanner work, and parser traversal.
- Grant the renderer event listen/unlisten only; observed events originate in Rust.

## Resource ceilings

| Surface | v0.1 ceiling |
| --- | --- |
| Workspace scan | 20,000 qualifying files; 256 MiB aggregate candidate source; 500,000 extracted facts; 120 seconds |
| Project bootstrap | 300-byte public GitHub HTTPS URL; 100-byte safe direct-child name; one serialized action; clone 120 s; init 12 s; 16 KiB bounded process output; no overwrite/submodule/LFS |
| Explorer file inventory/search | 5,000 initial path-ordered rows; 256 control-free query characters; case-insensitive complete-index substring search; 200 matches |
| Workspace code search | 256 control-free query characters; 500 returned matches; 50 occurrences/file; 64 MiB attempted source reads; cooperative 750 ms budget checked between candidate operations; 2,000 source/200 path/1,000 semantic candidates; 4,000 source candidates for 1-2 characters; explicit truncation |
| Indexed/preview source file | 2 MiB / 4 MiB |
| Document facts | Markdown/MDX/TXT/RST/AsciiDoc safe UTF-8 only; 250,000 visited lines and 5,000 facts/file; fenced/source code blocks skipped; explicit truncation metadata |
| Editor write/format input | existing indexed UTF-8 file; 2 MiB source; 64-hex expected content hash; print width 40-500; external formatter 10 s |
| Graph projection | ordinary neighborhood: 500 nodes, depth 4, 50 explicit roots; `systemOverview`: 60 nodes, 120 edges, default depth 2, the same 50-root ceiling, and fixed per-layer seed quotas; `executionFlow`: exactly one root, depth 4, 500 nodes / 2,000 edges per response, 100 root links/page, 40 links/descendant, 500 expansions, 20,000 scanned links |
| Relationship search/community | 256 control-free query characters; depth-two neighborhood; 500 nodes; 32 node-kind and 32 edge-kind filters of 64 characters each; deterministic annotation of the returned bounded snapshot |
| API inventory | exact complete-index totals; 128-character query; stable opaque keyset cursor; 200 operations/page; renderer follows at most 10,000 operations and fails visibly instead of silently truncating |
| Runtime evidence | 2,000 events in Rust; process stdout/stderr share a 50 events/s FIFO; 100 ms React batches (at most 10 commits/s); latest 500 retained; Observe accepts 8,192 trace records and 4,096 unique spans/run |
| Instrumented debugger | one active serialized workflow/process; 2,000 retained safe points/session; newest 250/snapshot; eight retained sessions; 16 KiB marker; 30 s control acknowledgement; running timeline samples 10:1; shape-only preview; one pending cooperative control |
| API request | 16 KiB URL; 128 headers and 64 KiB aggregate header bytes; 1 MiB JSON body; 100 ms to 60 s timeout |
| API response | 2 MiB body; emitted runtime event carries metadata/byte count only, never body/header values |
| WebSocket connections | 8 connecting/open sessions; 16 KiB URL; 64 headers and 32 KiB aggregate header bytes; 16 protocols, 128 bytes each and 1 KiB aggregate; 250 ms to 60 s connect timeout, default 15 s; 5 s DNS limit |
| WebSocket transport | 8 queued outbound messages per session; 1 MiB outbound text/decoded binary and inbound message/frame; 1,398,104-byte encoded-base64 input; 10 s send/flush/close limit |
| WebSocket event projection | 256 KiB text preview; 192 KiB raw binary encoded to at most 256 KiB base64; 1 KiB detail; 64-byte session ID; 120 message events per one-second session window plus explicit dropped summaries; original byte length and truncation flag |
| Terminal | one starting/active session; 64 KiB decoded input/call; 64 outstanding input messages or 256 KiB including in-progress write; 2-500 columns; 1-300 rows; 16 KiB output chunks; 32-chunk backpressure queue; at least 16 ms between data events (about 62.5/s); 32 renderer pre-open events |
| Local Git | exactly one validated file per 512 KiB `--no-renames` diff; 200 paths/64 KiB aggregate for stage/unstage; 512 KiB raw staged-content identity; 4 KiB commit message; 12 s/process; no hard-denied, repository-wide diff, remote, or destructive endpoint |
| Project environment | one inspection; 39 fixed tools; 64 inherited/128 total search dirs; four candidates/tool; 80 filename marker facts; 16 fixed probes; four concurrent; 6 s/probe; 12 KiB per stdout/stderr; 240-byte version line |
| AI evidence/provider response | renderer selects at most 16 nodes from the active two-hop corridor; Rust keeps 24 node/edge evidence items and 32 KiB input; 512 KiB provider response; 1,200 output tokens |
