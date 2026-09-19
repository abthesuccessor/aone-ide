# Domain contracts

The domain feature contains the serialized contracts shared by the Rust backend, Tauri command
boundary, persistent graph, runtime event stream, and React renderer. It intentionally contains no
filesystem, network, database, or process behavior.

## Responsibilities and layout

- `graph.rs` defines evidence levels, language capabilities, source locations, graph queries,
  nodes, edges, snapshots, and the `neighborhood` / `systemOverview` projection enum.
- `workspace.rs` defines application/workspace summaries, indexed and previewed files, scan and
  change events, and environment-load acknowledgements.
- `runtime.rs` defines run profiles, start/stop requests, bounded runtime-event
  records, and cooperative debug safe-point/session/control DTOs.
- `terminal.rs` defines the interactive PTY profile, action, and ephemeral event contracts.
- `api.rs` defines the API client request/response envelope.
- `api_inventory.rs` defines the workspace-bound, cursor-paginated API operation catalog.
- `ai.rs` defines evidence-selected explanation requests and responses.
- `devtools.rs` defines local Git and AI tool-configuration contracts.

## Data flow and security

All renderer-facing fields retain camel-case Serde names and the same optional/default behavior as
the original contracts. These structures describe data but do not grant authority: backend commands
still select workspaces and environment files through native dialogs, and privileged subsystems
validate paths, destinations, secrets, evidence IDs, and process arguments before using values.
Evidence kind remains explicit so declared, resolved, observed, and inferred information cannot be
silently conflated. System-layer evidence is additive presentation placement and
must not be interpreted as node/edge provenance.

## Tests

The consuming feature suites exercise serialization-compatible construction and use of every
contract. HTTP, AI, scanner, store, runner, watcher, and graph-algorithm tests provide the behavioral
coverage; Rust compilation catches any broken public re-export or changed field shape.
