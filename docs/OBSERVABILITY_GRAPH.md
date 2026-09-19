# Local observability graph

Aone can combine its static workspace graph with bounded runtime trace evidence.
The implementation is a local developer tool: it is not a general collector,
APM backend, distributed debugger, or durable telemetry store.

## Implemented path

```text
static scan -> declared/resolved/inferred SQLite graph
                                   |
OTLP/HTTP JSON -> validated spans -> observed in-memory runtime events
                                   |
                                   v
                     renderer-only D3 projection
                     + trace history replay
                     + candidate source jumps
```

SQLite remains canonical for the static workspace graph. Runtime observations
live only in a bounded Rust memory queue and are projected into the renderer.
No OTLP request changes a static node's provenance or writes telemetry into the
workspace.

## Starting the receiver

Open **Instrumented Debug**, then select **Start receiver**. A native dialog
describes the network and retention boundary before Aone binds a socket. The
receiver:

- binds IPv4 loopback only, normally `127.0.0.1:4318`;
- accepts only `POST /v1/traces` over HTTP/1.1;
- accepts OTLP JSON with `Content-Type: application/json`;
- accepts identity or gzip content encoding;
- requires the displayed `x-aone-ingest-token` capability header;
- stops and invalidates its token when requested or when the workspace changes.

The UI provides copyable exporter variables:

```bash
export OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://127.0.0.1:4318/v1/traces
export OTEL_EXPORTER_OTLP_TRACES_PROTOCOL=http/json
export OTEL_EXPORTER_OTLP_TRACES_HEADERS=x-aone-ingest-token=EPHEMERAL_TOKEN
```

`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` is signal-specific, so it includes
`/v1/traces`. If an SDK supports only a base `OTEL_EXPORTER_OTLP_ENDPOINT`, use
the loopback origin and follow that SDK's signal-path rules.

The checked-in `fixtures/trace-demo` has a deterministic six-span JSON export.
After copying the live variables, run:

```bash
cd fixtures/trace-demo
npm run send:otlp
```

## Runtime data model

One accepted OTLP span becomes one `RuntimeEvent` with observed provenance.
The renderer uses the following stable fields:

| Runtime field | Meaning |
| --- | --- |
| `traceId` | validated 16-byte OTLP trace identity, rendered as 32 lowercase hex characters |
| `metadata.spanId` | validated 8-byte span identity, rendered as 16 lowercase hex characters |
| `metadata.parentSpanId` | optional validated parent identity |
| `metadata.traceProtocol` | `OTLP_HTTP_JSON`, `W3C_TRACE_CONTEXT`, or the existing `AONE_TRACE_V1` |
| `metadata.phase` | `event` for one complete OTLP span |
| `metadata.durationMs` | non-negative duration derived from start/end timestamps |
| `metadata.status` | `unset`, `ok`, or `error` |
| `metadata.flowStage` | stable presentation lane derived from supported semantic fields |
| source fields | optional process-reported workspace-relative path and line |

The graph projection groups unambiguous events by trace/span identity and draws
an observed `parentSpan` edge only when one unique retained parent and child are
present. If duplicate identities make a relationship ambiguous, the edge is
omitted rather than guessed.

## Trace context from Aone's API client

While the receiver is running for the current workspace, Aone's API client can
inject a W3C `traceparent` header into a loopback or private-network request.
It never overwrites a caller-supplied `traceparent`, never injects into a public
destination, and remains behind the API client's authorization boundary:
explicit Send for IP-literal loopback without a managed-run prerequisite, or
bounded native confirmation otherwise.

The injected client span and an instrumented server's OTLP spans share a trace
ID. A server span that reports the client span as its parent can therefore be
joined in the renderer even though the observations came from separate local
producers. This is trace-context correlation, not proof that every intermediate
operation was captured.

The header format follows the W3C Trace Context recommendation. OTLP request and
response shapes follow the OpenTelemetry protocol and generated trace-service
schema:

- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
- [OpenTelemetry context propagation](https://opentelemetry.io/docs/concepts/context-propagation/)
- [OTLP specification](https://opentelemetry.io/docs/specs/otlp/)
- [OpenTelemetry trace service protobuf](https://github.com/open-telemetry/opentelemetry-proto/blob/main/opentelemetry/proto/collector/trace/v1/trace_service.proto)
- [Canonical OTLP JSON trace example](https://github.com/open-telemetry/opentelemetry-proto/blob/main/examples/trace.json)

## Semantic mapping

Aone retains a narrow semantic allowlist. Supported fields include:

- resource `service.name` and `service.namespace`;
- code file, line, and function attributes;
- HTTP request method and route template;
- database system, operation, and collection identifiers;
- messaging system and operation identifiers;
- optional Aone-specific `aone.flow.stage`, accepted only as one of the seven
  fixed lane identifiers and used as a presentation hint;
- start/end time, span kind, and status code.

HTTP server/client, database, messaging, external-client, and function spans map
to existing graph lanes. The mapping follows current OpenTelemetry semantic
attribute names while accepting a small set of documented legacy aliases:

- [service resource conventions](https://opentelemetry.io/docs/specs/semconv/resource/service/)
- [HTTP span conventions](https://opentelemetry.io/docs/specs/semconv/http/http-spans/)
- [database span conventions](https://opentelemetry.io/docs/specs/semconv/db/database-spans/)
- [code attribute migration](https://opentelemetry.io/docs/specs/semconv/non-normative/code-attrs-migration/)

Arbitrary attributes, span events, links, payloads, request/response bodies,
headers, database statements, binds, and raw values are ignored. Original span
names are validated for envelope correctness but not retained because producers
can place high-cardinality runtime values in them. Display labels are rebuilt
from the allowlisted semantic identifiers.

Semantic identifiers are still producer-reported strings, not a DLP guarantee.
Instrumentation authors must keep service, route, collection, operation,
function, and file identifiers free of user data and secrets.

## Static-source correlation

If a span reports `code.file.path` and `code.line.number`, Rust first reduces the
path to a contained, non-sensitive workspace-relative path. A unique static
source match may then produce `metadata.mappedNodeId`.

That edge is always `inferred` with correlation basis
`processReportedSourceRange`. The runtime card remains `observed`; the static
node keeps its original `declared`, `resolved`, or `inferred` provenance. A
double-click opens the reported candidate range. It does not assert that the
current file content is identical to the code that emitted the span.

## Bounds and lifecycle

- maximum request headers: 16 KiB;
- maximum compressed or decompressed body: 2 MiB;
- maximum spans decoded from one export: 1,024;
- maximum unique spans in one active receiver session: 8,192;
- maximum concurrent receiver connections: eight;
- connection deadline: ten seconds;
- general runtime event tail: 2,000 events;
- renderer runtime graph projection: newest 80 relevant events;
- debugger execution path: fixed 120-safe-point pages.

The receiver is single-workspace and in-memory. Stop closes the listener and
zeroizes the active token. Switching workspaces stops it and clears all runtime
events. **Delete trace** asks for confirmation and removes only events with the
selected trace ID from the current in-memory tail.

## Replay truth boundary

**Replay**, **Pause replay**, **Previous event**, and **Next event** move a
selection position over retained observations. Replay selection is visually
distinct from the live current-safe-point highlight. Replay does not call the
target, execute source, restore application state, or move a process backward.

Cooperative `AONE_DEBUG_V1` controls remain separate. A live current safe point
uses its own state and styling; selecting an OTLP trace or historical workflow
cannot become a live pause/continue authority.

## Unsupported

- OTLP/gRPC or OTLP/HTTP protobuf;
- metrics, logs, and profiles;
- TLS, remote bind addresses, collector pipelines, exporters, sampling, or disk persistence;
- arbitrary attribute browsing or raw payload/query inspection;
- automatic browser/DOM interception;
- generic stack frames, variables, breakpoints, reverse execution, or time travel;
- complete distributed-service topology or guaranteed source correlation.

For a full multi-service example to compare SDK wiring and semantic coverage,
use the upstream [OpenTelemetry Demo](https://github.com/open-telemetry/opentelemetry-demo)
as a reference system. Aone does not vendor or claim parity with that demo.
