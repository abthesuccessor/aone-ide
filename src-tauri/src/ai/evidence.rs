use serde_json::Value;
use zeroize::Zeroizing;

use super::state::SecretState;
use crate::{
    domain::{AiExplainRequest, EvidenceReference},
    error::{AoneError, AoneResult},
    runner::{is_sensitive_name, redact_json, redact_text},
};

pub(super) const MAX_EVIDENCE_ITEMS: usize = 24;
const MAX_NODE_METADATA_BYTES: usize = 1_200;
const MAX_EDGE_METADATA_BYTES: usize = 600;
pub(super) const MAX_EVIDENCE_INPUT_BYTES: usize = 32 * 1024;

#[derive(Debug)]
pub(super) struct BoundedEvidence {
    pub(super) input: String,
    pub(super) references: Vec<EvidenceReference>,
}

pub(super) fn build_evidence(
    request: &AiExplainRequest,
    secrets: &SecretState,
    redaction_values: &[Zeroizing<String>],
) -> AoneResult<BoundedEvidence> {
    if !request.runtime_event_ids.is_empty() {
        return Err(AoneError::InvalidRequest(
            "runtime event evidence requires event-specific provider consent and is not supported"
                .into(),
        ));
    }
    let node_limit = request.node_ids.len().min(MAX_EVIDENCE_ITEMS);
    let nodes = if request.node_ids.is_empty() {
        Vec::new()
    } else {
        secrets.selected_nodes(&request.workspace_id, &request.node_ids, node_limit)?
    };
    let mut input = String::new();
    let mut references = Vec::with_capacity(nodes.len());
    for node in nodes {
        let citation = format!("node:{}", node.id);
        let label = provider_safe_text(&node.label, 300, redaction_values);
        let source = node
            .source
            .as_ref()
            .map(|source| {
                let relative_path =
                    provider_safe_text(&source.relative_path, 1_024, redaction_values);
                format!(
                    "{}:{}:{}-{}:{}",
                    relative_path,
                    source.start_line,
                    source.start_column,
                    source.end_line,
                    source.end_column
                )
            })
            .unwrap_or_else(|| "no source location".into());
        let metadata = bounded_json(
            &Value::Object(node.metadata.clone().into_iter().collect()),
            MAX_NODE_METADATA_BYTES,
            redaction_values,
        );
        let line = format!(
            "[{citation}] graph node; kind={}; label={}; evidence={:?}; source={source}; metadata={metadata}\n",
            provider_safe_text(&node.kind, 120, redaction_values),
            label,
            node.evidence,
        );
        if !append_bounded(&mut input, &line, MAX_EVIDENCE_INPUT_BYTES) {
            break;
        }
        references.push(EvidenceReference {
            kind: "graphNode".into(),
            id: node.id,
            label,
            evidence: node.evidence,
        });
    }

    let selected_node_ids = references
        .iter()
        .filter(|reference| reference.kind == "graphNode")
        .map(|reference| reference.id.clone())
        .collect::<Vec<_>>();
    let remaining = MAX_EVIDENCE_ITEMS.saturating_sub(references.len());
    for edge in secrets.selected_edges(&request.workspace_id, &selected_node_ids, remaining)? {
        let citation = format!("edge:{}", edge.id);
        let kind = provider_safe_text(&edge.kind, 120, redaction_values);
        let metadata = bounded_json(
            &Value::Object(edge.metadata.clone().into_iter().collect()),
            MAX_EDGE_METADATA_BYTES,
            redaction_values,
        );
        let confidence = edge
            .confidence
            .map_or_else(|| "not supplied".into(), |value| format!("{value:.3}"));
        let line = format!(
            "[{citation}] graph edge; source=node:{}; target=node:{}; kind={kind}; evidence={:?}; confidence={confidence}; metadata={metadata}\n",
            edge.source, edge.target, edge.evidence,
        );
        if !append_bounded(&mut input, &line, MAX_EVIDENCE_INPUT_BYTES) {
            break;
        }
        references.push(EvidenceReference {
            kind: "graphEdge".into(),
            id: edge.id,
            label: format!("{} --{kind}--> {}", edge.source, edge.target),
            evidence: edge.evidence,
        });
    }

    if references.is_empty() {
        return Err(AoneError::InvalidRequest(
            "select at least one available graph node as evidence".into(),
        ));
    }

    Ok(BoundedEvidence { input, references })
}

fn bounded_json(value: &Value, max_bytes: usize, redaction_values: &[Zeroizing<String>]) -> String {
    let value = redact_json(value, redaction_values);
    let value = redact_urls_in_json(&value);
    let serialized = serde_json::to_string(&value).unwrap_or_else(|_| "{}".into());
    bounded_text(&serialized, max_bytes)
}

pub(super) fn provider_safe_text(
    value: &str,
    max_bytes: usize,
    redaction_values: &[Zeroizing<String>],
) -> String {
    let redacted = redact_text(value, redaction_values);
    let redacted = redact_url_userinfo(&redacted);
    let redacted = redact_sensitive_url_parameters(&redacted);
    bounded_text(&redacted, max_bytes)
}

fn redact_urls_in_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), redact_urls_in_json(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact_urls_in_json).collect()),
        Value::String(value) => {
            Value::String(redact_sensitive_url_parameters(&redact_url_userinfo(value)))
        }
        _ => value.clone(),
    }
}

/// Removes URL user-info without needing to parse arbitrary log prose as a
/// complete URL. The host and path remain available for flow explanation.
fn redact_url_userinfo(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative_scheme) = value[cursor..].find("://") {
        let scheme_end = cursor + relative_scheme + 3;
        output.push_str(&value[cursor..scheme_end]);
        let authority_end = value[scheme_end..]
            .find(|character: char| {
                character == '/'
                    || character == '?'
                    || character == '#'
                    || character.is_whitespace()
                    || matches!(character, '\'' | '"' | ')' | ']' | '}')
            })
            .map_or(value.len(), |offset| scheme_end + offset);
        if let Some(at_offset) = value[scheme_end..authority_end].rfind('@') {
            output.push_str("[REDACTED]@");
            cursor = scheme_end + at_offset + 1;
        } else {
            cursor = scheme_end;
        }
    }
    output.push_str(&value[cursor..]);
    output
}

/// Redacts values for sensitive URL query keys while preserving the endpoint
/// shape (`?token=[REDACTED]`) needed to understand an API flow.
fn redact_sensitive_url_parameters(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    let bytes = value.as_bytes();

    while cursor < bytes.len() {
        let Some(relative_separator) = value[cursor..].find(['?', '&']) else {
            output.push_str(&value[cursor..]);
            break;
        };
        let separator = cursor + relative_separator;
        output.push_str(&value[cursor..=separator]);
        let key_start = separator + 1;
        let key_end = value[key_start..]
            .find(|character: char| {
                character == '='
                    || character == '&'
                    || character == '#'
                    || character.is_whitespace()
                    || matches!(character, '\'' | '"' | ')' | ']' | '}')
            })
            .map_or(value.len(), |offset| key_start + offset);

        if key_end >= value.len() || bytes[key_end] != b'=' {
            cursor = key_start;
            continue;
        }

        output.push_str(&value[key_start..=key_end]);
        let value_start = key_end + 1;
        let value_end = value[value_start..]
            .find(|character: char| {
                character == '&'
                    || character == '#'
                    || character.is_whitespace()
                    || matches!(character, '\'' | '"' | ')' | ']' | '}')
            })
            .map_or(value.len(), |offset| value_start + offset);
        if is_sensitive_name(&value[key_start..key_end]) {
            output.push_str("[REDACTED]");
        } else {
            output.push_str(&value[value_start..value_end]);
        }
        cursor = value_end;
    }

    output
}

pub(super) fn bounded_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}…", &value[..boundary])
}

fn append_bounded(target: &mut String, value: &str, max_bytes: usize) -> bool {
    if target.len().saturating_add(value.len()) > max_bytes {
        return false;
    }
    target.push_str(value);
    true
}
