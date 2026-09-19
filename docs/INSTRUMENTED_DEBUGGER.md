# Instrumented execution debugger

Aone v0.1 includes a cooperative debugger for a managed child process that
explicitly implements `AONE_DEBUG_V1`. It turns application-defined safe points
into a deterministic ordered workflow. It does not suspend arbitrary
threads, intercept an ordinary browser, or discover every executed source line.

## What the debugger proves

After the user explicitly selects Debug, Aone starts one registered local run profile and owns
its process group. Debug mode also reserves the child's standard input for a
fixed control protocol and injects a fresh, per-run nonce and session ID.

An accepted step proves only that this managed child emitted a valid safe-point
record for the current run, workspace, nonce, session, workflow, sequence, and
control epoch. Source locations, labels, branch outcomes, operation names,
resource names, and data shapes are process-reported. Source matching remains a
candidate correlation and does not upgrade static AST evidence.

The focused API trace desktop has four horizontally resizable panes:

1. API client for the explicit request
2. Monaco source at the selected process-reported range
3. The selected API workflow's ordered execution path
4. Step shape changes plus the separate request and response views

Each safe point is a compact node in report order. The renderer never uses a
force simulation. Advancing or selecting a node opens its bounded source range
in a replaceable preview tab; an explicit double-click pins it. A bounded static
API path may appear as muted indexed evidence before the request reports safe
points. Only accepted runtime safe points receive the live observed highlight.

At API-client Send time, the renderer records the accepted debug sequence and
the workflow IDs already present, then excludes all later events belonging to
those older workflows. This binds the focused view to the next serialized
cooperative workflow rather than the tail of an earlier paused request. The
protocol does not yet carry an API-client request ID, so an unrelated request
that is newly queued first remains an explicit correlation limitation.

## Controls

- **Pause** requests a pause at the next instrumented safe point. It is not an
  operating-system or thread suspension.
- **Continue** releases the paused workflow.
- **Cooperative breakpoint** can arm a previously reported source safe point in
  the active workflow. Continue then sends one Step Over permit, waits for the
  child's acknowledgement, and repeats until that stable checkpoint ID is
  reported. It cannot arm or stop an arbitrary Monaco line.
- **Step into hint** and **Step over hint** each release one instrumented
  safe-point permit. The adapter decides which next checkpoint is emitted;
  there is no stack-frame authority in v0.1.
- **Stop** uses Aone's process-group `TERM` then `KILL` authority. A cooperative
  stdin stop request is best effort and cannot block the native stop path.
- **Previous** selects retained history. **Next** selects a later retained step;
  at the newest step of a live paused workflow it sends one Step Over permit.
  Neither action reverses or re-executes the child process.
- **Replay** and **Pause replay** move only the retained observation selection.
  They are not cooperative child controls. OTLP receiver details are collapsed
  under the API-client pane. See [Local observability graph](OBSERVABILITY_GRAPH.md).

One pending cooperative control is allowed. If an instrumented child does not
acknowledge it within 30 seconds, only the debugger protocol session becomes
failed. Aone does not infer that the operating-system process paused, resumed,
or stopped.

## Workflow scope

The protocol supports one active serialized workflow in one managed process.
Sequential HTTP requests can reuse stable checkpoint IDs because every step is
namespaced by workflow ID. Concurrent requests, threads, child services,
distributed traces, and cross-process stepping are outside this protocol.

An application adapter should wrap each workflow so success emits
`workflowCompleted`, failure emits `workflowFailed`, and cleanup always releases
the workflow slot. The included Node fixture serializes requests to demonstrate
this contract. Ordinary projects do not gain internal steps until they add an
adapter or use a future debugger integration.

## Result previews

Debug safe points may contain only bounded structural previews:

- preview kind: none, scalar, object, array, or row set;
- type name;
- field name, field type, and nullability;
- item, row, and null counts;
- explicit truncation state;
- a bounded semantic operation and resource name.

The wire schema has no dedicated value, request/response body, header/query
value, environment value, raw-query, bind, stack-local, or arbitrary-metadata
field. Bounded labels, field names, operations, resources, and type names are
still process-reported strings, not a DLP boundary. Instrumentation authors must
keep those identifiers free of user data, query text, and secrets.

The adjacent API-client Request and Response tabs are a separate authorized
exchange, not safe-point fields. Their display remains subject to the native
HTTP client's bounds, configured-secret redaction, and response truncation.

## Protocol admission

The child writes exact `AONE_DEBUG_V1 ` JSON lines to stdout. Aone accepts them
only in Debug mode and validates the following before retaining an event:

- the per-run nonce and debugger session ID;
- bounded ASCII workflow, step, and parent identifiers;
- strict sequence and control-epoch progression;
- one active workflow and parent-before-child ordering;
- an allowlisted event kind, flow stage, state, branch outcome, and preview;
- a contained, non-sensitive workspace-relative source and bounded line range;
- a maximum 16 KiB logical line with no unknown fields.

The first malformed, truncated, secret-bearing, cyclic, duplicate, or
out-of-order marker fails the debugger protocol closed. Later markers cannot
resynchronize or become observed cards. Ordinary stdout remains ordinary output
and does not fail the debugger merely because it is long.

Controls use exact `AONE_DEBUG_CONTROL_V1 ` JSON lines over the reserved stdin.
The child receives only the nonce, session, action, next control epoch, and the
expected accepted sequence. A marker cannot execute a command, select a file,
change the workspace, mutate the static graph, or authorize an external action.

## Bounds and performance

- one starting or active managed process;
- at most 2,000 accepted safe points retained per debugger session;
- at most 250 recent safe points returned in one session snapshot;
- at most eight retained debugger sessions;
- running safe points are sampled 10:1 only for the general runtime timeline;
  the bounded debugger registry still retains every accepted checkpoint;
- frontend ordered trace-path disclosure remains bounded to 120 steps per page;
- paused and terminal safe points are never sampled out;
- Stop remains responsive even if the cooperative channel is stale or full.

## Included proof fixture

`fixtures/trace-demo` implements a dependency-free Node adapter and a real HTTP
workflow:

```text
request -> route -> service -> repository -> data shape -> branch -> response
```

Run `npm run test:debug` in that directory. The process test proves initial
pause with no HTTP progress, one checkpoint per step, resume, sequential
workflows, high-rate pause responsiveness, shape-only data, and process-group
termination when the child deliberately ignores cooperative Stop.

## Not yet implemented

- Debug Adapter Protocol stack frames, scopes, variables, and real step modes;
- Node Inspector, `debugpy`, or `lldb-dap` adapters;
- Chrome DevTools Protocol browser attachment or DOM/network interception;
- automatic line, `if`/`else`, SQL, Cypher, trait-dispatch, or repository
  instrumentation for arbitrary projects;
- remote/distributed collector operation and cross-service stepping; the local
  loopback OTLP/HTTP JSON receiver is observation/replay only;
- reverse execution or time-travel debugging;
- arbitrary safe-point runtime values; request and response values are shown
  only for the separately authorized Aone API-client exchange.

A browser or Aone API Client request can trigger a workflow only when the target
managed application emits the safe points. Without that explicit adapter, Aone
shows the request boundary and static source evidence, not a fabricated internal
execution chain.
