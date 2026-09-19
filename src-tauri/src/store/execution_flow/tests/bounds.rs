use std::collections::HashMap;

use super::{edge, flow_query, indexed_file, node, open_store};
use crate::domain::EvidenceKind;

#[test]
fn execution_flow_dense_cycles_respect_edge_cap_and_truthful_child_counts() {
    let mut nodes = vec![node(
        "dense:root",
        "function",
        "root",
        "backend/src/dense.rs",
        "Rust",
        1,
    )];
    nodes.extend((0..50).map(|index| {
        node(
            &format!("dense:{index:02}"),
            "function",
            &format!("dense_{index:02}"),
            "backend/src/dense.rs",
            "Rust",
            index + 2,
        )
    }));
    let mut edges = (0..50)
        .map(|index| {
            edge(
                &format!("edge:root:{index:02}"),
                "dense:root",
                &format!("dense:{index:02}"),
                "calls",
                EvidenceKind::Resolved,
            )
        })
        .collect::<Vec<_>>();
    for source in 0..50 {
        for offset in 0..40 {
            let target = (source + offset + 1) % 50;
            edges.push(edge(
                &format!("edge:dense:{source:02}:{target:02}"),
                &format!("dense:{source:02}"),
                &format!("dense:{target:02}"),
                "calls",
                EvidenceKind::Resolved,
            ));
        }
    }
    let file = indexed_file("backend/src/dense.rs", nodes, edges);
    let (_directory, store) = open_store(&[file]);
    let snapshot = store
        .query_graph(&flow_query("dense:root", 500, None))
        .unwrap();

    assert_eq!(snapshot.edges.len(), 2_000);
    assert!(snapshot.truncated);
    let root = snapshot
        .nodes
        .iter()
        .find(|node| node.id == "dense:root")
        .unwrap();
    assert_eq!(root.metadata["flowResponseEdgeLimit"], 2_000);
    assert_eq!(root.metadata["flowOmittedEdgeCountKnown"], false);

    let returned_by_source =
        snapshot
            .edges
            .iter()
            .fold(HashMap::<&str, usize>::new(), |mut counts, edge| {
                *counts.entry(edge.source.as_str()).or_default() += 1;
                counts
            });
    for node in &snapshot.nodes {
        let Some(total) = node
            .metadata
            .get("flowChildCount")
            .and_then(serde_json::Value::as_u64)
        else {
            continue;
        };
        let omitted = node.metadata["flowOmittedChildCount"].as_u64().unwrap();
        let shown = total.saturating_sub(omitted) as usize;
        assert_eq!(
            shown,
            returned_by_source
                .get(node.id.as_str())
                .copied()
                .unwrap_or(0),
            "child metadata diverged for {}",
            node.id
        );
    }
}

#[test]
fn execution_flow_root_identifier_and_cursor_inputs_are_bounded() {
    let file = indexed_file(
        "backend/src/root.rs",
        vec![node(
            "root",
            "function",
            "root",
            "backend/src/root.rs",
            "Rust",
            1,
        )],
        vec![],
    );
    let (_directory, store) = open_store(&[file]);
    for root in ["", "bad\nroot"] {
        let error = store.query_graph(&flow_query(root, 20, None)).unwrap_err();
        assert!(error.to_string().contains("rootId"));
    }
    let error = store
        .query_graph(&flow_query(&"x".repeat(201), 20, None))
        .unwrap_err();
    assert!(error.to_string().contains("rootId"));
    let error = store
        .query_graph(&flow_query("root", 20, Some("x".repeat(97))))
        .unwrap_err();
    assert!(error.to_string().contains("cursor"));
}

#[test]
fn execution_flow_rejects_source_less_legacy_node_from_denied_file() {
    let mut denied = node(
        "legacy:secret",
        "function",
        "legacy secret",
        ".aws/credentials",
        "Text",
        1,
    );
    denied.source = None;
    let file = indexed_file(".aws/credentials", vec![denied], vec![]);
    let (_directory, store) = open_store(&[file]);
    let mut query = flow_query("legacy:secret", 20, None);
    query.include_tests = true;
    let error = store.query_graph(&query).unwrap_err();
    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn execution_flow_source_less_legacy_test_root_requires_include_tests() {
    let mut test_node = node(
        "legacy:test",
        "function",
        "legacy test",
        "tests/legacy.rs",
        "Rust",
        1,
    );
    test_node.source = None;
    let file = indexed_file("tests/legacy.rs", vec![test_node], vec![]);
    let (_directory, store) = open_store(&[file]);

    let error = store
        .query_graph(&flow_query("legacy:test", 20, None))
        .unwrap_err();
    assert!(error.to_string().contains("does not exist"));

    let mut query = flow_query("legacy:test", 20, None);
    query.include_tests = true;
    let snapshot = store.query_graph(&query).unwrap();
    assert_eq!(snapshot.nodes[0].id, "legacy:test");
}
