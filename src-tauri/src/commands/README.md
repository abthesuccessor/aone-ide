# Tauri command boundary

The commands feature is the renderer-facing orchestration layer. Its public command names and Tauri
macro symbols are re-exported from `mod.rs`, preserving the handler paths registered in `lib.rs`.

## Responsibilities and data flow

- `snapshot.rs` returns application, workspace, and runtime summary state.
- `workspace.rs` owns native workspace selection, initial scan/store/watcher installation, explicit
  rescans, indexed-file listing, and bounded source previews.
- `graph.rs` serves ordinary bounded neighborhood queries and the fixed-cap
  evidence-backed system overview, then refreshes the AI feature's evidence snapshot.
- `profiles.rs` detects structured local run profiles and installs them for the current workspace.
- `api_inventory.rs` pages the deduplicated API operation catalog for an exact workspace ID.
- `environment.rs` owns the two native environment-file pickers and separates provider configuration
  from variables explicitly available to local runs.

Workspace opening is prepared off the UI thread and committed only after the bounded scan, database
replacement, state construction, and watcher startup all succeed. A failed preparation cancels the
runtime workspace switch. For an initial open, `complete` is emitted only after SQLite replacement,
watcher startup, workspace/runtime installation, and the AI evidence projection is refreshed or
explicitly cleared; the scanner's earlier terminal phase is `analyzed`. Explicit rescans hold the
shared analysis reservation through store commit and emit `complete` only after post-commit summary
handling and AI refresh-or-clear finish.

Open Folder and explicit rescan share one backend workspace-operation reservation, so two renderer
requests cannot scan or switch concurrently. After native selection, the accepted operation waits for
active watcher/editor analysis in a blocking worker rather than blocking the async executor or failing
spuriously. Selecting the already active canonical root reuses its context, watcher, and SQLite handle
and routes through transactional rescan; it neither opens the same database twice nor installs a
second watcher. For a different root, the old analysis reservation is held until the new watcher is
ready, then the old watcher is released/joined before the prepared context is installed.

## Security and budgets

Privileged filesystem paths come from backend-owned native dialogs, never renderer parameters.
Relative previews are required to exist in the index and pass scanner containment plus descriptor-safe
read checks. Environment results expose names only: `AONE_AI_PROVIDER`, the
OpenAI key/model names, and the Anthropic key/model names enter only
`SecretState`, while those names are stripped from run environments. API, AI, and process commands
remain in their owning hardened features. Scanner, store, graph-query, preview, and runtime limits are
preserved by delegation rather than duplicated here.

`list_api_endpoints` accepts only the current opaque workspace ID, a bounded query,
opaque cursor, and 1-200 page limit. The store owns parameterization, service-aware
deduplication, exact totals, metadata allowlisting, and external-target sanitization.

## Tests

`tests.rs` verifies provider configuration never enters runtime state, provider names are removed
from a selected run environment, the active canonical root is routed to its existing context, and
workspace selection waits for active analysis instead of failing. The full Rust suite validates every
Tauri macro re-export, workspace flow dependency, graph synchronization path, and serialized command
contract.
