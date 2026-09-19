# Runtime runner

This feature owns local process execution and runtime evidence. The renderer can
select only backend-detected profiles; it cannot submit an arbitrary command.

## Responsibilities

- `commands.rs` treats the explicit Run or Debug action as launch authorization,
  then coordinates process launch, stop escalation,
  and Tauri command responses.
- `state.rs` stores workspace-scoped profiles, zeroizing environment values,
  the single-run reservation, and the bounded runtime-event tail.
- `preparation.rs` resolves a backend-registered profile to canonical executable,
  working-directory, and source identities for immediate pre-spawn revalidation.
- `environment.rs` validates deny-by-default environment selection and redacts
  secrets from arguments, text, and JSON.
- `output.rs` converts bounded stdout/stderr lines to observed events and owns
  their shared per-run emission gate.
- `session.rs` binds every reader to one unforgeable run/workspace lifetime and
  cancels inherited pipes when the leader exits or a confirmed stop begins.
- `trace.rs` validates the opt-in `AONE_TRACE_V1` JSONL envelope; `trace_registry.rs`
  bounds process-reported span identities, events, parents, and phase changes.
- `debugger/` validates cooperative `AONE_DEBUG_V1` checkpoints, sends fixed
  reserved-stdin controls, and stores a bounded live workflow. Its README
  defines the intentionally narrower-than-DAP contract.
- `process.rs` signals the isolated process group.
- `events.rs` records and emits backend-authored runtime evidence.
- `timestamp.rs` creates dependency-free RFC 3339 UTC timestamps.

`mod.rs` is deliberately limited to module declarations and public re-exports.

## Invariants

1. At most one process may be starting, running, or completing stop escalation.
2. Workspace switching is rejected while that process slot is occupied.
3. Only a registered profile may run; executable and argument overrides must
   exactly match the backend profile.
4. The renderer submits only an opaque registered profile ID, selected environment
   names, and run mode. It cannot submit executable text or arguments. Clicking
   Run or Debug is authorization; no redundant native process dialog is opened.
   Filesystem identities are revalidated immediately before spawn.
5. Child environments start empty, inherit only the safe parent allowlist, and
   receive explicitly selected project variables that cannot alter loaders or
   parent process controls.
6. Commands never pass through a shell. Unix children receive a fresh process
   group so stop targets descendants as well as the leader.
7. Output lines, event history, selected environment names, and arguments are
   bounded. Known secrets and sensitive assignments are redacted before IPC.
8. Stdout and stderr share one FIFO gate capped at 50 emitted events per second
   (one slot every 20 ms). Lifecycle and error events bypass it. Each reader
   retains at most one 16 KiB logical line while waiting, so the bounded OS pipes
   provide backpressure; the gate never samples, merges, strips ANSI sequences,
   or changes the accepted line payload.
9. Observe mode recognizes only explicitly instrumented `AONE_TRACE_V1` lines
   carrying the per-run nonce. Fields and kinds are allowlisted; attributes,
   headers, bodies, query values, and arbitrary payloads are not accepted.
10. Trace labels and source ranges remain process-reported. Known loaded secrets,
    sensitive assignments, and hard-denied source paths are rejected/redacted,
    but instrumentation authors must still keep labels free of user data.
11. A uniquely matched source range is only an inferred candidate correlation;
    it never upgrades a static AST hop to observed. An ordinary browser refresh
    or uninstrumented application reveals no internal handler/service flow.
12. Debug mode requires Observe mode, one explicitly instrumented leader
    adapter, and one serialized workflow. Pause/Step are acknowledged safe-point
    permits; Stop remains backend process-group authority. Process-reported
    names may disclose schema, so instrumentation authors must not place values
    or personal data in them.

## Data flow

explicit Run/Debug action -> `StartRunRequest` -> registered profile -> canonical
path preparation -> identity revalidation -> empty-environment spawn -> bounded stream
readers -> backend-authored `RuntimeEvent` records -> Tauri event listener.

The output gate serializes stdout and stderr at the point they are observed.
When producers exceed 50 lines per second, both async readers wait on the same
gate and the process eventually blocks on its bounded OS pipes. This deliberately
trades log latency for bounded memory and IPC work without an unbounded queue.
Recognized debug markers bypass this log gate: every accepted checkpoint is
drained into a 2,000-event registry, dense running UI notifications are sampled
10:1, and paused/terminal checkpoints are always published.

`StartRunRequest.observe=true` supplies the child with the protocol name and a
fresh nonce after the explicit launch action. The child must emit the documented
structured lines itself. Accepted span/parent facts mean "reported by this
launched process"; they are not debugger, OpenTelemetry, or browser proof.

`StartRunRequest.debug=true` additionally reserves stdin for fixed
`AONE_DEBUG_CONTROL_V1` JSON lines. It accepts only bounded nonce/session-bound
checkpoints and shape/type/count previews. The grammar has no dedicated raw
value/query fields, but process-reported names are not independently proven
value-free. Pause occurs at the next cooperative checkpoint, Step releases
exactly one checkpoint, and a 30-second missing acknowledgement fails only the
protocol session. See `debugger/README.md`.

Stopping marks the run, sends `SIGTERM` to its process group, waits 1.5 seconds,
then sends `SIGKILL` when necessary. The global slot is released only after the
leader is reaped and any requested escalation is complete.

## Tests

`tests.rs` and `tests/trace_tests.rs` preserve the runtime security regressions:
trace bounds/transitions, stop rollback, cancellation, workspace isolation,
terminal-event ordering, environment
isolation and bounds, single-run state transitions, path escape and replacement
rejection, explicit-action launch policy, event redaction, event ordering,
timestamps, and bounded stream
allocation. `output.rs` also locks the combined 50 events/second cadence and
verifies delayed consumers cannot trigger a catch-up burst. Run it with:

```bash
cargo test runner::tests
```

The dependency-free process fixture exercises actual pause, one-permit Step,
Resume, sequential workflows, failure recovery, burst responsiveness, and an
ignored cooperative Stop followed by an OS process-group signal:

```bash
cd fixtures/trace-demo && npm run test:debug
```
