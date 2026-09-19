use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

use serde_json::{Value, json};

use crate::{
    domain::{EvidenceKind, GraphEdge, GraphNode, SourceLocation},
    error::{AoneError, AoneResult},
};

use super::{AnalyzedSource, ids::stable_id, text::MAX_LABEL_BYTES};

const HTTP_METHODS: [&str; 8] = [
    "delete", "get", "head", "options", "patch", "post", "put", "trace",
];
const MAX_ROUTE_BYTES: usize = 1_024;
const MAX_METADATA_TEXT_BYTES: usize = 240;
const MAX_OPENAPI_SOURCE_BYTES: usize = 2 * 1024 * 1024;

pub(super) fn extract_openapi(
    workspace_id: &str,
    relative_path: &str,
    source: &str,
    content_hash: &str,
    file_node: GraphNode,
    deadline: Option<Instant>,
) -> AoneResult<Option<AnalyzedSource>> {
    check_deadline(deadline)?;
    if !looks_like_openapi(relative_path, source) {
        return Ok(None);
    }
    let document = parse_document(relative_path, source);
    check_deadline(deadline)?;
    let Some(document) = document else {
        return Ok(None);
    };
    if document.get("openapi").is_none() && document.get("swagger").is_none() {
        return Ok(None);
    }
    let Some(paths) = document.get("paths").and_then(Value::as_object) else {
        return Ok(None);
    };
    check_deadline(deadline)?;

    let title = bounded_text(
        document
            .pointer("/info/title")
            .and_then(Value::as_str)
            .unwrap_or("OpenAPI service"),
    );
    let version = document
        .pointer("/info/version")
        .and_then(Value::as_str)
        .map(bounded_text);
    let service_id = stable_id("api-service", &[workspace_id, relative_path, &title]);
    let service_root = spec_service_root(relative_path);
    let mut service_metadata = base_metadata(content_hash);
    service_metadata.insert("apiFramework".into(), json!("OpenAPI"));
    service_metadata.insert("specTitle".into(), json!(title));
    service_metadata.insert("apiServiceRoot".into(), json!(service_root));
    if let Some(version) = version.as_ref() {
        service_metadata.insert("specVersion".into(), json!(version));
    }
    let service = GraphNode {
        id: service_id.clone(),
        kind: "apiService".into(),
        label: title.clone(),
        source: file_node.source.clone(),
        language: Some(spec_language(relative_path).into()),
        evidence: EvidenceKind::Declared,
        metadata: service_metadata,
    };
    let mut nodes = vec![file_node, service];
    let mut edges = vec![edge(&nodes[0].id, &service_id, "contains")];
    let mut groups = BTreeMap::<String, String>::new();
    let paths_start = source.find("paths").unwrap_or(0);
    let mut truncated = false;

    'paths: for (path, path_item) in paths {
        check_deadline(deadline)?;
        if path.len() > MAX_ROUTE_BYTES
            || !path.starts_with('/')
            || path.contains(['?', '#'])
            || path.chars().any(char::is_control)
        {
            continue;
        }
        let Some(operations) = resolve_local_path_item(&document, path_item).as_object() else {
            continue;
        };
        for method in HTTP_METHODS {
            let Some(operation) = operations.get(method).and_then(Value::as_object) else {
                continue;
            };
            let primary_tag = operation
                .get("tags")
                .and_then(Value::as_array)
                .and_then(|tags| tags.iter().find_map(Value::as_str))
                .map_or_else(|| "Untagged".into(), bounded_text);
            let group_id = groups
                .entry(primary_tag.clone())
                .or_insert_with(|| stable_id("api-group", &[&service_id, &primary_tag]))
                .clone();
            if !nodes.iter().any(|node| node.id == group_id) {
                if nodes.len().saturating_sub(1) >= super::extract::MAX_FACTS_PER_FILE {
                    truncated = true;
                    break 'paths;
                }
                let mut metadata = base_metadata(content_hash);
                metadata.insert("apiFramework".into(), json!("OpenAPI"));
                metadata.insert("specTitle".into(), json!(title));
                metadata.insert("apiGroup".into(), json!(primary_tag));
                metadata.insert("apiServiceRoot".into(), json!(service_root));
                nodes.push(GraphNode {
                    id: group_id.clone(),
                    kind: "apiGroup".into(),
                    label: primary_tag.clone(),
                    source: Some(path_source_location(
                        source,
                        relative_path,
                        paths_start,
                        path,
                    )),
                    language: Some(spec_language(relative_path).into()),
                    evidence: EvidenceKind::Declared,
                    metadata,
                });
                edges.push(edge(&service_id, &group_id, "contains"));
            }

            if nodes.len().saturating_sub(1) >= super::extract::MAX_FACTS_PER_FILE {
                truncated = true;
                break 'paths;
            }

            let method_upper = method.to_ascii_uppercase();
            let endpoint_id = stable_id(
                "openapi-endpoint",
                &[workspace_id, relative_path, &method_upper, path],
            );
            let mut metadata = base_metadata(content_hash);
            metadata.insert("apiFramework".into(), json!("OpenAPI"));
            metadata.insert("apiRole".into(), json!("producer"));
            metadata.insert("protocol".into(), json!("http"));
            metadata.insert("httpMethod".into(), json!(method_upper));
            metadata.insert("routePath".into(), json!(path));
            metadata.insert("specTitle".into(), json!(title));
            metadata.insert("apiGroup".into(), json!(primary_tag));
            metadata.insert("apiServiceRoot".into(), json!(service_root));
            let (operation_source, source_exact) =
                operation_source_location(source, relative_path, paths_start, path, method);
            metadata.insert("sourceExact".into(), json!(source_exact));
            metadata.insert("sourceKind".into(), json!("openApiOperationKey"));
            if let Some(operation_id) = operation.get("operationId").and_then(Value::as_str) {
                metadata.insert("operationId".into(), json!(bounded_text(operation_id)));
            }
            nodes.push(GraphNode {
                id: endpoint_id.clone(),
                kind: "endpoint".into(),
                label: bounded_text(&format!("{method_upper} {path}")),
                source: Some(operation_source),
                language: Some(spec_language(relative_path).into()),
                evidence: EvidenceKind::Declared,
                metadata,
            });
            edges.push(edge(&group_id, &endpoint_id, "contains"));
        }
    }
    if nodes.len() == 2 {
        return Ok(None);
    }
    let fact_count = nodes.len().saturating_sub(1);
    if let Some(file) = nodes.first_mut() {
        file.metadata
            .insert("analysisTruncated".into(), json!(truncated));
        file.metadata
            .insert("factsExtracted".into(), json!(fact_count));
        if truncated {
            file.metadata.insert(
                "analysisTruncationReasons".into(),
                json!(["per-file fact limit reached"]),
            );
        }
    }
    Ok(Some(AnalyzedSource {
        nodes,
        edges,
        parse_errors: truncated,
        fact_count,
    }))
}

fn resolve_local_path_item<'a>(document: &'a Value, mut item: &'a Value) -> &'a Value {
    let mut visited = BTreeSet::new();
    for _ in 0..8 {
        let Some(reference) = item.get("$ref").and_then(Value::as_str) else {
            break;
        };
        let Some(pointer) = reference.strip_prefix('#') else {
            break;
        };
        if !pointer.starts_with('/') || !visited.insert(pointer.to_owned()) {
            break;
        }
        let Some(resolved) = document.pointer(pointer) else {
            break;
        };
        item = resolved;
    }
    item
}

fn parse_document(relative_path: &str, source: &str) -> Option<Value> {
    match Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => serde_json::from_str(source).ok(),
        "yaml" | "yml" => serde_yaml_ng::from_str(source).ok(),
        _ => None,
    }
}

fn looks_like_openapi(relative_path: &str, source: &str) -> bool {
    if source.len() > MAX_OPENAPI_SOURCE_BYTES {
        return false;
    }
    let extension = Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "json" => {
            (source.contains("\"openapi\"") || source.contains("\"swagger\""))
                && source.contains("\"paths\"")
        }
        "yaml" | "yml" => {
            (source.contains("openapi:") || source.contains("swagger:"))
                && source.contains("paths:")
        }
        _ => false,
    }
}

fn check_deadline(deadline: Option<Instant>) -> AoneResult<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(AoneError::InvalidRequest(
            "workspace scan exceeded its wall-clock budget during OpenAPI analysis".into(),
        ));
    }
    Ok(())
}

fn base_metadata(content_hash: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("contentHash".into(), json!(content_hash)),
        ("parser".into(), json!("openapi")),
    ])
}

fn edge(source: &str, target: &str, kind: &str) -> GraphEdge {
    GraphEdge {
        id: stable_id("edge", &[source, target, kind]),
        source: source.into(),
        target: target.into(),
        kind: kind.into(),
        evidence: EvidenceKind::Declared,
        confidence: Some(1.0),
        metadata: BTreeMap::new(),
    }
}

fn path_source_location(
    source: &str,
    relative_path: &str,
    search_start: usize,
    path: &str,
) -> SourceLocation {
    let offset = source
        .get(search_start..)
        .and_then(|suffix| suffix.find(path).map(|index| search_start + index))
        .unwrap_or(0);
    let prefix = &source[..offset.min(source.len())];
    let start_line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let start_column = source[line_start..offset].encode_utf16().count() + 1;
    SourceLocation {
        relative_path: relative_path.into(),
        start_line,
        start_column,
        end_line: start_line,
        end_column: start_column + path.encode_utf16().count(),
    }
}

fn operation_source_location(
    source: &str,
    relative_path: &str,
    search_start: usize,
    path: &str,
    method: &str,
) -> (SourceLocation, bool) {
    let path_offset = source
        .get(search_start..)
        .and_then(|suffix| suffix.find(path).map(|index| search_start + index));
    let Some(path_offset) = path_offset else {
        return (
            path_source_location(source, relative_path, search_start, path),
            false,
        );
    };
    let path_line_start = source[..path_offset]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let path_indent = source[path_line_start..]
        .chars()
        .take_while(|value| value.is_whitespace() && *value != '\n')
        .count();
    let mut line_start = source[path_offset..]
        .find('\n')
        .map_or(source.len(), |index| path_offset + index + 1);
    while line_start < source.len() {
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |index| line_start + index);
        let line = &source[line_start..line_end];
        let indent = line
            .chars()
            .take_while(|value| value.is_whitespace())
            .count();
        let trimmed = line.trim_start();
        if !trimmed.is_empty() && indent <= path_indent {
            break;
        }
        let json_key = format!("\"{method}\"");
        let yaml_key = format!("{method}:");
        if trimmed.starts_with(&json_key) || trimmed.starts_with(&yaml_key) {
            let key_offset = line_start + indent + usize::from(trimmed.starts_with('"'));
            return (
                location_from_offset(source, relative_path, key_offset, method.len()),
                true,
            );
        }
        line_start = line_end.saturating_add(1);
    }
    (
        path_source_location(source, relative_path, search_start, path),
        false,
    )
}

fn location_from_offset(
    source: &str,
    relative_path: &str,
    offset: usize,
    length: usize,
) -> SourceLocation {
    let prefix = &source[..offset.min(source.len())];
    let start_line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let start_column = source[line_start..offset].encode_utf16().count() + 1;
    SourceLocation {
        relative_path: relative_path.into(),
        start_line,
        start_column,
        end_line: start_line,
        end_column: start_column + length,
    }
}

fn spec_language(relative_path: &str) -> &'static str {
    if relative_path.ends_with(".json") {
        "JSON"
    } else {
        "YAML"
    }
}

fn spec_service_root(relative_path: &str) -> String {
    let path = Path::new(relative_path);
    if path.starts_with(Path::new("openapi")) {
        return "Workspace root".into();
    }
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .and_then(Path::to_str)
        .map_or_else(|| "Workspace root".into(), str::to_owned)
}

fn bounded_text(value: &str) -> String {
    let maximum = MAX_LABEL_BYTES.min(MAX_METADATA_TEXT_BYTES);
    if value.len() <= maximum {
        return value.into();
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
