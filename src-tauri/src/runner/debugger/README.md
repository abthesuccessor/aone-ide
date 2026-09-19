# Cooperative execution debugger

This module implements an experimental, bounded safe-point protocol for one
explicitly instrumented managed child. It does not infer that arbitrary source
lines ran and is not DAP, OS suspension, a browser debugger, or thread-wide
process control.

## Layout

- `protocol.rs` parses nonce/session-bound `AONE_DEBUG_V1` stdout envelopes,
  validates source candidates and shape-only previews, and creates qualified
  runtime notifications.
- `control.rs` owns the reserved-stdin `AONE_DEBUG_CONTROL_V1` grammar and its
  bounded delivery channel.
- `registry.rs` enforces run/session/workflow identity, global event sequence,
  control epochs, one-safe-point Step permits, and bounded snapshots.
- `tests.rs` locks protocol bounds, pause/step races, sequential workflows,
  timeout failure, truncation truth, and secret-name rejection.

## Contract and limits

- Debug mode requires Observe mode and an explicit Run/Debug action; it does
  not open a redundant native process-launch dialog.
- Each run has a fresh backend nonce and debug-session ID. The managed child
  and its dependencies share that nonce, so it is correlation—not attestation.
- One cooperative leader adapter may report one serialized workflow at a time.
  Concurrent requests, descendants reading stdin, threads, and distributed
  traces are not controlled. The bundled fixture queues concurrent requests.
- A session accepts at most 2,000 checkpoints. Snapshots return the latest 250
  with total count, start sequence, and explicit truncation state.
- Dense `running` checkpoints are stored in the registry, while renderer
  notifications are sampled 10:1. Paused and workflow-terminal checkpoints are
  always published, so the stdout reader never waits on the normal 50/s log
  gate before draining protocol input.
- Pause is only acknowledged by a later process-reported `paused` checkpoint.
  Step releases one cooperative checkpoint permit. A 30-second unacknowledged
  control terminalizes only the debug protocol; it never proves process state.
- Stop uses the existing process-group TERM then KILL authority. Cooperative
  stdin Stop is best-effort, and `stopped` is recorded only after leader exit is
  observed.

## Data safety and evidence truth

The child may report bounded workflow/step IDs, semantic operation/resource
identifiers, workspace-relative source candidates, branch enums, and preview
shape/type/counts. The grammar has no dedicated scalar-value, header, body,
raw-query, bind, environment-value, or arbitrary-attribute fields. Known loaded
secret substrings are rejected across all names and sensitive field names are
replaced. However, Aone cannot independently distinguish a semantic identifier
from a value or query fragment encoded into that name. Instrumentation authors
must keep every identifier and schema name free of values and personal data.

Accepted events mean only “the managed child emitted this valid protocol
checkpoint.” Source, operation, state, and schema metadata remain
process-reported candidates and never promote a static graph edge to observed.

Run the process-level adapter proof with:

```bash
cd fixtures/trace-demo
npm run test:debug
```
