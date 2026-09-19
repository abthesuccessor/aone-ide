use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

const MAX_SPANS_PER_EXPORT: usize = 1_024;
const MAX_NAME_CHARS: usize = 240;
const MAX_ATTRIBUTE_CHARS: usize = 1_024;
const MAX_DURATION_NANOS: u64 = 86_400_000_000_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ExportTraceServiceRequest {
    #[serde(default)]
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceSpans {
    #[serde(default)]
    resource: Resource,
    #[serde(default)]
    scope_spans: Vec<ScopeSpans>,
}

#[derive(Debug, Default, Deserialize)]
struct Resource {
    #[serde(default)]
    attributes: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopeSpans {
    #[serde(default)]
    spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Span {
    #[serde(default)]
    trace_id: String,
    #[serde(default)]
    span_id: String,
    #[serde(default)]
    parent_span_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: i32,
    #[serde(default)]
    start_time_unix_nano: Value,
    #[serde(default)]
    end_time_unix_nano: Value,
    #[serde(default)]
    attributes: Vec<KeyValue>,
    #[serde(default)]
    status: SpanStatus,
}

#[derive(Debug, Default, Deserialize)]
struct SpanStatus {
    #[serde(default)]
    code: i32,
}

#[derive(Debug, Deserialize)]
struct KeyValue {
    key: String,
    value: AnyValue,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnyValue {
    #[serde(default)]
    string_value: Option<String>,
    #[serde(default)]
    int_value: Option<Value>,
    #[serde(default)]
    double_value: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SpanCategory {
    HttpServer,
    HttpClient,
    Database,
    Event,
    External,
    Function,
}

impl SpanCategory {
    pub(super) fn event_kind(self) -> &'static str {
        match self {
            Self::HttpServer => "trace.http.server.event",
            Self::HttpClient => "trace.http.client.event",
            Self::Database => "trace.database.event",
            Self::Event => "trace.event.event",
            Self::External => "trace.external.event",
            Self::Function => "trace.function.event",
        }
    }

    pub(super) fn flow_stage(self) -> &'static str {
        match self {
            Self::HttpServer => "api",
            Self::HttpClient | Self::External => "external",
            Self::Database => "data",
            Self::Event => "service",
            Self::Function => "service",
        }
    }
}

#[derive(Debug)]
pub(super) struct DecodedSpan {
    pub(super) trace_id: String,
    pub(super) span_id: String,
    pub(super) parent_span_id: Option<String>,
    pub(super) category: SpanCategory,
    pub(super) flow_stage: Option<&'static str>,
    pub(super) start_unix_nanos: u64,
    pub(super) duration_ms: u64,
    pub(super) status: &'static str,
    pub(super) service_name: Option<String>,
    pub(super) service_namespace: Option<String>,
    pub(super) source_path: Option<String>,
    pub(super) source_line: Option<usize>,
    pub(super) function_name: Option<String>,
    pub(super) http_method: Option<String>,
    pub(super) http_route: Option<String>,
    pub(super) db_system: Option<String>,
    pub(super) db_operation: Option<String>,
    pub(super) db_collection: Option<String>,
    pub(super) messaging_system: Option<String>,
    pub(super) messaging_operation: Option<String>,
}

pub(super) struct DecodedExport {
    pub(super) spans: Vec<DecodedSpan>,
    pub(super) rejected: usize,
}

impl ExportTraceServiceRequest {
    pub(super) fn decode(self) -> DecodedExport {
        let mut decoded = Vec::new();
        let mut rejected = 0_usize;
        for resource_spans in self.resource_spans {
            let resource = string_attributes(&resource_spans.resource.attributes);
            for scope in resource_spans.scope_spans {
                for span in scope.spans {
                    if decoded.len() >= MAX_SPANS_PER_EXPORT {
                        rejected = rejected.saturating_add(1);
                        continue;
                    }
                    match decode_span(span, &resource) {
                        Some(span) => decoded.push(span),
                        None => rejected = rejected.saturating_add(1),
                    }
                }
            }
        }
        decoded.sort_by(|left, right| {
            left.start_unix_nanos
                .cmp(&right.start_unix_nanos)
                .then_with(|| left.trace_id.cmp(&right.trace_id))
                .then_with(|| left.span_id.cmp(&right.span_id))
        });
        DecodedExport {
            spans: decoded,
            rejected,
        }
    }
}

fn decode_span(span: Span, resource: &BTreeMap<String, String>) -> Option<DecodedSpan> {
    let trace_id = valid_hex_id(&span.trace_id, 32)?;
    let span_id = valid_hex_id(&span.span_id, 16)?;
    let parent_span_id = if span.parent_span_id.is_empty() {
        None
    } else {
        Some(valid_hex_id(&span.parent_span_id, 16)?)
    };
    if parent_span_id.as_deref() == Some(span_id.as_str()) {
        return None;
    }
    // A span name is required by OTLP, but it is deliberately not retained:
    // instrumentation can put high-cardinality runtime values in this field.
    bounded_identifier(&span.name, MAX_NAME_CHARS)?;
    let start_unix_nanos = json_u64(&span.start_time_unix_nano)?;
    let end_unix_nanos = json_u64(&span.end_time_unix_nano)?;
    let duration_nanos = end_unix_nanos.checked_sub(start_unix_nanos)?;
    if start_unix_nanos == 0 || duration_nanos > MAX_DURATION_NANOS {
        return None;
    }
    let attributes = string_attributes(&span.attributes);
    let db_system = attribute(&attributes, &["db.system.name", "db.system"]);
    let http_route = attribute(&attributes, &["http.route"]);
    let http_method = attribute(&attributes, &["http.request.method", "http.method"])
        .map(|method| method.to_ascii_uppercase());
    let messaging_system = attribute(&attributes, &["messaging.system"]);
    let flow_stage = attribute(&attributes, &["aone.flow.stage"])
        .as_deref()
        .and_then(valid_flow_stage);
    let category = if db_system.is_some() {
        SpanCategory::Database
    } else if messaging_system.is_some() || matches!(span.kind, 4 | 5) {
        SpanCategory::Event
    } else if span.kind == 2 || http_route.is_some() {
        SpanCategory::HttpServer
    } else if span.kind == 3 && http_method.is_some() {
        SpanCategory::HttpClient
    } else if span.kind == 3 {
        SpanCategory::External
    } else {
        SpanCategory::Function
    };
    Some(DecodedSpan {
        trace_id,
        span_id,
        parent_span_id,
        category,
        flow_stage,
        start_unix_nanos,
        duration_ms: duration_nanos / 1_000_000,
        status: if span.status.code == 2 || attributes.contains_key("error.type") {
            "error"
        } else if span.status.code == 1 {
            "ok"
        } else {
            "unset"
        },
        service_name: attribute(resource, &["service.name"]),
        service_namespace: attribute(resource, &["service.namespace"]),
        source_path: attribute(&attributes, &["code.file.path", "code.filepath"]),
        source_line: numeric_attribute(&span.attributes, &["code.line.number", "code.lineno"]),
        function_name: attribute(&attributes, &["code.function.name", "code.function"]),
        http_method,
        http_route,
        db_system,
        db_operation: attribute(&attributes, &["db.operation.name", "db.operation"]),
        db_collection: attribute(&attributes, &["db.collection.name"]),
        messaging_system,
        messaging_operation: attribute(
            &attributes,
            &["messaging.operation.type", "messaging.operation"],
        ),
    })
}

fn string_attributes(attributes: &[KeyValue]) -> BTreeMap<String, String> {
    attributes
        .iter()
        .filter_map(|attribute| {
            let value = attribute.value.string_value.as_deref()?;
            bounded_identifier(value, MAX_ATTRIBUTE_CHARS)
                .map(|value| (attribute.key.clone(), value))
        })
        .collect()
}

fn numeric_attribute(attributes: &[KeyValue], names: &[&str]) -> Option<usize> {
    attributes.iter().find_map(|attribute| {
        if !names.contains(&attribute.key.as_str()) {
            return None;
        }
        let value = attribute
            .value
            .int_value
            .as_ref()
            .and_then(json_u64)
            .or_else(|| {
                attribute
                    .value
                    .double_value
                    .filter(|value| value.fract() == 0.0 && *value > 0.0)
                    .map(|value| value as u64)
            })?;
        usize::try_from(value)
            .ok()
            .filter(|value| *value > 0 && *value <= 10_000_000)
    })
}

fn attribute(attributes: &BTreeMap<String, String>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| attributes.get(*name).cloned())
}

fn valid_flow_stage(value: &str) -> Option<&'static str> {
    match value {
        "trigger" => Some("trigger"),
        "api" => Some("api"),
        "backend" => Some("backend"),
        "service" => Some("service"),
        "data" => Some("data"),
        "external" => Some("external"),
        "response" => Some("response"),
        _ => None,
    }
}

fn bounded_identifier(value: &str, max_chars: usize) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > max_chars
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn valid_hex_id(value: &str, length: usize) -> Option<String> {
    (value.len() == length
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value.bytes().any(|byte| byte != b'0'))
    .then(|| value.to_ascii_lowercase())
}

fn json_u64(value: &Value) -> Option<u64> {
    match value {
        Value::String(value) => value.parse().ok(),
        Value::Number(value) => value.as_u64(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_value_shape_does_not_retain_arrays_or_objects() {
        let value: AnyValue = serde_json::from_value(serde_json::json!({
            "arrayValue": { "values": [{ "stringValue": "secret" }] },
            "kvlistValue": { "values": [{ "key": "password", "value": { "stringValue": "hidden" } }] }
        })).unwrap();
        assert!(value.string_value.is_none());
        assert!(value.int_value.is_none());
    }

    #[test]
    fn trace_ids_follow_otlp_hex_rules() {
        assert_eq!(
            valid_hex_id("5B8EFFF798038103D269B633813FC60C", 32).as_deref(),
            Some("5b8efff798038103d269b633813fc60c")
        );
        assert!(valid_hex_id("0000000000000000", 16).is_none());
        assert!(valid_hex_id("not-hex", 16).is_none());
    }

    #[test]
    fn flow_stage_hint_is_a_closed_presentation_enum() {
        assert_eq!(valid_flow_stage("trigger"), Some("trigger"));
        assert_eq!(valid_flow_stage("response"), Some("response"));
        assert_eq!(valid_flow_stage("frontend-user-42"), None);
    }
}
