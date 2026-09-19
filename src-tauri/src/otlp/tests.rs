use super::wire::ExportTraceServiceRequest;

#[test]
fn decodes_a_bounded_semantic_otlp_trace_without_raw_values() {
    let request: ExportTraceServiceRequest = serde_json::from_value(serde_json::json!({
        "resourceSpans": [{
            "resource": { "attributes": [
                { "key": "service.name", "value": { "stringValue": "checkout" } },
                { "key": "secret.token", "value": { "stringValue": "must-not-survive" } }
            ] },
            "scopeSpans": [{ "spans": [{
                "traceId": "5B8EFFF798038103D269B633813FC60C",
                "spanId": "EEE19B7EC3C1B174",
                "name": "customer-secret-value-must-not-survive",
                "kind": 2,
                "startTimeUnixNano": "1544712660000000000",
                "endTimeUnixNano": "1544712661000000000",
                "attributes": [
                    { "key": "http.request.method", "value": { "stringValue": "POST" } },
                    { "key": "http.route", "value": { "stringValue": "/checkout" } },
                    { "key": "code.file.path", "value": { "stringValue": "src/checkout.ts" } },
                    { "key": "code.line.number", "value": { "intValue": "42" } },
                    { "key": "http.request.body", "value": { "stringValue": "private-body" } }
                ]
            }] }]
        }]
    }))
    .unwrap();
    let decoded = request.decode();
    assert_eq!(decoded.rejected, 0);
    assert_eq!(decoded.spans.len(), 1);
    let span = &decoded.spans[0];
    assert_eq!(span.service_name.as_deref(), Some("checkout"));
    assert_eq!(span.http_route.as_deref(), Some("/checkout"));
    assert_eq!(span.source_line, Some(42));
    let debug = format!("{span:?}");
    assert!(!debug.contains("must-not-survive"));
    assert!(!debug.contains("private-body"));
    assert!(!debug.contains("customer-secret-value"));
}

#[test]
fn rejects_invalid_ids_timestamps_and_self_parenting_independently() {
    let request: ExportTraceServiceRequest = serde_json::from_value(serde_json::json!({
        "resourceSpans": [{ "scopeSpans": [{ "spans": [
            {
                "traceId": "00000000000000000000000000000000",
                "spanId": "EEE19B7EC3C1B174",
                "name": "invalid trace",
                "startTimeUnixNano": "10",
                "endTimeUnixNano": "20"
            },
            {
                "traceId": "5B8EFFF798038103D269B633813FC60C",
                "spanId": "EEE19B7EC3C1B174",
                "parentSpanId": "EEE19B7EC3C1B174",
                "name": "self parent",
                "startTimeUnixNano": "10",
                "endTimeUnixNano": "20"
            }
        ] }] }]
    }))
    .unwrap();
    let decoded = request.decode();
    assert_eq!(decoded.spans.len(), 0);
    assert_eq!(decoded.rejected, 2);
}
