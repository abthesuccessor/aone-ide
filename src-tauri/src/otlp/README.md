# Local OTLP receiver

This feature implements an opt-in, loopback-only OTLP/HTTP JSON trace receiver
for the currently opened workspace. It is intentionally smaller than an
OpenTelemetry Collector and does not replace one.

## Trust and retention boundary

- Rust binds only `127.0.0.1` after native user confirmation.
- Every request requires a fresh 64-hex-character `x-aone-ingest-token` value.
- Only `POST /v1/traces` with `Content-Type: application/json`, identity or gzip
  encoding, a declared content length, and a body no larger than 2 MiB is read.
- One export accepts at most 1,024 spans; one receiver session retains at most
  8,192 unique span identities and eight simultaneous connections.
- Unknown OTLP JSON fields are ignored as required by OTLP JSON. Aone itself
  retains only allowlisted service, code, HTTP-route, database-identifier, and
  messaging-identifier attributes. Span events, links, status messages, URL
  values, queries, bodies, raw database statements, and arbitrary attributes
  are never copied into `RuntimeEvent` metadata.
- Trace state is in memory, cleared on workspace replacement or app exit, and
  deletable by trace ID. No OTLP value enters SQLite or AI evidence.

## Correlation truth

`code.file.path` is converted to a non-sensitive workspace-relative path.
`code.line.number` maps to a static symbol only when exactly one indexed symbol
contains that line. The resulting edge remains inferred candidate correlation;
neither OTLP nor source coordinates upgrade static evidence to runtime proof.

The Aone API client may inject W3C `traceparent` only when this receiver is
active and every resolved destination address is loopback or private. Public
destinations never receive Aone-generated trace context.

## Unsupported receiver surfaces

OTLP/HTTP protobuf, OTLP/gRPC, metrics, logs, profiles, TLS, remote binding,
Collector pipelines, tail sampling, disk persistence, distributed debugger
control, raw payload capture, and reverse execution are not implemented.
