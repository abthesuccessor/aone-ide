# Graph algorithms

This feature projects a bounded `GraphSnapshot` into an in-memory directed Petgraph graph and runs
deterministic structural algorithms over stable node IDs. `algorithms.rs` contains the projection and
cycle algorithms; `communities.rs` contains deterministic structural community annotation.
`tests.rs` and `community_tests.rs` own fixtures and regression coverage.

## Algorithms and data flow

`strongly_connected_components` uses Kosaraju's algorithm, discards singleton components, sorts IDs
inside each component, and sorts the final list. The store uses this deterministic result when it
materializes cycle relationships. The test-only shortest-path helper uses A* with uniform edge cost
to verify the same projection preserves reachability.

`annotate_structural_communities` treats returned edges as an undirected, deduplicated structural
projection, ignores dangling endpoints, and applies bounded deterministic label propagation in sorted
node-ID order. Neighbor votes are weighted by one plus shared-neighbor count; ties select the
lexicographically smallest label. Community IDs hash sorted assigned member IDs, and isolates receive
singleton communities. Each node receives `communityId`, `communitySize`, `communityAlgorithm`,
`communityBasis`, and `communityComplete`. Size and identity describe only the returned snapshot;
`communityBasis` is `boundedGraphSnapshot`, and `communityComplete` is false whenever that snapshot is
truncated. The annotation does not change node or edge evidence and is not persisted as source fact.

The input snapshot has already been bounded by the graph store query limits. Edges whose endpoint is
absent from the snapshot are ignored, so a truncated or filtered snapshot cannot introduce phantom
nodes. Algorithms return IDs only and do not mutate persistent state.

## Security and tests

No source text, credentials, filesystem paths, network data, or process input is interpreted here.
Memory and work are constrained by the caller's bounded snapshot. Tests verify directed path
projection, multi-node cycle detection, singleton exclusion, deterministic component ordering,
order-independent community assignments, isolates, evidence preservation, and truncation metadata.
