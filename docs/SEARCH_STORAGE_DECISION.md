# Search and storage decision

Status: accepted for Aone IDE 0.1 on 2026-08-17.

## Decision

Aone keeps one Rust-owned SQLite database per workspace, adds a derived SQLite
FTS5 search projection, and continues to run bounded graph algorithms with
`petgraph`. It does not bundle PGlite, Memgraph, Neo4j, or another database
daemon.

This is a product decision, not a preference for a smaller download. The chosen
path keeps filesystem authority and source snippets behind typed Tauri commands,
shares transactions with the existing AST graph, starts without a second
runtime, and remains fully offline.

## Amendment: transactional outbox (2026-09-19)

The SQLite decision above is unchanged. Workspace change notifications now go
through a transactional outbox table in the same database rather than being
emitted directly from a mutation.

Emitting straight from a mutation lets facts and notifications diverge: the
process can exit between a committed transaction and a delivered event, or an
event can be published for a transaction that later rolls back. Recording the
event as a row inside the same transaction removes both cases. The watcher
drains the outbox after each committed batch and acknowledges only after
emitting, making delivery at-least-once and strictly ordered.

This is a pattern, not a database feature, and needs no additional engine.

## What the sizes mean

The DMG is a compressed installer, not the workspace database. The current
debugger ARM64 local-test DMG is 8,813,878 bytes (about 8.41 MiB), while an earlier real TYSON
scan had already produced a 204,791,808-byte SQLite file (195.31 MiB
logical; 213,088 KiB allocated) containing 4,747 files, 170,482 graph nodes, and
189,813 graph edges. A small installer therefore does not mean that AST or graph
analysis is missing. Tauri also uses the operating system's
[WKWebView on macOS](https://v2.tauri.app/reference/webview-versions/) instead of
shipping another browser engine inside the application. That historical DMG's
canonical, target-specific, and staged copies were byte-identical and bound SHA-256
`c15166f8369e39c792d66fe620846ca1c42d5f8ccbebfda5d08e75757ada019f`.

Aone will not add padding or an unrelated engine to reach an arbitrary 20-50 MB
download. New binary size is accepted when it buys a measured product capability.
Derived per-workspace index size is reported separately from application size.

## Why SQLite FTS5

[SQLite FTS5](https://www.sqlite.org/fts5.html) is an inverted full-text index
inside the database already linked into Aone. The bundled SQLite 3.53.2 build has
`SQLITE_ENABLE_FTS5`; no runtime extension download is needed. FTS5 supports
prefix search, BM25 ranking, contentless indexes, and a trigram tokenizer for
general substring matching. Contentless storage lets Aone index source terms
without storing a second renderer-readable copy of every file.

The search projection is rebuildable from the selected folder and AST facts.
It participates in the same replacement transactions as files, graph nodes,
and graph edges. Source candidates are keyed by relative file path and semantic
candidates by graph-node ID. Current source is securely re-opened through the
existing canonical containment/no-symlink reader and accepted only when its
BLAKE3 hash still matches the committed file row before an exact line, column,
and preview cross IPC.

The renderer waits 170 ms and sends an opaque workspace ID plus a literal
control-free query. Three-or-more-character queries use contentless trigram
indexes; one- and two-character queries use a bounded indexed-file fallback.
Literal verification is ASCII case-insensitive and non-ASCII case-exact. One
request returns at most 500 results and 50 occurrences per file, reads at most
64 MiB of attempted source, and checks a cooperative 750 ms budget between
candidate operations. Source, path, and semantic
candidate lists cap at 2,000, 200, and 1,000; short-query source fallback caps
at 4,000 files. Reaching any ceiling sets the response's truncation flag.

SQLite is explicitly designed for desktop application files, and WAL allows
readers and a writer to proceed concurrently on one host. See SQLite's official
[application guidance](https://www.sqlite.org/whentouse.html) and
[WAL documentation](https://www.sqlite.org/wal.html). Aone additionally
serializes analysis mutations, so it does not depend on arbitrary parallel
write throughput.

### Current scanner/store proof

The pre-fix scanner/store proof processed TYSON directly into a disposable
database without modifying or executing the workspace. It indexed 4,753 files
into 170,684 graph nodes and 190,020 graph edges. The old alphabetic default
query nevertheless returned 100 file nodes and zero edges: the canonical graph
was populated, but its default seed window was a poor high-level view. Those
stored counts do not prove that every fact belongs to one giant component;
disconnected components and isolated facts remain valid.

A release-mode read-only query of a copied index then returned a diverse
35-node/9-edge overview: layer counts 4/4/7/15/5/0 and four `contains` plus
five `handles` edges. One warm-up plus 20 serial outputs were identical, with no
test/E2E/evaluation/example/benchmark/sensitive/raw/inferred-resolution/dangling
leakage. External correctly remained empty because no eligible absolute
non-local HTTP target was stored. This changed no canonical rows and required no
second graph engine.

Using those 35 sorted overview IDs as roots, the depth-4 detail query reached
500 nodes / 491 edges and remained truncated. Dependencies retained 347 edges
(94 `calls`, 60 `imports`, 193 `resolvesTo`) and repeated identically 20 times.
This broader snapshot deliberately retains noisy raw/inferred/test evidence and
is not the curated architecture view. A module-only alternative returned 100
modules / zero edges on TYSON and was rejected.

Full replacement now prepares each row shape once, performs supported
contentless FTS delete-all operations, and rebuilds selective semantic rows and
indexes in sets inside the same rollback-safe transaction. Incremental
replacement remains trigger-driven. On a generated 500-file corpus with 18,500
nodes and 24,000 edges, initial persistence improved from 1.904 to 0.439 seconds
(about 4.3x), and replacing the existing graph improved from 3.515 to 0.505
seconds (about 7x). The manual benchmark is ignored in the normal test suite;
rollback and incremental-search invariants remain ordinary automated tests.

### Earlier projection-sizing probe

A disposable TYSON probe on the earlier workspace snapshot indexed the exact
4,747 scanner-approved UTF-8 files:
38,987,418 source bytes produced a 74,510,336-byte contentless trigram index in
7.58 seconds. Warm candidate lookups took 0.18–1.05 ms for representative terms
such as `websocket`, `repository`, `api/v1`, and `GRANTS_CAPABILITY`. Exact
locations were deliberately not copied into the index; Aone re-opens candidate
files through its secure reader.

Indexing every one of the 170,482 graph-node labels was wasteful: it added about
76.60 MB. Restricting the semantic projection to the 15,988 meaningful facts
after excluding file, import-module, and call-target rows reduced it to about
1.54 MB and built it in 0.37 seconds. This measured selective design is what the
implementation uses.

## PGlite evaluation

[PGlite](https://pglite.dev/docs/about) is a capable Postgres build compiled to
WebAssembly and exposed through TypeScript/JavaScript. Its official documentation
describes a package under 3 MB gzipped, Postgres extensions, and browser, Node,
and Bun use. Those are valuable qualities for web applications, local Postgres
development, and browser-side RAG.

They do not improve Aone's current boundary:

- persistent filesystem storage is a Node/Bun/Deno-runtime facility; Aone's
  macOS WKWebView renderer is a browser and therefore uses
  IndexedDB or an OPFS worker, according to the official
  [filesystem matrix](https://pglite.dev/docs/filesystems);
- PGlite has one exclusive database connection and uses a worker/proxy when
  multiple clients need access, as documented in its
  [getting-started notes](https://pglite.dev/docs/) and
  [multi-tab worker guide](https://pglite.dev/docs/multi-tab-worker);
- PGlite's [upgrade guide](https://pglite.dev/docs/upgrade) requires dump and
  restore for some version transitions, while Aone's derived SQLite projection
  is rebuilt from the selected workspace;
- placing the canonical index in the WebView would move source-derived state
  out of the Rust authority boundary, while running it in a sidecar would add a
  JavaScript/WASM runtime and a second persistence system;
- Postgres compatibility, `pgvector`, and PGlite live queries are not current
  requirements. Tauri events already carry bounded filesystem and runtime
  changes to React.

### Local feasibility probe

A disposable, repository-external probe used `@electric-sql/pglite` 0.5.5 and
10,000 synthetic documents. On this Mac, an in-memory PGlite process took about
1.1–1.4 seconds to initialize, built its table and GIN text index in roughly
117–175 ms, and answered a warm prefix query in about 3.3 ms. The process RSS
increase was about 1.0–1.27 GiB. The npm package reported about 25.4 MB unpacked.

For a directional baseline, the system-native SQLite process created and queried
an equivalent 10,000-row trigram FTS5 set in about 50 ms with roughly 17 MB peak
RSS. This is not a universal database benchmark: the engines, tokenizers, and
process startup paths differ. It is a directional observation, not a reproducible
benchmark artifact: the disposable harness and raw output were intentionally not
retained in this product repository. It indicates that a PGlite migration would
add cold-start and memory cost without solving a current Aone requirement. The
temporary package and data were removed after the probe.

## Why no graph database daemon

The graph is real: Tree-sitter produces source-ranged AST facts, normalized node
and edge rows persist in SQLite, indexed adjacency queries construct a bounded
60-node/120-edge Architecture overview, a depth-4/500-node Dependencies
neighborhood, or a root-selected cursor-paged `executionFlow`; `petgraph` runs a
linear-time Kosaraju SCC pass on a bounded in-process projection, and React plus
focused D3 zoom/selection modules render deterministic SVG rectangles. Fixed
overview quotas prevent alphabetically early disconnected file rows from
consuming the Architecture view; their sorted IDs anchor the Dependencies
neighborhood. System and Runtime use `executionFlow`, whose responses cap at
500 nodes/2,000 edges and whose renderer progressively mounts bounded source
groups/cards/edges without force physics. Static TypeScript/JavaScript listener and
publisher literals also become inferred event nodes with `listensTo` or `emits`
owner edges; dynamic event names remain unresolved calls. A separate graph
database would duplicate the same facts and introduce synchronization failure
modes.

Memgraph is optimized for an in-memory graph server with persistence, ACID
transactions, Cypher, drivers, and graph procedures. Its official macOS guide
requires a running Memgraph server (shown through Docker) before clients or
Memgraph Lab can connect. See Memgraph's
[capability overview](https://memgraph.com/capabilities) and
[macOS deployment guide](https://memgraph.com/blog/how-to-install-memgraph-and-memgraph-lab-with-docker-on-macos).
That is useful for multi-client, whole-graph Cypher workloads; it is heavier than
the single-user, local, bounded-query workload Aone currently has.

Distribution is also not a neutral dependency choice. Memgraph's current
[official legal page](https://memgraph.com/legal) states that Community Edition
uses the Business Source License and that embedding Memgraph in a product for
end-user distribution accepts its OEM license. Aone therefore cannot describe a
bundled Memgraph build as a simple permissive open-source addition.

Two smaller candidates were also rejected. The
[Kuzu repository](https://github.com/kuzudb/kuzu) is archived, which is not an
acceptable foundation for a new production store. Embedded Rust
[CozoDB](https://github.com/cozodb/cozo) remains pre-1.0 without promised API,
query-language, or storage compatibility, and its lightweight persistent path
already uses SQLite. Either choice would add a second projection and migration
surface without removing Aone's existing store.

## Revisit criteria

Reconsider the storage engine only after a benchmark demonstrates one of these:

- bounded SQLite adjacency reads cannot meet an explicit latency target;
- the product needs arbitrary whole-repository Cypher exposed to users;
- several independent processes must concurrently mutate the same graph;
- Postgres wire compatibility or a Postgres-only extension becomes a product
  requirement; or
- vector retrieval is approved with an evidence, privacy, size, and lifecycle
  design that the current deterministic search cannot satisfy.

Until then, SQLite FTS5 plus `petgraph` is the faster, smaller, and safer embedded
graph-search architecture for this desktop IDE.
