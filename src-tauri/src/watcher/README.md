# Workspace watcher

The watcher turns native filesystem notifications into bounded, serialized graph updates. Its
public surface remains `start_workspace_watcher` and `WorkspaceWatcher`; the implementation is
split so batching, path normalization, and store mutation can be reviewed independently.

## Responsibilities and data flow

1. `worker.rs` registers a recursive `notify` watcher and owns the background worker thread.
2. `batching.rs` coalesces paths deterministically in a `BTreeSet` after a 250 ms quiet period,
   with a maximum two-second collection window.
3. `paths.rs` normalizes existing and deleted paths without allowing a path outside the canonical
   workspace to become a relative graph key.
4. `worker.rs` waits for the workspace analysis reservation, analyzes each eligible changed file,
   and applies transactional per-file graph replacements. Ignore-file changes and queue overflow
   use the scanner's bounded full-rescan path.
5. A successful mutation refreshes the in-memory summary and AI graph snapshot, then emits one
   `aone-workspace-changed` event.

## Budgets and bounded behavior

- The callback queue holds at most 64 filesystem events.
- A delta contains at most 2,048 distinct paths and is collected for at most two seconds.
- Queue overflow, an oversized native event, or too many distinct paths becomes one full rescan;
  paths are not retained without a bound.
- Incremental analysis has the same 120-second wall-clock limit as a full scan. The deadline is
  passed into file analysis and checked between files. A failed delta falls back to the scanner,
  which enforces the workspace limits of 20,000 files, 256 MiB of source, and 500,000 facts.
- The store enforces the same aggregate workspace caps transactionally for each replacement.

## Concurrency and security

Explicit scans and watcher mutations share the workspace analysis coordinator. Watcher work waits
for an active explicit scan; an accepted Open Folder/rescan likewise waits off the async executor for
active watcher work. Neither side can commit a stale delta over a full replacement. A different-root
open holds the old context reservation until its replacement watcher is ready, avoiding an observation
gap before the old watcher is released and joined.
Hard-denied paths are discarded before reads. Discoverability uses the scanner's gitignore and
credential exclusions, and actual reads retain the scanner's canonical containment, final-component
no-follow, and opened-file identity checks.

## Tests

`tests.rs` verifies targeted replacement and deletion, serialization behind an explicit scan,
oversized-batch fallback, and deadline failure. Scanner tests separately cover exclusions,
workspace budgets, deterministic discovery, and descriptor-safe reads.
