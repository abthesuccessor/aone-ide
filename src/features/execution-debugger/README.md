# Instrumented execution debugger

This feature coordinates the focused four-pane API trace workbench: API client,
Monaco source, ordered execution path, and structured trace data. Selecting or
advancing a reported step opens its process-reported source candidate in the
replaceable preview editor; explicitly opening it pins the tab.

`AONE_DEBUG_V1` safe points provide the cooperative pause and control state.
`AONE_TRACE_V1` spans and HTTP boundaries remain replay-only. The execution path
shows only the selected API workflow in deterministic report order, caps each
page at 120 safe points, and performs no force simulation. Before runtime
evidence arrives, a bounded indexed path may appear muted; only an accepted
runtime report receives the live observed highlight.

When **Send** starts a trace, the renderer records the accepted sequence and all
workflow IDs that already exist. Later steps from those workflows are excluded,
so a paused older request cannot become the new request path. The cooperative
protocol permits one serialized workflow at a time; without an adapter-reported
request identity, a different newly queued request could still win a race and
must not be described as cryptographically correlated.

Captured previews use a shape-only protocol with no dedicated value, body,
header, raw-query, bind, or arbitrary-metadata fields. Names are bounded and
process-reported; known loaded secrets are rejected and sensitive field names
are replaced, but instrumentation authors must keep identifiers data-free
because Aone cannot independently verify their meaning.

The data pane keeps that boundary visible. **Step data** displays only reported
shape/type/count metadata and changes from the previous safe point. **Request**
and **Response** display the separate explicit API-client exchange, with bounded
JSON rendering and raw-text fallback; those values are not line-local variables
or proof of an internal source operation.
