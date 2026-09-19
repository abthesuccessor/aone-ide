# Aone performance and data-structure policy

## Performance boundary

Aone optimizes for a single engineer exploring one local repository. The design
favors bounded work, deterministic output, and a small installer before adding
distributed infrastructure or sophisticated algorithms.

The repository currently proves structural properties through limits, tests,
bundle checks, and release inspection. It does **not** yet contain a repeatable
benchmark result for idle RSS, a 10,000-file scan, single-file refresh latency,
or graph-query latency. Those values must be measured on declared hardware and
fixtures before they are used as production claims.

## Data path

| Stage | Primary structures | Reason |
| --- | --- | --- |
| Discovery | bounded `Vec<PathBuf>` | Paths are collected once, sorted deterministically, then consumed sequentially. |
| Parsing | iterative `Vec` stacks | Iteration avoids call-stack growth; independent visit, depth, fact, byte, and time ceilings bound work. |
| Identity | BLAKE3 content and structured-ID hashes | Changed bytes are detected without timestamps alone; length-prefixed ID parts avoid ambiguous concatenation. |
| Persistence | SQLite tables, transactions, and B-tree indexes | The graph is durable without a bundled database daemon. |
| Workspace search | contentless FTS5 trigram indexes, file/node document keys, bounded candidate vectors, BLAKE3 verification | Indexed retrieval stays fast while exact source and snippets remain under the Rust filesystem boundary. |
| API inventory | SQLite window/CTE projection, service-aware key, opaque keyset cursor | Full-index search and exact totals stay deterministic while each IPC page remains bounded to 200 operations. |
| Neighborhood expansion | `HashMap`, `HashSet`, and layer frontiers | Average constant-time membership prevents repeatedly scanning selected nodes and edges. |
| System overview | six fixed seed quotas, deterministic SQL ordering, bounded expansion | A diverse high-level map cannot be consumed by alphabetically early disconnected file rows. |
| Execution-flow layout | per-lane/source-group arrays and bounded degree ranking | Rectangles, orthogonal routes, and disclosure order remain deterministic without a physics loop. |
| Explorer tree | bounded `Map<string, FileTreeNode>` | One path lookup per component avoids repeated sibling-array scans while building the 5,000-row initial tree. |
| Runtime history | bounded `VecDeque`, shared FIFO gate, renderer batch queue | Appending/eviction stay constant-time; output and React commit rates are explicit. |
| WebSocket sessions | capped `DashMap`, semaphore, bounded channels | Independent session commands and tasks need concurrent keyed access without one global lock. |
| Workspace opening | synchronous mutation counter, boolean/ref guard, request generations | Folder opening cannot cross an active save/format; old-workspace mutations stop immediately after leave consent and cancellation restores the prior view without accepting late work. |
| Editor concurrency | workspace ID, current-workspace lock, pre-commit AST, BLAKE3 expected hash, atomic rename | Rejects stale workspaces/buffers and prevents a workspace switch across the commit section; disk commit stays distinct from recoverable derived-index refresh. |
| PTY transport | one session, bounded input budget, synchronous output channel, fixed-size chunks | Interactive bytes cannot create an unbounded queue or multi-shell process fan-out; output pressure blocks the reader without breaking ANSI byte order. |
| Git status/commit identity | NUL-delimited porcelain, bounded byte buffers, BLAKE3 raw-index fingerprint | Paths are unambiguous, per-file diff cannot expand repository-wide, and unchanged filenames cannot hide staged-content changes during consent. |
| Tool configuration | eleven-element static allowlist plus path-identity vector | Constant small lookup replaces renderer-supplied paths; inspection is explicit and the bounded parent chain is re-attested around create/open consent. |
| Project environment | fixed catalogs, sorted/deduped path vectors, bounded candidate maps, semaphore, latest-report slot | Permissioned deterministic discovery stays linear in explicit limits and avoids recursive home scans or unbounded process fan-out. |
| Network destinations | sorted/deduplicated `Vec<SocketAddr>` plus shared classifier | All DNS answers are checked once, then transports connect only to the validated set. |
| Deterministic metadata | `BTreeMap` and sorted vectors | Stable serialization and ordering make evidence and tests reviewable. |
| Graph algorithms | bounded `petgraph` projection | Algorithms run only after the persistent graph has been reduced to the requested projection. |

## SQLite storage

SQLite is the canonical local structural store. It runs in WAL mode with
foreign keys enabled, `synchronous=NORMAL`, and a five-second busy timeout.
Full-workspace and per-file replacements are transactions: file, node, edge,
quota, and derived-resolution changes either commit together or roll back.

The schema has indexes for the access paths used by the application:

- the `files`, `graph_nodes`, and `graph_edges` primary keys support exact ID
  lookup;
- `files(language)` supports language aggregation and filtering work;
- `graph_nodes(file_path)` supports file-owned replacement and deletion;
- `graph_nodes(kind, label)` supports declaration and kind-oriented reads;
- `graph_edges(source)` and `graph_edges(target)` support neighborhood
  expansion in either direction;
- `graph_edges(kind)` supports edge-kind filtering.
- contentless `source_search` and `graph_search` FTS5 trigram tables provide
  ranked document-key candidates, with mapping tables joining them back to
  authoritative file hashes and selective graph-node facts.

SQLite WAL improves durability and reader/writer behavior; it is not presented
as proof of arbitrary parallel throughput. The current desktop workload is
small and state access is deliberately coordinated. Explorer path filtering
remains a separate bounded `%term%` query: it renders 5,000 ordered paths
initially and returns at most 200 complete-index path matches for a debounced
256-character control-free query.

The Search activity uses the implemented FTS5 projection. A query of at least
three characters retrieves at most 2,000 source document keys and 1,000
selective semantic node keys; path candidates cap at 200. One- and two-character
queries use a deterministic 4,000-file source fallback because trigram lookup
does not represent shorter terms. The engine then securely re-opens candidate
files, BLAKE3-verifies them against the committed file rows, and performs exact
literal location matching. One request returns at most 500 matches and 50
occurrences per file, attempts at most 64 MiB of source reads, and checks a
cooperative 750 ms budget between candidate operations. ASCII letters match
without case; non-ASCII characters retain exact case. Every reached cap sets
`truncated` rather than implying an exhaustive result.

Full-workspace replacement uses one set of prepared row statements, clears the
contentless FTS projections through their supported delete-all command, and
rebuilds selective semantic documents and indexes in sets inside the same
transaction. Incremental file replacement remains trigger-driven. Rollback
tests prove that rows, indexes, and triggers return to the previous valid state
when a bulk replacement fails.

On the checked-in ignored manual benchmark corpus of 500 files, 18,500 graph
nodes, and 24,000 graph edges, initial full replacement improved from 1.904 to
0.439 seconds (about 4.3x) and replacement of an existing graph improved from
3.515 to 0.505 seconds (about 7x). This is a local directional benchmark, not a
cross-machine percentile claim.

The pre-fix scanner/store proof processed the real TYSON workspace into a
disposable database without modifying or executing TYSON: 4,753 indexed files
produced 170,684 stored nodes and 190,020 stored edges. The prior alphabetic
default query then returned 100 file nodes and zero edges. That result did not
mean the AST/edge store was empty; it demonstrated a seed-selection failure.
It also cannot establish that the full repository is one giant component:
disconnected components and isolated facts are expected in static analysis.
The system overview corrects presentation selection without changing the
canonical stored counts.

A release-mode query harness then copied the existing 277,934,080-byte TYSON
index and ran `systemOverview` without rescanning. The result contained 35 nodes
and 9 edges with `truncated: true`: 4 configuration, 4 entry, 7 interface, 15
application, 5 data, and 0 external nodes; edge kinds were 4 `contains` and 5
`handles`. One warm-up plus 20 serial queries produced identical serialized
output on every measured run. No test/E2E/evaluation/example/benchmark,
sensitive, raw call-target/module, inferred-resolution, or dangling fact
appeared. The original TYSON index remained unchanged; disposable copies were
moved to macOS Trash after verification.

The result contained the backend `main` function/file, frontend/admin main
files, five concrete endpoints, two path-inferred worker/task definitions, and
schema/model/repository facts. `ProviderInputEvidence` and
`TaskExecutionMethod` remain inferred definitions, not observed schedules. One
repository function likely belongs to inline Rust `cfg(test)`, but safely
removing it needs analyzer-owned test-context evidence; guessing from its name
would create false negatives. Data placement likewise does not prove a
datastore technology or persistence operation.

Warm observed query latency was 990.4 ms minimum, 1040.4 ms median, 1501.5 ms
p95, and 1646.3 ms maximum. Substantial unrelated CPU load was present, so this
is local observational evidence, not a controlled benchmark or product SLO.

The 35 overview IDs anchored a 500-node/491-edge detail result with
`truncated: true`. Dependencies retained 347 edges: 94 `calls`, 60 `imports`,
and 193 `resolvesTo`. Twenty repeated detail results were identical, and warm
latency was 8.25 ms median / 9.20 ms p95 in the same observational harness. This
detail view included 26 test/E2E nodes and false inferred cross-language
`resolvesTo` candidates; they remain visibly inferred/dashed and outside the
curated System claim. An unshipped module-only seed probe produced 100 module
rows / zero edges on TYSON.

A separate initial overview construction in the disposable real-store audit was
about 1.8 seconds. Its setup/load conditions differ from the warm serial timings
above, so neither number is a “blazing-fast” production claim or SLO.

No Neo4j, Memgraph, or other graph server is bundled. For this local workload,
Tree-sitter facts, indexed SQLite adjacency reads, bounded `petgraph`
algorithms, and deterministic SVG with D3 zoom/selection already implement the graph without a daemon,
network protocol, duplicated source of truth, or separately managed lifecycle.
PGlite would add a JavaScript/WebAssembly persistence boundary without a
current Postgres-only requirement. The store boundary can change if measured
workloads no longer meet it; see [Search and storage decision](SEARCH_STORAGE_DECISION.md).

## Scanning, hashing, and stable identity

The ignored-directory walker does not follow links. It collects at most 20,000
qualifying files and 256 MiB of candidate source, then uses Rust's standard
path ordering before analysis. Sorting makes the same workspace produce a
repeatable processing order. Because sorting occurs only after a hard file cap,
its `O(F log F)` comparison cost is bounded.

Every opened source file is checked against filesystem identity and bounded to
2 MiB before parsing. A full BLAKE3 digest records its content. Incremental
updates compare content rather than trusting modification time alone, so a
timestamp-preserving edit still invalidates analysis.

Editor save and format requests bind to the backend-issued workspace ID. Saves
reuse the digest as an optimistic concurrency token. While holding
the analysis reservation, the backend validates and AST-analyzes the complete
proposed UTF-8 content within the same 2 MiB limit before any mutation. It then
writes a same-directory `create_new` temporary file, preserves permissions,
fsyncs it, and rechecks the original identity/hash. A current-workspace read
lock spans that final validation and atomic rename, preventing workspace
installation from crossing the short commit section.

Rename is the disk commit point. Every preceding failure returns an IPC error
with the old target intact. After rename, the command returns the exact committed
content and new digest; directory-fsync, transactional GraphStore replacement,
or summary-refresh failure is logged but cannot honestly turn that disk commit
into an error. The prior graph record remains intact on transaction failure,
and the watcher or **Rescan Workspace** repairs derived state. This separates
source durability/provenance from a rebuildable projection.

Stable graph IDs hash a namespace plus length-prefixed identity parts. Named
symbols include source identity such as path, kind, qualified label, signature,
and sibling ordinal; anonymous structures include edit-sensitive location data.
The displayed stable ID uses 32 hexadecimal digest characters, while the file
content hash retains the full digest. Hashing is an identity and invalidation
tool here, not evidence that two semantically equivalent programs are equal.

Tree-sitter traversal is iterative. AST walks stop at 250,000 visits or depth
512, identifier searches stop at 4,096 visits or depth 128, and one file can
emit at most 5,000 facts. A limited traversal records truncation metadata rather
than silently claiming complete analysis.

## Graph queries and algorithms

Neighborhood reads use layer-by-layer breadth-first expansion because every
edge has equal hop cost in the current UI. The query is bounded to 50 explicit
roots, depth 4, 500 nodes, and a proportional edge ceiling. `HashMap` provides
node and edge deduplication; `HashSet` filters edges whose endpoints both remain
in the selected projection. Candidate, node, and edge vectors are sorted before
return so database or hash iteration order cannot change the UI result.

For a bounded projection with `Vb` nodes and `Eb` edges, traversal and endpoint
filtering are `O(Vb + Eb)` aside from database work and final deterministic
sorting. The bounds matter more than the asymptotic label: the renderer never
receives an unbounded whole-repository graph.

The application deliberately loads the structural snapshots in dependency order. First,
`systemOverview` runs at depth 2 with a hard 60-node / 120-edge cap. The
controller then sorts those IDs, takes at most 50, and anchors a depth-4 /
500-node neighborhood. Architecture uses the curated overview; Dependencies
alone uses the anchored detail snapshot. System and Runtime use a separate
root-selected `executionFlow`. All results share workspace and same-workspace
request guards.

Overview selection runs six fixed layer queries with quotas 4/4/7/7/5/3, then
round-robins candidates before bounded relationship expansion. Entry ordering
prefers production `src/main.*`, deprioritizes examples/build/evaluation/
migration paths, and keeps one conventional entry per project root. Data-path
tokens such as repositories, persistence, storage, models, schemas, and
migrations provide inferred Data placement; this is classification, not a
datastore-operation claim. Only statically extracted absolute non-local HTTP(S)
targets qualify for inferred External placement, and even then the target and
provider remain unverified. Final output sorts by layer, case-folded label, and
stable ID.

`executionFlow` accepts exactly one root, uses depth at most four, and returns at
most 500 nodes / 2,000 edges per page. Its root relationship page is capped at
100 links and each descendant expansion at 40; total traversal is capped at 500
expansions and 20,000 scanned links. The opaque cursor is bound to root, depth,
limit, and test-inclusion state. Counts and omissions apply the production-path
and scanner hard-deny filters before paging, so a hidden sensitive/test row
cannot consume a visible page. Cycle metadata is derived on the returned
projection, not by starting a second graph service.

The renderer merges at most 1,000 flow nodes, 2,000 edges, and ten pages. It
then mounts no more than seven source groups per lane, three cards per collapsed
group or eighteen when expanded, and 600 SVG edges. Each reached ceiling is
reported; a narrower selected root is the continuation strategy after the
client ceiling.

Cycle metadata uses Kosaraju strongly connected components through `petgraph`,
which is `O(Vb + Eb)` on the projection. Component members and the final
component list are sorted for deterministic output. A uniform-cost A* helper is
compiled only for tests; Aone v0.1 does not claim a user-facing shortest-path
feature. Weighted paths should be added only when evidence classes and product
semantics define meaningful costs.

## Collections and ordering

`Vec` is the default for bounded sequential data because it is compact and
cache-friendly. It is used for scan results, parser output, graph projections,
and IPC arrays. Capacity is reserved where the upper or discovered size is
known. A `Vec` is not used as an unbounded event log.

`VecDeque` stores the Rust runtime-event tail. New observations append at the
back and old observations pop from the front after 2,000 items; React queues
arrivals and commits one ordered batch every 100 ms, retaining the latest 500.
This matches the queue access pattern without shifting every element and caps
timeline reconciliation at ten commits per second.

`HashMap` and `HashSet` are used for identity lookup, deduplication, graph
membership, detected profiles, and small in-memory indexes. Their iteration
order is never treated as presentation order. A sort or ordered collection is
used whenever output must be reproducible.

`BTreeMap` is used for graph and runtime metadata because key order affects
human review, snapshots, and serialized evidence. `BTreeSet` is used where
deduplication and deterministic iteration are both required, including selected
environment names and coalesced watcher paths.

Sorting uses Rust or JavaScript standard-library comparators with an explicit
stable key. Aone does not ship a custom quicksort, merge sort, or radix sort:
the data is already bounded, and custom implementations would add correctness
and maintenance risk without a measured benefit.

## Binary search and concurrent maps

Binary search is appropriate only when a collection is already sorted and has
a clear comparison key. The WebSocket validator has one deliberate use: its
compile-time `MANAGED_HEADERS` array is lexically sorted, immutable, and checked
for every proposed custom header, so membership needs no per-request sorting or
allocation. Current large exact lookups use SQLite indexes or hash collections;
one-pass bounded vectors are consumed sequentially. Adding binary search to
those paths would increase complexity without reducing the dominant parsing,
I/O, or SQL work. A measured hot path may introduce another sorted snapshot and
binary search later, with build and lookup cost benchmarked together.

Likewise, `DashMap` is not a general replacement for `Mutex<HashMap<...>>` or
`RwLock<HashMap<...>>`. Low-cardinality application, run, profile, and secret
state benefits from one explicit invariant-preserving lock. The WebSocket
registry is the narrow exception: independently keyed session commands and
reader/writer tasks access it concurrently, no cross-session transaction is
required, and an eight-permit semaphore caps connecting plus open entries. No
`DashMap` guard survives an `.await`; session controls are cloned before async
channel or shutdown operations. This keeps sharding local to the access pattern
that justifies it.

## Watcher and runtime backpressure

Filesystem notifications enter a 64-item bounded channel. A batch retains at
most 2,048 distinct paths, waits for a 250 ms quiet period, and has a two-second
maximum collection window. Overflow or an oversized batch becomes one bounded
full rescan rather than an unbounded backlog. Explicit scans and watcher work
share an analysis coordinator so stale incremental work cannot commit over a
new full result.

Process stdout and stderr are read asynchronously, converted into at most
16 KiB logical lines, redacted, and serialized through one shared FIFO gate.
The combined gate reserves one emission every 20 ms (50 events/s); each reader
holds at most one line while waiting so sustained production applies OS-pipe
backpressure without an unbounded IPC queue. API response bodies, event bodies,
AI evidence, and provider output have independent byte or item limits. These
ceilings protect memory; they do not by themselves establish end-to-end latency
or throughput targets.

WebSocket state admits at most eight connecting/open sessions, each with an
eight-message outbound channel. `try_send` rejects a saturated queue, so slow
network output cannot turn renderer messages into unbounded memory. Text and
decoded binary payloads are capped at 1 MiB; accepted base64 input is capped at
1,398,104 bytes before decode. Tungstenite permits at most 1 MiB per inbound
message and frame, uses 16 KiB read/write buffers, and caps the write buffer at
1,064,961 bytes. Connect, DNS, send, flush, and close operations also have
explicit time limits. Event previews are smaller than the transport maximum and
carry a truncation flag plus original byte length.

Inbound renderer work is also bounded per session: a fixed one-second window
admits 120 text/binary message events. Further frames are consumed, counted,
and summarized by one typed `dropped` event at the next window or immediately
before close. Lifecycle/error handling and ping/pong service bypass this limit.
Secret patterns are deduplicated and capped at 256 values and 256 KiB total;
leftmost-longest matching is cached by fingerprint and writes through a capped
output buffer so redaction cannot amplify a preview without bound.

The integrated PTY has a separate one-session reservation. On Unix, the master
is set `O_NONBLOCK` before cloning reader/writer descriptors; non-Unix launch
fails until an equally interruptible implementation exists. Renderer input is
limited to 64 KiB decoded bytes per call and uses non-blocking enqueue to a
dedicated writer. A shared atomic budget includes the in-progress write and
permits at most 64 outstanding messages or 256 KiB; saturation is retryable
without closing the session. Columns clamp to 2-500 and rows to 1-300. The
reader places at most 16 KiB per chunk into a 32-item bounded queue and spaces
data events by at least 16 ms (about 62.5/s). Partial-write/flush, read, and
full-queue retries inspect cancellation every 5 ms, applying bounded
backpressure while preserving chunk and ANSI byte order instead of dropping
output. Writer/read/child-wait failure closes the registry session. Shutdown
sends HUP then SIGKILL to the original process group and does not rely on a
retained slave descriptor unblocking I/O. Terminal bytes bypass runtime history,
SQLite, graph facts, and AI evidence entirely.

Local Git invokes one fixed executable with a 12-second deadline. Status/diff
first copy the bounded index into a private temporary metadata snapshot and use
an empty attribute tree plus the original object store as read-only alternate;
repository/global/system configuration and executable helpers are unavailable.
Status uses NUL-delimited porcelain and bounded path counts; each diff requires
one validated non-hard-denied file, uses `--no-renames`, and retains at most 512 KiB.
An empty path cannot turn the command into repository-wide diff work. Commit
captures validated paths between two equal raw-index reads, BLAKE3-hashes at
most 512 KiB of `--cached --raw -z --full-index --no-renames` output, and repeats
the snapshot after consent. Identical filenames with changed staged content
therefore invalidate approval.
Mutations share one lock because repository index changes need serialization,
while the exact-root and path checks are repeated after consent. This is a
small, predictable local surface rather than an attempt to implement a full Git
client.

Tool-configuration lookup is constant-small over eleven entries, but path
validation walks the bounded relative parent chain. Rust records canonical path,
file/directory identity, length, mode, and change metadata before consent, then
requires an equal attestation afterward and directly before `/usr/bin/open`.
This deliberate repeated `O(P)` work, for parent depth `P`, closes the useful
consent race without adding a filesystem watcher or content read.

Project Setup uses one fixed 39-tool catalog. After native approval, it
deduplicates at most 64 inherited PATH entries plus compiled standard and
home-relative directories into a sorted vector capped at 128. Each tool keeps
at most four canonical candidates, so discovery is bounded by `O(T * D)` fixed
path joins/stat checks rather than a recursive disk walk. Exact-name stack
markers are queried from the existing index and capped independently. The
names-only hint pass considers at most 24 allowlisted indexed build/config
files, reads at most 64 KiB per file and 512 KiB total, and emits only bounded
environment names plus controlled PostgreSQL/Redis classifications. The
optional second approval runs at most 16 relevant fixed probes through a
four-permit semaphore, six-second deadlines, bounded pipes, and 350 ms drain
joins. One latest-report slot and workspace/report IDs prevent unbounded report
history or cross-workspace reuse.

## Frontend work

Flow and Map share one deterministic rectangular renderer. Stable lane/rank/
source ordering and orthogonal routes replace force physics, so there is no
continuous tick loop or drag-triggered topology restart. The canvas begins at
0.9 scale; pan/zoom preserve card size and explicit **Fit** computes a transform
from measured content bounds. Full-width Focus removes non-graph workbench cost
until Escape.

Progressive disclosure mounts at most seven groups per lane, three cards per
collapsed group, eighteen per expanded group, and 600 SVG edges. Search still
indexes every loaded node and forces a hidden match into the selected corridor.
Native flow pages cap each response at 500 nodes/2,000 edges; the renderer caps
merged state at 1,000 nodes/2,000 edges/ten pages and reports further omission.
D3 supplies selection and zoom transforms only; `d3-force` is not installed.

API + Data avoids force layout entirely. Rust searches all indexed operations,
returns exact filtered/unfiltered totals through stable 200-row keyset pages, and
merges only service/protocol/method/path duplicates. The renderer loads each page
into ID-keyed maps, caps retained operations at 10,000, then derives role, source,
and route groups with deterministic sorting. A stale, repeated, partial, or
cross-workspace cursor response fails visibly instead of becoming an incomplete list.

Monaco is loaded on demand so the default graph workbench does not pay its full
startup cost. The xterm.js terminal and fit addon are also lazy-loaded on first
terminal-tab use; once opened, the component stays mounted while other console
tabs are selected so a live PTY is not recreated. Its renderer scrollback is
fixed at 5,000 lines and remains session-only. Workbench preferences are small
versioned local-storage data, not source snapshots.

The Explorer loads at most 5,000 rows on initialization and uses a 180 ms
debounce before complete-index search. It can immediately show matches from the
loaded window, then replaces them only when the response still matches the
current workspace/query generation. Search status is live-announced and reports
initial-window, full-index pending, result-cap, empty, and error states. Tree
construction uses a path-keyed `Map`; keyboard focus has one roving tab stop and
falls back to the active or first visible path when prior focus disappears.

The separate workspace Search chunk loads only when selected. Its 170 ms
debounce starts a workspace-bound request whose generation must still match on
success or failure. Results preserve backend relevance order, group through a
path-keyed `Map`, and expose one roving tree tab stop. Activating a match reuses
the exact Monaco source-range path. This is distinct from Explorer filename
filtering and does not search runtime log history.

Async developer panels apply the same stale-result discipline at their own
boundaries. Editor requests carry workspace ID plus a renderer generation and
save/format results compare the submitted text so newer typing remains dirty;
Git status/diff/mutations, tool-config inspection/open, and Project Setup/AI use
workspace/request generations; settings moves focus into the dialog, closes on
Escape, traps Tab focus, capture-blocks workbench command shortcuts, and restores
the invoking element. Graph nodes are keyboard focusable,
support selection/source navigation without pointer input, and give the first
visible node the tab stop when filtering hides the selection.

Save/format increments a ref counter before its first await and decrements it in
`finally`. Folder opening checks that counter synchronously, so even a same-tick
click before React busy state renders cannot open a discard prompt or native
picker while an editor mutation is unresolved. After dirty-document leave
consent, `openingWorkspace` is set synchronously and
stays true across the native picker, backend scan, and frontend file/graph load.
Editor changes/save/format/close, new source opens, Git mutations, and tool-config
create/open handlers all refuse work during that interval. Picker cancellation
unlocks the unchanged tabs; a successful load increments generations before
old state is cleared, so late async results cannot repopulate it.

Terminal setup lists profiles and installs the native listener before enabling
Open. During native consent/open, React holds the newest 32 early events and
prints an omission count if that renderer-only buffer overflows; events are
filtered to the returned session ID. Write and resize promise failures surface
as a visible error; `{ accepted: false }` also triggers best-effort native close
before stale local state is cleared. Teardown unregisters the listener/ResizeObserver,
disposes xterm.js, and requests closure of an active session.

The native retained-slave regression uses a real Unix PTY, keeps its slave open
and unread, fills a writer call, sets cancellation, and verifies both cloned
reader/writer handles and worker liveness clear within a bounded deadline. This
proves the `O_NONBLOCK` plus 5 ms retry design rather than assuming handle close
eventually wakes a blocking worker.

React memoizes stable projections and retains bounded history. Runtime events
flush every 100 ms; unsourced output is excluded before graph projection so logs
do not recompute the workflow. WebSocket arrivals commit at most once per frame,
retain 300 events, and display 120 for the selected session. Cleanup cancels the
frame/listener, and Connect stays disabled until every listener is installed.
This is renderer backpressure, not a claim that every network frame is rendered.

## What is verified and what remains measured

### Canonical npm bundle baseline (2026-09-07)

A fresh `npm ci` in an isolated source copy exposed a mismatch between the
canonical lock and the existing mixed npm/pnpm installation. Resolving
dependencies from their actual owners found Monaco using DOMPurify 3.4.8 and
Vite using Rolldown 1.2.5 in the old installation. The canonical npm lock instead
resolves DOMPurify 3.4.13 and Rolldown 1.2.4. The checked source trees were
identical; the different resolved dependencies and compiler produced different
chunk boundaries.

Two consecutive builds of the fresh npm installation produced the same entry
asset and the same measurements below, on Node 24.18.0/npm 12.0.1/macOS ARM64.

| Measurement | Existing mixed installation | Canonical `npm ci` | Enforced limit |
| --- | ---: | ---: | ---: |
| Total frontend bytes | 15,702,990 | 15,710,460 | 15,720,000 |
| Entry JavaScript gzip bytes | 114,210 | 107,080 | 120,000 |
| All JavaScript gzip bytes | 3,716,260 | 3,723,644 | 3,730,000 |

The previous total-JavaScript limit of 3,720,000 bytes rejected the canonical
build by 3,644 bytes. Only that limit was recalibrated, by 10,000 bytes (0.27%),
leaving 6,356 bytes of measured headroom. Raw output and entry limits remain
unchanged. No feature was removed, dependency downgraded, or bundle excluded
from measurement. Future changes must use the canonical npm installation for
comparison; a new overrun needs its own investigation and rationale.

These are frontend build measurements. They do not establish installed startup
time, runtime memory use, or a cross-platform result.

### Remaining runtime evidence

Automated gates cover scan/parser/query ceilings, deterministic projections and
layouts, transactional storage, runtime eviction, generated contracts, and
bundle budgets. The cooperative debugger additionally has a child-process proof
for a blocked initial safe point, one checkpoint per Step, Continue, sequential
workflows, a 122-checkpoint burst, and authoritative process-group Stop. Exact
current aggregate totals and artifact identity are recorded only after the final
post-debugger gates and package audit.

A previously recorded 8,813,878-byte debugger DMG passed its sanitized local-test
pipeline and artifact audit. It was not rebuilt in the 2026-09-07 source review
and is not evidence for the current changed tree. Packaged interaction evidence
is tracked separately from these automated size and correctness gates.
Compressed installer size is never a completeness metric.

Before a production-performance claim, add a repeatable benchmark command for:

1. cold and warm scan time at 1,000 and 10,000 files;
2. no-change and one-file incremental refresh latency;
3. graph query latency for the 60/120 overview, depth-4 500-node neighborhood,
   and first/subsequent `executionFlow` pages at the 500-node/2,000-edge limits;
4. static Flow/Map pan, Fit, and selection plus debugger path selection, replay,
   pane resizing, and safe-point update responsiveness at their maximum projections;
5. Monaco open/save/format latency at the 2 MiB file ceiling;
6. sustained PTY throughput/backpressure, renderer pre-open overflow reporting, resize responsiveness, and xterm.js rendering;
7. status/diff latency at the Git path and diff ceilings;
8. idle, scan-peak, and runtime-stream RSS;
9. SQLite cache size and disk growth across repeated edits;
10. FTS build/growth plus cold and warm workspace-search latency at short-query,
    candidate, result, byte, and time ceilings;
11. cold and warm API inventory search/pagination at 200, 500, and 10,000 operations;
12. sustained trace/debug rates, polling/IPC sampling, control latency, and eviction behavior.

Each result must name the commit, fixture digest, Mac model, CPU, memory,
macOS version, warm/cold state, repetitions, and percentile calculation.
