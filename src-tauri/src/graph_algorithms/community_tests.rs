use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use super::annotate_structural_communities;
use crate::domain::{EvidenceKind, GraphEdge, GraphNode, GraphSnapshot};

fn node(id: &str, evidence: EvidenceKind) -> GraphNode {
    GraphNode {
        id: id.into(),
        kind: "function".into(),
        label: id.into(),
        source: None,
        language: None,
        evidence,
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
        confidence: Some(1.0),
        metadata: BTreeMap::new(),
    }
}

fn snapshot(truncated: bool) -> GraphSnapshot {
    GraphSnapshot {
        nodes: vec![
            node("a", EvidenceKind::Declared),
            node("b", EvidenceKind::Resolved),
            node("c", EvidenceKind::Inferred),
            node("d", EvidenceKind::Observed),
            node("e", EvidenceKind::Declared),
            node("f", EvidenceKind::Resolved),
            node("isolated", EvidenceKind::Declared),
        ],
        edges: vec![
            edge("a", "b"),
            edge("b", "c"),
            edge("c", "a"),
            edge("d", "e"),
            edge("e", "f"),
            edge("f", "d"),
            edge("a", "missing"),
        ],
        truncated,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    }
}

fn metadata_by_id(snapshot: &GraphSnapshot, key: &str) -> HashMap<String, Value> {
    snapshot
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.metadata[key].clone()))
        .collect()
}

#[test]
fn deterministic_label_propagation_annotates_clusters_and_isolates() {
    let mut graph = snapshot(false);
    let node_evidence = graph
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.evidence))
        .collect::<HashMap<_, _>>();
    let edge_evidence = graph
        .edges
        .iter()
        .map(|edge| (edge.id.clone(), edge.evidence))
        .collect::<HashMap<_, _>>();
    annotate_structural_communities(&mut graph);

    let communities = metadata_by_id(&graph, "communityId");
    assert_eq!(communities["a"], communities["b"]);
    assert_eq!(communities["b"], communities["c"]);
    assert_eq!(communities["d"], communities["e"]);
    assert_eq!(communities["e"], communities["f"]);
    assert_ne!(communities["a"], communities["d"]);
    assert_ne!(communities["a"], communities["isolated"]);
    for node in &graph.nodes {
        let expected_size = if node.id == "isolated" { 1 } else { 3 };
        assert_eq!(node.metadata["communitySize"], json!(expected_size));
        assert_eq!(
            node.metadata["communityAlgorithm"],
            json!("deterministicLabelPropagationV1")
        );
        assert_eq!(
            node.metadata["communityBasis"],
            json!("boundedGraphSnapshot")
        );
        assert_eq!(node.metadata["communityComplete"], json!(true));
        assert_eq!(node.evidence, node_evidence[&node.id]);
    }
    for edge in &graph.edges {
        assert_eq!(edge.evidence, edge_evidence[&edge.id]);
    }
}

#[test]
fn community_assignments_are_order_independent_and_report_truncation() {
    let mut forward = snapshot(false);
    let mut reversed = snapshot(true);
    reversed.nodes.reverse();
    reversed.edges.reverse();

    annotate_structural_communities(&mut forward);
    annotate_structural_communities(&mut reversed);

    assert_eq!(
        metadata_by_id(&forward, "communityId"),
        metadata_by_id(&reversed, "communityId")
    );
    assert_eq!(
        metadata_by_id(&forward, "communitySize"),
        metadata_by_id(&reversed, "communitySize")
    );
    assert!(
        forward
            .nodes
            .iter()
            .all(|node| node.metadata["communityComplete"] == json!(true))
    );
    assert!(
        reversed
            .nodes
            .iter()
            .all(|node| node.metadata["communityComplete"] == json!(false))
    );
}
