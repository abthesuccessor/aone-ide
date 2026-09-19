use std::collections::BTreeMap;

use serde_json::json;

use crate::domain::{EvidenceKind, GraphEdge, GraphNode};

use super::{api::HttpHandler, ids::stable_id};

pub(super) struct HttpHandlerIdentity<'a> {
    pub(super) workspace_id: &'a str,
    pub(super) relative_path: &'a str,
    pub(super) content_hash: &'a str,
    pub(super) language_name: &'a str,
    pub(super) endpoint_id: &'a str,
}

pub(super) fn handler_fact(
    identity: HttpHandlerIdentity<'_>,
    handler: &HttpHandler,
    confidence: f64,
) -> (GraphNode, GraphEdge) {
    let start_line = handler.source.start_line.to_string();
    let start_column = handler.source.start_column.to_string();
    let handler_id = stable_id(
        "http-handler",
        &[
            identity.workspace_id,
            identity.relative_path,
            identity.content_hash,
            identity.endpoint_id,
            &handler.label,
            &start_line,
            &start_column,
        ],
    );
    let node = GraphNode {
        id: handler_id.clone(),
        kind: handler.kind.into(),
        label: handler.label.clone(),
        source: Some(handler.source.clone()),
        language: Some(identity.language_name.into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::from([
            ("astKind".into(), json!(handler.ast_kind.as_str())),
            ("contentHash".into(), json!(identity.content_hash)),
            ("parser".into(), json!("tree-sitter")),
            ("sourceExact".into(), json!(true)),
            ("referenceOnly".into(), json!(handler.reference_only)),
        ]),
    };
    let edge = GraphEdge {
        id: stable_id("edge", &[&handler_id, identity.endpoint_id, "handles"]),
        source: handler_id,
        target: identity.endpoint_id.into(),
        kind: "handles".into(),
        evidence: EvidenceKind::Inferred,
        confidence: Some(confidence),
        metadata: BTreeMap::from([(
            "projectionBasis".into(),
            json!("static route handler argument"),
        )]),
    };
    (node, edge)
}
