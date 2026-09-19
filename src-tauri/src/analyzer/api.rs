use std::collections::BTreeMap;

use reqwest::Url;
use serde_json::json;
use tree_sitter::Node;

use crate::domain::{EvidenceKind, GraphEdge, GraphNode, SourceLocation};

use super::{ids::stable_id, syntax::location, text::compact_text};

pub(super) const HTTP_METHODS: [&str; 9] = [
    "connect", "delete", "get", "head", "options", "patch", "post", "put", "trace",
];
const MAX_STATIC_ROUTE_BYTES: usize = 1_024;

pub(super) struct RustEndpoint {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) operation_id: Option<String>,
    pub(super) source: SourceLocation,
}

pub(super) struct HttpMetadata {
    pub(super) role: &'static str,
    pub(super) method: Option<String>,
    pub(super) path: Option<String>,
    pub(super) framework: Option<&'static str>,
    pub(super) source: Option<SourceLocation>,
    pub(super) handler: Option<HttpHandler>,
}

impl HttpMetadata {
    pub(super) fn decorate(&self, node: &mut GraphNode) {
        if let Some(source) = self.source.as_ref() {
            node.source = Some(source.clone());
            node.metadata.insert("sourceExact".into(), json!(true));
        }
        node.metadata.insert("apiRole".into(), json!(self.role));
        node.metadata.insert("protocol".into(), json!("http"));
        node.metadata.insert("targetSanitized".into(), json!(true));
        if let Some(method) = self.method.as_ref() {
            node.metadata.insert("httpMethod".into(), json!(method));
        }
        if let Some(path) = self.path.as_ref() {
            node.metadata.insert("routePath".into(), json!(path));
            node.metadata.insert(
                "apiTargetForm".into(),
                json!(if is_relative_route(path) {
                    "workspaceRelative"
                } else {
                    "absoluteHttp"
                }),
            );
        }
        if let Some(framework) = self.framework {
            node.metadata
                .insert("apiFramework".into(), json!(framework));
        }
        if let Some(source) = node.source.as_ref() {
            node.metadata.insert(
                "apiServiceRoot".into(),
                json!(code_service_root(&source.relative_path)),
            );
        }
    }
}

pub(super) struct HttpHandler {
    pub(super) kind: &'static str,
    pub(super) label: String,
    pub(super) source: SourceLocation,
    pub(super) ast_kind: String,
    pub(super) reference_only: bool,
}

fn code_service_root(relative_path: &str) -> String {
    if let Some(index) = relative_path.find("/src/") {
        return relative_path[..index].to_owned();
    }
    if relative_path.starts_with("src/") {
        return "Workspace root".into();
    }
    relative_path
        .split_once('/')
        .map_or_else(|| "Workspace root".into(), |(root, _)| root.into())
}

pub(super) struct HttpCall {
    pub(super) node_kind: &'static str,
    pub(super) edge_kind: &'static str,
    pub(super) label: String,
    pub(super) confidence: f64,
    pub(super) metadata: HttpMetadata,
}

pub(super) fn bounded_api_label(method: &str, path: &str) -> String {
    let value = format!("{method} {path}");
    if value.len() <= super::text::MAX_LABEL_BYTES {
        return value;
    }
    let mut end = super::text::MAX_LABEL_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

pub(super) fn sanitized_http_target(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_STATIC_ROUTE_BYTES
        || value.chars().any(char::is_control)
    {
        return None;
    }
    if is_relative_route(value) {
        let path = value.split(['?', '#']).next()?;
        return (!path.is_empty()).then(|| path.to_owned());
    }

    let mut url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    url.set_query(None);
    url.set_fragment(None);
    let sanitized = url.to_string();
    (sanitized.len() <= MAX_STATIC_ROUTE_BYTES).then_some(sanitized)
}

pub(super) fn is_relative_route(value: &str) -> bool {
    value.starts_with('/') && !value.starts_with("//")
}

pub(super) fn rust_utoipa_endpoint(
    function: Node<'_>,
    source: &str,
    relative_path: &str,
) -> Option<RustEndpoint> {
    let mut sibling = function.prev_named_sibling();
    while let Some(candidate) = sibling {
        if candidate.kind() == "attribute_item" {
            if let Some(endpoint) = endpoint_from_attribute(candidate, source, relative_path) {
                return Some(endpoint);
            }
        } else if !candidate.kind().ends_with("comment") {
            break;
        }
        sibling = candidate.prev_named_sibling();
    }
    None
}

pub(super) fn rust_endpoint_fact(
    workspace_id: &str,
    relative_path: &str,
    content_hash: &str,
    owner_id: &str,
    route: RustEndpoint,
) -> (GraphNode, GraphEdge) {
    let endpoint_id = stable_id(
        "utoipa-endpoint",
        &[
            workspace_id,
            relative_path,
            &route.method,
            &route.path,
            owner_id,
        ],
    );
    let mut endpoint = GraphNode {
        id: endpoint_id.clone(),
        kind: "endpoint".into(),
        label: bounded_api_label(&route.method, &route.path),
        source: Some(route.source),
        language: Some("Rust".into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::from([
            ("astKind".into(), json!("attribute_item")),
            ("contentHash".into(), json!(content_hash)),
            ("parser".into(), json!("tree-sitter")),
            ("declarationKind".into(), json!("utoipaPath")),
            ("sourceExact".into(), json!(true)),
        ]),
    };
    HttpMetadata {
        role: "producer",
        method: Some(route.method),
        path: Some(route.path),
        framework: Some("Utoipa"),
        source: None,
        handler: None,
    }
    .decorate(&mut endpoint);
    if let Some(operation_id) = route.operation_id {
        endpoint
            .metadata
            .insert("operationId".into(), json!(operation_id));
    }
    let edge = GraphEdge {
        id: stable_id("edge", &[owner_id, &endpoint_id, "handles"]),
        source: owner_id.into(),
        target: endpoint_id,
        kind: "handles".into(),
        evidence: EvidenceKind::Declared,
        confidence: Some(1.0),
        metadata: BTreeMap::new(),
    };
    (endpoint, edge)
}

fn endpoint_from_attribute(
    attribute_item: Node<'_>,
    source: &str,
    relative_path: &str,
) -> Option<RustEndpoint> {
    let attribute = attribute_item.named_child(0)?;
    let name = attribute.named_child(0)?;
    if compact_text(name, source) != "utoipa::path" {
        return None;
    }
    let arguments = attribute.child_by_field_name("arguments")?;
    let children = named_children(arguments);
    let method = children.iter().find_map(|child| {
        (child.kind() == "identifier")
            .then(|| compact_text(*child, source).to_ascii_lowercase())
            .filter(|value| HTTP_METHODS.contains(&value.as_str()))
    })?;
    let path_literal = value_after_key(&children, "path", source)?;
    let path = static_rust_string(path_literal, source)?;
    if !is_relative_route(&path) || path.contains(['?', '#']) || path.chars().any(char::is_control)
    {
        return None;
    }
    let operation_id = value_after_key(&children, "operation_id", source)
        .and_then(|literal| static_rust_string(literal, source));
    Some(RustEndpoint {
        method: method.to_ascii_uppercase(),
        path,
        operation_id,
        source: location(relative_path, path_literal, source),
    })
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

fn value_after_key<'tree>(
    children: &[Node<'tree>],
    key: &str,
    source: &str,
) -> Option<Node<'tree>> {
    let key_index = children
        .iter()
        .position(|child| child.kind() == "identifier" && compact_text(*child, source) == key)?;
    children
        .get(key_index + 1)
        .copied()
        .filter(|child| child.kind() == "string_literal")
}

fn static_rust_string(node: Node<'_>, source: &str) -> Option<String> {
    let raw = source.get(node.byte_range())?;
    if raw.len() > MAX_STATIC_ROUTE_BYTES || raw.chars().any(char::is_control) {
        return None;
    }
    if let Some(value) = raw
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return (!value.contains('\\')).then(|| value.to_owned());
    }
    let hashes = raw
        .strip_prefix('r')?
        .chars()
        .take_while(|value| *value == '#')
        .count();
    let prefix = format!("r{}\"", "#".repeat(hashes));
    let suffix = format!("\"{}", "#".repeat(hashes));
    raw.strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(&suffix))
        .map(str::to_owned)
}
