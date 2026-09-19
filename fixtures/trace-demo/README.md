# Cooperative execution-debug fixture

This dependency-free Node fixture gives Aone a real local flow to index and execute:

```text
client fetch -> HTTP route -> shipment service -> shipment repository -> in-memory datastore
```

Run it with `npm start`, then request
`http://127.0.0.1:4310/api/shipments/SHP-1047` from Aone's API panel.

When Aone starts this profile with `debug: true` and `observe: true`, it injects
the private nonce/session environment and reserves the managed child's stdin.
The fixture then emits nonce-bound `AONE_DEBUG_V1` checkpoints for:

```text
HTTP request -> route -> service -> repository -> data shape -> branch -> response
```

The adapter pauses the first request at its first checkpoint. `stepInto` or
`stepOver` releases exactly one cooperative checkpoint; `resume` continues the
serialized workflow; `pause` takes effect at the next checkpoint. A second
request may run after the first completes and can reuse stable checkpoint IDs.
Concurrent requests wait behind the active instrumented workflow because v1
does not claim thread-wide or multi-request control.

Run the deterministic child-process proof with:

```bash
npm run test:debug
```

That test verifies acknowledged pause/no progress, exactly-one checkpoint per
step, resume, sequential requests with reused checkpoint IDs, a thrown workflow
reported as `workflowFailed` without wedging the next request, a 122-checkpoint
burst that remains pause-responsive, and an adapter that ignores cooperative
Stop until an OS process-group signal terminates it.

This is not DAP, OS suspension, browser interception, or automatic
line-by-line observation. The claimed source, operation, schema names, and
checkpoint state are process-reported by this explicit adapter. This fixture
sends only bounded static identifiers and shape/type/count previews and does not
put shipment IDs, bodies, headers, environment values, query text, or binds in
them. The protocol has no dedicated raw-value/query fields, but Aone cannot
independently prove a reported name is value-free; instrumentation authors must
not place values or personal data in identifier/schema fields.

## OTLP/HTTP JSON replay

The same fixture includes `otlp-trace.json`, a deterministic six-span trace for
frontend, API route, service, repository, database, and response operations.
Start Aone's **Local OTLP trace receiver**, copy the three displayed exporter
variables into a shell, and run:

```bash
npm run send:otlp
```

The receiver accepts the standard OTLP JSON envelope and maps the reported
`code.file.path` and `code.line.number` candidates to this fixture. It retains
semantic identifiers and timing/status fields only; the sample deliberately
contains no request body, database statement, bind, event payload, or arbitrary
attribute value. Replay moves a UI selection token over retained observations;
it does not re-execute or reverse an application.
