# Project Environment Intelligence

This module performs an explicit, permission-first inspection of the open
workspace's development environment. It is deterministic and local; it does
not use AI to choose commands or alter the machine.

## Consent boundary

1. The renderer can only request inspection for the current workspace ID.
2. A native dialog is shown before this command reads the language summary,
   bounded run/build/config sources, `PATH`, or any local executable location.
3. Discovery considers only a fixed tool catalog, inherited absolute `PATH`
   entries, fixed standard directories, and fixed common directories under the
   OS-resolved user home (Cargo, Volta, pyenv, SDKMAN, and similar). It never
   invokes a shell or reads shell startup files. Workspace-contained
   executables are excluded.
4. A second native dialog lists the exact canonical executable and fixed argv
   for every relevant version probe that could run. Declining it returns the
   detected paths as `unverified`.

After a report has been approved and created, the renderer may retrieve that
same in-memory report while its workspace is still current. This cache read
does not inspect the environment or show another dialog. It returns no report
for a different workspace; only an explicit inspection can refresh it.

## Execution boundary

Version probes are fixed by `catalog.rs`; renderer input never supplies an
executable, path, argument, environment variable, or working directory. Each
approved probe revalidates filesystem identity, clears the environment, uses a
bounded output drain, times out, and is terminated as a process group. At most
four probes run concurrently and at most sixteen are offered in one dialog.

The feature never invokes a shell, login shell, package installer, downloader,
project run command, or configuration writer. After the first approval it may
read at most 24 fixed-name, already-indexed build/config files, capped at 64
KiB each and 512 KiB total, through the scanner's containment, symlink, and
hard-deny checks. Only recognized environment names and controlled
PostgreSQL/Redis dependency hints survive; values are never returned or
retained, and hints are not required-service or readiness claims. Dedicated env
files, known credential paths, shell startup files, and AI/CLI configuration
contents remain excluded. Reports are bounded to the fixed catalog and the
latest report is keyed by workspace and report IDs.

## Files

- `command.rs` owns the two-consent workflow, read-only cache retrieval, and
  workspace race checks.
- `catalog.rs` is the fixed executable and version-argument allowlist.
- `discovery.rs` resolves safe executable candidates outside the workspace.
- `identity.rs` captures and revalidates executable filesystem identity.
- `probe.rs` plans and runs bounded version checks.
- `analysis.rs` derives stacks and recommendations from observed local facts.
- `config_hints.rs` securely bounds allowlisted build/config reads and evidence.
- `hint_parsing.rs` extracts names and controlled dependency classifications.
- `recommendations.rs` labels non-authoritative readiness guidance.
- `markers.rs` queries only bounded indexed filenames for exact stack markers.
- `confirmation.rs` builds escaped, bounded native consent messages.
- `state.rs` owns the single-flight reservation and latest bounded report.
- `time.rs` creates report timestamps without an additional dependency.
- `tests.rs` covers catalog, normalization, path identity, messages, state, and
  deterministic recommendations.
