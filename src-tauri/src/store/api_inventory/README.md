# API inventory store projection

This module exposes every statically indexed HTTP operation through deterministic,
workspace-local keyset pagination. Production source paths are included while standard
test, spec, E2E, and fixture paths are excluded. It merges duplicate facts only
when service identity, protocol, method, and path match, so an OpenAPI declaration,
a Utoipa handler attribute, and client call sites remain one operation with
separate provenance while same-route operations in different services remain distinct.
Source-only TypeScript/JavaScript and FastAPI operations therefore remain available when no OpenAPI file exists;
static route-handler references or inline handler ranges are returned without upgrading their
inferred HTTP association to declared or observed behavior. Static Fetch calls contribute a GET by
default or a literal allowlisted method, while dynamic method configuration is not guessed.

`query.rs` owns validation, the bounded SQLite projection, exact matching and indexed
totals, source-path or OpenAPI-tag grouping, handler evidence, and client-coverage counts.
`tests.rs` covers OpenAPI/client/handler merging and cursor coverage beyond 500 operations.

Only allowlisted indexed labels, methods including CONNECT, paths, operation IDs, tags, frameworks, and source
locations are returned. Descriptions, examples, schemas, security values, source bodies, and
remote references never enter this projection. Each page contains at most 200 operations;
opaque keyset cursors make the entire indexed result reachable without loading it at once. Each
source file remains subject to the scanner's 5,000-fact ceiling, and truncated file analyses are
marked as incomplete. Facts without the current role metadata remain explicitly `unknown` until
the workspace is rescanned.
