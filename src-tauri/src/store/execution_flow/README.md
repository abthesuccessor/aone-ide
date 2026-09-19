# Execution-flow projection

This module turns indexed static facts into a bounded, deterministic workflow
view. It is a presentation projection, not a claim that the displayed path ran.

One selected endpoint, client/event boundary, or static graph node roots each
query. The response caps at 500 nodes, 2,000 edges, and depth four. Root links
are deterministically paged with an opaque query-bound cursor; cumulative totals
and omissions make continuation/truncation explicit instead of implying that a
hidden 500-node hairball was complete.

- `query.rs` owns pagination, bounded traversal, cycle marking, and trace-source
  resolution.
- `links.rs` derives directed presentation links from stored facts. It reverses
  declared `handles` relations for endpoint-to-handler reading, collapses a
  `calls` plus `resolvesTo` pair while retaining both source edge IDs, and joins
  client and producer HTTP facts only for an exact method/path match with one
  producer service. When an OpenAPI path includes a mount prefix omitted from a
  FastAPI decorator, it may bridge to one same-service handler only when the
  shorter path is an exact suffix and the OpenAPI operation ID starts with the
  handler name. This fallback edge is always labeled inferred and static-only;
  ambiguity produces no bridge.
- `metadata.rs` assigns stable lanes and expandable-card metadata without
  changing source evidence. Traits, implementations, methods, services, and
  utilities occupy Service/Agent; repositories, stores, schemas, and models
  occupy Data only when their kind or backend context supports that inference.
- `tests.rs` locks cycles, truncation, production-path filtering, cross-language
  inference filtering, source mapping, and a representative frontend-to-data
  chain.

Test and E2E paths are excluded unless `GraphQuery.includeTests` is explicitly
true. Scanner-hard-denied paths are always excluded, including legacy facts.
Global identifier inference is never followed across languages, and a
same-language global-name match remains a non-recursive candidate. Runtime
events are overlaid later only when their backend-issued `sourceNodeId` exactly
matches a returned node. Process-reported `mappedNodeId` metadata is not exact
runtime authority and cannot change static evidence.
