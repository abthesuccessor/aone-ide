# Real-value debugger plan

Status: design accepted 2026-09-20. Increment 0 implemented; increments 1+ not
started. This supersedes nothing in
[Instrumented debugger](INSTRUMENTED_DEBUGGER.md) — that document still
describes what Aone actually ships today.

## The goal

Debug any opened project with debug mode on. When a request reaches the
project's API, show the executing path step by step — through DB, DTO and DAO
layers — with real data values at each step, the way a JetBrains IDE does.
Plus environment recommendations and one-click runnability.

## The constraint nobody can design around

The goal contains two requirements that no single mechanism satisfies at once.

| Requirement | Pausing debugger (CDP/DAP) | Tracing (OTLP) |
| --- | --- | --- |
| Real data values | Yes — complete, at a stop | No — OTLP carries no values by design |
| Whole path, automatically | No — only where you step | Yes — every instrumented boundary |

A pausing debugger gives real values only **at a stop**: one frame, one moment,
because you put a breakpoint there. It cannot light up an entire request path
unaided. Tracing gives the whole path automatically but carries no values, and
OpenTelemetry auto-instrumentation does not emit the `code.file.path` /
`code.line.number` attributes that `otlp/wire.rs` needs to map a span back to a
graph node — so auto-instrumented spans arrive unmapped and cannot highlight
source at all.

Anything promising both at once is promising two features. Say which one is on
screen, in the UI copy, or the feature will read as broken.

## What was verified

A spike drove Node's built-in V8 Inspector over CDP against an ordinary,
**uninstrumented** HTTP API. A real request produced:

```
#0 toDto        api.js:12
#1 (anonymous)  api.js:19      <- request handler
row = { id=77, customer="amra", total_cents=4250, status="PAID" }   <- DAO row
dto = { orderId=77, customer="amra", total=42.5, paid=true }        <- DTO
```

Real stack, real DAO and DTO values, working `stepOver`, zero changes to the
target project. That is the qualitative jump: today the `fixtures/trace-demo`
reference needs hand-placed `debugSafePoint()` calls through router, service and
repository to produce shape-only metadata.

Two adapters need no installation on a current macOS machine: Node's inspector
is built into Node, and `lldb-dap` ships with the Command Line Tools.
`tokio-tungstenite` is already a dependency, so CDP's transport exists.

## The honesty cost, which is not optional

Aone currently asserts in several places that the debug wire has no value field
at all. Shipping real values makes those statements false. They must change in
the same commit as the first value that crosses IPC:

- `SECURITY.md` — "no dedicated value, body, header/query value, environment
  value, raw-query, bind, stack-local, or arbitrary-metadata field"
- `README.md` — "The cooperative debugger is not DAP"
- [Instrumented debugger](INSTRUMENTED_DEBUGGER.md) — result previews, and the
  "Not yet implemented" list
- `src-tauri/src/runner/debugger/README.md`, the execution-debugger README
- The fixed UI copy in `TraceDataInspector.tsx` and `DebuggerChrome.tsx`, and
  the `limitation` string in `registry.rs` — several asserted verbatim by
  `ExecutionDebugger.test.tsx`

Redaction cannot be made sound for arbitrary values, and the plan should not
pretend otherwise. `redact_text` replaces already-loaded `.env` values and
`is_sensitive_name` masks field *names*; a token held in a local called `t`
renders in full. Real values need their own validator, their own caps, and an
explicit statement of what the boundary does not cover.

## Sequence

### Increment 0 — environment requirements (done)

`run_profiles/profile.rs` hardcoded `required_env: Vec::new()`, while
`ensure_required_env` and the renderer's missing-variable pre-flight gate were
both already implemented. The enforcement chain was live code fed an empty list.
`attach_required_env` now joins in the names the project's own configuration
references. Days of work, no new consent surface, and it delivers the
runnability half of the request.

### Increment 1 — Node CDP backend

A second debug backend behind the existing `DebugRegistry` state machine and its
two Tauri commands, speaking CDP to `node --inspect-brk` over the existing
WebSocket transport. Keeps `AONE_DEBUG_V1` untouched. Delivers real frames,
scopes, variables and true step in/over/out.

Scope honestly: **plain-JavaScript Node only**. There is no source-map code
anywhere in the repo, so TypeScript — and therefore NestJS, Next.js and most
real Node backends — will not bind breakpoints until increment 2. Frames and
variables must be fetched lazily by reference, not inlined per step; the
snapshot budget is 250 events and a variables tree does not fit.

### Increment 2 — source maps

Without this, increment 1 is a demo rather than a tool for most projects.

### Increment 3 — DAP backend

`lldb-dap`, `debugpy`, `dlv dap`, JVM. The same session contract; CDP does not
make this cheaper, only sequenced later.

### Increment 4 — observed path

Keep OTLP as the automatic path *underneath* the pausing debugger, not as a
substitute. Closing the `code.*` attribute gap requires Aone-specific span
processors that capture stack frames — a separate project, not a bundling task.

## Effort

Increment 1 alone is 13–16 weeks in this codebase: a CDP client, breakpoint
binding across CJS/ESM/`file://` URL forms, four IPC commands through
contract-first regeneration, new frontend files under the 500-line cap, and the
documentation rewrite above — all through the eight-stage `npm run check`.
Cross-language breadth and cross-service stepping are multiples on top.

"Any project, monolith or microservices, with values at every step" is not a
single release.
