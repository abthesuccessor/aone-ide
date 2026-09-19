# Store feature

## Responsibilities

This feature owns the embedded SQLite graph projection. `GraphStore` opens the database, replaces whole-workspace or per-file facts transactionally, removes deleted paths, lists files, and returns bounded graph snapshots. Persistence and row conversion are separated from traversal so storage changes do not blur query behavior.

## Security invariants

- SQLite runs with WAL journaling, foreign keys, a bounded busy timeout, and normal synchronous durability.
- Every replacement is transactional. Quota failure rolls back the file, nodes, edges, and derived resolution edges together.
- Whole-workspace replacement suspends only derived FTS triggers and secondary indexes inside the same transaction, rebuilds them before commit, and therefore exposes either the old complete projection or the new complete projection—never an intermediate schema or index.
- Per-file and aggregate limits cap indexed files, source bytes, and extracted facts.
- Query roots, depth, nodes, seeds, and edges are bounded before data reaches the renderer.
- The optional system-overview projection is capped at 60 real nodes and 120 real edges. It suppresses test/E2E paths, raw call targets/modules, and inferred `resolvesTo` links; ordinary neighborhood behavior is unchanged.
- The API inventory is a separate complete-index projection with exact totals,
  parameterized full-inventory search, and stable opaque keyset pages capped at
  200 operations. It never inherits the 60-node overview cap.
- Initial file lists stop at 5,000 entries. A case-insensitive path-substring query searches the complete index but accepts at most 256 non-control characters and returns at most 200 matches.
- SQL values are parameterized. The only interpolated table name is selected by internal fixed callers.
- Derived `resolvesTo` edges are rebuilt after mutations, preventing stale or newly ambiguous relationships from surviving.
- Source and semantic FTS5 projections are updated in the same transaction as their owning file and AST rows.

## Data structures and algorithms

- `files`, `graph_nodes`, and `graph_edges` form the normalized SQLite projection.
- `source_search` is a contentless trigram index mapped by `search_documents`; source bodies are not duplicated in SQLite. `graph_search` indexes only declaration-like facts, excluding high-volume files, imports, and call targets.
- `HashMap` and `HashSet` provide bounded frontier expansion and endpoint filtering.
- Full replacement reuses five prepared row statements, clears contentless FTS projections with their transactional `delete-all` command, rebuilds selective semantic rows set-wise, and recreates secondary indexes once after ingestion. Incremental file replacement and removal retain trigger-driven FTS updates.
- Relative imports are normalized component-by-component and linked only when one indexed file matches.
- Final call identifiers produce inferred links only for a unique workspace declaration, with explicit confidence and basis metadata.
- Tarjan-style strongly connected component results from `graph_algorithms` are attached to the returned node metadata.
- Fixed per-layer seed quotas select connected, non-test configuration, entry, interface/job, application, data, and external facts deterministically. Safe filename and semantic-kind placement is exact; conventional path placement is explicitly inferred.
- Window functions merge endpoint facts only by service/protocol/method/path and
  retain producer, client coverage, and legacy unknown counts.

## Flow

1. The scanner produces an `IndexedFile`.
2. Whole-workspace limits are preflighted, then checked again against inserted rows inside one transaction.
3. Full replacement clears derived projections and reuses prepared statements for file, source-index, node, and edge rows; incremental updates use the normal trigger path.
4. Contentless source and selective semantic search projections are rebuilt before their triggers and secondary indexes are restored.
5. Derived call/import resolution edges are rebuilt from the current database state with one reusable insert statement.
6. Neighborhood queries load bounded seeds and expand their normal frontier. System-overview queries choose diverse evidence-backed seeds, traverse only conservative relationship kinds, then both projections annotate components.
7. API inventory queries derive a deterministic canonical occurrence and cursor
   without loading an unbounded operation set into Rust or the renderer.

## Tests

`tests.rs` covers WAL-backed atomic replacement, bounded/truncated graph queries, import and unique-call resolution, ambiguity cleanup, rollback for file, byte, and fact quota violations, and discovery of a file beyond the initial 5,000-entry explorer window. `overview/tests.rs` proves diverse deterministic selection beyond 100 alphabetic dead files, safe configuration handling, source-less file classification, test/E2E suppression, honest quota truncation, execution-path inference, and inferred-resolution exclusion. `bulk_tests.rs` verifies one-time statement preparation, FTS counts, trigger/index restoration, incremental replace/remove behavior, rollback after a mid-ingest constraint failure, and an ignored timing-only generated-corpus benchmark. The search feature tests cover exact locations, stale-source rejection, transactional index updates, result truncation, and a 4,750-file synthetic corpus.
