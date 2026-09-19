use std::collections::BTreeMap;

use super::{shortest_path, strongly_connected_components};
use crate::domain::{EvidenceKind, GraphEdge, GraphNode, GraphSnapshot};

fn node(id: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: "function".into(),
        label: id.into(),
        source: None,
        language: None,
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::new(),
    }
}

fn edge(source: &str, target: &str) -> GraphEdge {
    GraphEdge {
        id: format!("{source}-{target}"),
        source: source.into(),
        target: target.into(),
        kind: "calls".into(),
        evidence: EvidenceKind::Resolved,
        confidence: None,
        metadata: BTreeMap::new(),
    }
}

#[test]
fn bounded_projection_supports_path_and_cycle_algorithms() {
    let snapshot = GraphSnapshot {
        nodes: vec![node("a"), node("b"), node("c")],
        edges: vec![edge("a", "b"), edge("b", "c"), edge("c", "b")],
        truncated: false,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    };
    assert_eq!(
        shortest_path(&snapshot, "a", "c"),
        Some(vec!["a".into(), "b".into(), "c".into()])
    );
    assert_eq!(
        strongly_connected_components(&snapshot),
        vec![vec!["b".to_string(), "c".to_string()]]
    );
}
