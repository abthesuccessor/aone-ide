# Application and workspace state

The state feature owns the currently open workspace, its graph store and summary, the filesystem
watcher lifetime, the coordinator that serializes analysis with watcher mutations, and the
application-wide single-flight guard for renderer-requested workspace operations.

## Responsibilities and data flow

- `context.rs` defines `WorkspaceContext` and the RAII analysis reservation. Dropping a reservation
  releases the coordinator and wakes one waiting watcher operation.
- `summary.rs` builds a new context and derives a workspace summary from authoritative store counts
  and language aggregates.
- `app.rs` owns the application-support root, current context, watcher, and RAII workspace-operation
  reservation shared by Open Folder and explicit rescan. It installs workspace state only after
  scan/store/watcher preparation succeeds and rejects stale summary refreshes.

Workspace opening creates a per-workspace SQLite path beneath `Application Support`, performs a
bounded scan, creates the context, and installs its watcher. A concurrent renderer-requested open or
rescan fails at the outer single-flight boundary. The accepted operation waits for any active
per-context watcher/editor analysis without blocking the async executor, preventing concurrent stale
graph replacement. Reopening the active canonical root keeps that context and watcher and performs an
atomic store rescan instead of creating a second handle to the same database.

## Security and bounded behavior

Database filenames replace colon separators and are always joined beneath the backend-owned app-data
root. The state layer never accepts renderer paths directly. It delegates canonical containment and
scan budgets to the scanner, aggregate graph limits and transactions to the store, and watcher queue
limits to the watcher feature. Summary refresh validates the expected workspace ID before committing.

## Tests

`tests.rs` verifies database-path containment, single-flight workspace operations, and both analysis
reservation behaviors: a competing try-reservation fails cleanly, while waiting work blocks and
resumes without overlap after the active reservation is dropped.
