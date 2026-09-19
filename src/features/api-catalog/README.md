# API catalog

The API catalog is the center-workbench paged, indexed endpoint inventory shown
by `API + Data`. It is intentionally separate from the bounded relationship
graph. Query limits on a graph must never be presented as a complete API list.

- `ApiCatalog.tsx` owns loading, filters, expansion, selection, and empty/error
  states.
- `ApiEndpointTree.tsx` renders server declarations and client requests as
  separate hierarchical groups. Source groups and route branches expose exact
  operation counts. Large groups reveal operations in bounded 80-row pages.
- `model.ts` owns stable grouping, filtering, sorting, and wire-level inventory
  types.
- `useApiCatalog.ts` follows opaque cursors until the backend exact total is
  loaded. It rejects repeated cursors, cross-workspace responses, and silent
  partial completion.
- `bridge.ts` type-checks the Tauri command and page against the generated
  OpenAPI client contract before adapting them to the catalog model.

The renderer collects at most 10,000 operations. This prevents an untrusted
workspace from growing renderer memory without a bound. A larger exact backend
total is reported as an incomplete inventory instead of silently truncating it.

Search is performed by Rust across the complete inventory, then the loaded
result can be narrowed by method and coverage. It covers method, route, label,
operation ID, handler label or kind, framework, grouping, and source path. A double-click or Command-Enter opens only the backend-provided
indexed source range. No client request is labelled as a server declaration.
Legacy facts remain visibly unclassified until a workspace rescan establishes
their direction.
