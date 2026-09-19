use std::collections::BTreeMap;

use serde_json::json;
use tree_sitter::Node;

use crate::domain::{EvidenceKind, GraphEdge, GraphNode};

use super::{
    api::{
        HTTP_METHODS, HttpMetadata, RustEndpoint, bounded_api_label, is_relative_route,
        rust_endpoint_fact, rust_utoipa_endpoint,
    },
    ids::stable_id,
    syntax::location,
    text::compact_text,
};

const MAX_STATIC_ROUTE_BYTES: usize = 1_024;

pub(super) struct DefinitionHttpIdentity<'a> {
    pub(super) workspace_id: &'a str,
    pub(super) relative_path: &'a str,
    pub(super) content_hash: &'a str,
    pub(super) language_name: &'a str,
    pub(super) owner_id: &'a str,
}

pub(super) fn definition_http_facts(
    identity: DefinitionHttpIdentity<'_>,
    function: Node<'_>,
    source: &str,
) -> Vec<(GraphNode, GraphEdge)> {
    if identity.language_name == "Rust" {
        return rust_utoipa_endpoint(function, source, identity.relative_path)
            .map(|route| {
                rust_endpoint_fact(
                    identity.workspace_id,
                    identity.relative_path,
                    identity.content_hash,
                    identity.owner_id,
                    route,
                )
            })
            .into_iter()
            .collect();
    }
    if identity.language_name != "Python" || !has_fastapi_import(function, source) {
        return Vec::new();
    }
    python_routes(function, source, identity.relative_path)
        .into_iter()
        .map(|route| python_endpoint_fact(&identity, route))
        .collect()
}

fn python_routes(function: Node<'_>, source: &str, relative_path: &str) -> Vec<RustEndpoint> {
    let Some(decorated) = function
        .parent()
        .filter(|parent| parent.kind() == "decorated_definition")
    else {
        return Vec::new();
    };
    named_children(decorated)
        .into_iter()
        .filter(|node| node.kind() == "decorator")
        .filter_map(|decorator| python_route(decorator, source, relative_path))
        .collect()
}

fn python_route(decorator: Node<'_>, source: &str, relative_path: &str) -> Option<RustEndpoint> {
    let call = decorator.named_child(0)?;
    if call.kind() != "call" {
        return None;
    }
    let function = call.child_by_field_name("function")?;
    if function.kind() != "attribute" {
        return None;
    }
    let receiver = compact_text(function.child_by_field_name("object")?, source);
    let receiver = receiver
        .rsplit('.')
        .next()
        .unwrap_or(&receiver)
        .to_ascii_lowercase();
    if !matches!(receiver.as_str(), "app" | "router") && !receiver.ends_with("router") {
        return None;
    }
    let method =
        compact_text(function.child_by_field_name("attribute")?, source).to_ascii_lowercase();
    if !HTTP_METHODS.contains(&method.as_str()) || method == "connect" {
        return None;
    }
    let arguments = call.child_by_field_name("arguments")?;
    let literal = arguments.named_child(0)?;
    let path = static_python_string(literal, source)?;
    if !is_relative_route(&path) || path.contains(['?', '#']) {
        return None;
    }
    Some(RustEndpoint {
        method: method.to_ascii_uppercase(),
        path,
        operation_id: None,
        source: location(relative_path, literal, source),
    })
}

fn python_endpoint_fact(
    identity: &DefinitionHttpIdentity<'_>,
    route: RustEndpoint,
) -> (GraphNode, GraphEdge) {
    let endpoint_id = stable_id(
        "fastapi-endpoint",
        &[
            identity.workspace_id,
            identity.relative_path,
            &route.method,
            &route.path,
            identity.owner_id,
        ],
    );
    let mut endpoint = GraphNode {
        id: endpoint_id.clone(),
        kind: "endpoint".into(),
        label: bounded_api_label(&route.method, &route.path),
        source: Some(route.source),
        language: Some("Python".into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::from([
            ("astKind".into(), json!("decorator")),
            ("contentHash".into(), json!(identity.content_hash)),
            ("parser".into(), json!("tree-sitter")),
            ("declarationKind".into(), json!("fastApiRouteDecorator")),
            ("sourceExact".into(), json!(true)),
        ]),
    };
    HttpMetadata {
        role: "producer",
        method: Some(route.method),
        path: Some(route.path),
        framework: Some("FastAPI"),
        source: None,
        handler: None,
    }
    .decorate(&mut endpoint);
    let edge = GraphEdge {
        id: stable_id("edge", &[identity.owner_id, &endpoint_id, "handles"]),
        source: identity.owner_id.into(),
        target: endpoint_id,
        kind: "handles".into(),
        evidence: EvidenceKind::Declared,
        confidence: Some(1.0),
        metadata: BTreeMap::new(),
    };
    (endpoint, edge)
}

fn has_fastapi_import(function: Node<'_>, source: &str) -> bool {
    let mut root = function;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    named_children(root).into_iter().any(|node| {
        matches!(node.kind(), "import_statement" | "import_from_statement")
            && compact_text(node, source)
                .to_ascii_lowercase()
                .contains("fastapi")
    })
}

fn static_python_string(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    let raw = source.get(node.byte_range())?;
    if raw.len() < 2 || raw.len() > MAX_STATIC_ROUTE_BYTES + 2 {
        return None;
    }
    let delimiter = *raw.as_bytes().first()?;
    if !matches!(delimiter, b'\'' | b'"') || raw.as_bytes().last() != Some(&delimiter) {
        return None;
    }
    let value = &raw[1..raw.len() - 1];
    (!value.is_empty() && !value.contains('\\') && !value.chars().any(char::is_control))
        .then(|| value.to_owned())
}

fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}
