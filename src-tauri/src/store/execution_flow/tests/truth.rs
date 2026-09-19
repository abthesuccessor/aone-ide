use serde_json::json;

use super::{by_id, edge, flow_query, indexed_file, node, open_store};
use crate::domain::EvidenceKind;

#[test]
fn execution_flow_excludes_test_and_canonical_sensitive_paths_before_paging() {
    let denied = [
        ".aws/config",
        ".config/gcloud/application_default_credentials.json",
        ".cargo/credentials.toml",
        "credentials.json",
        "service-account.json",
        "infra/terraform.tfstate",
        "certs/client.jks",
        "secret.production.json",
    ];
    let mut files = denied
        .iter()
        .enumerate()
        .map(|(index, path)| {
            indexed_file(
                path,
                vec![node(
                    &format!("denied:{index}"),
                    "function",
                    "denied",
                    path,
                    "Text",
                    1,
                )],
                vec![],
            )
        })
        .collect::<Vec<_>>();
    files.push(indexed_file(
        "tests/root_tests.rs",
        vec![node(
            "test:target",
            "function",
            "test_target",
            "tests/root_tests.rs",
            "Rust",
            1,
        )],
        vec![],
    ));
    files.push(indexed_file(
        "src/production.rs",
        vec![node(
            "production:target",
            "function",
            "production_target",
            "src/production.rs",
            "Rust",
            1,
        )],
        vec![],
    ));
    let mut edges = denied
        .iter()
        .enumerate()
        .map(|(index, _)| {
            edge(
                &format!("a-edge-denied:{index}"),
                "root",
                &format!("denied:{index}"),
                "calls",
                EvidenceKind::Resolved,
            )
        })
        .collect::<Vec<_>>();
    edges.push(edge(
        "b-edge-test",
        "root",
        "test:target",
        "calls",
        EvidenceKind::Resolved,
    ));
    edges.push(edge(
        "z-edge-production",
        "root",
        "production:target",
        "calls",
        EvidenceKind::Resolved,
    ));
    files.push(indexed_file(
        "src/root.rs",
        vec![node("root", "function", "root", "src/root.rs", "Rust", 1)],
        edges,
    ));
    let (_directory, store) = open_store(&files);

    let production = store.query_graph(&flow_query("root", 20, None)).unwrap();
    assert_eq!(production.total_root_links, Some(1));
    assert_eq!(production.nodes.len(), 2);
    assert!(
        production
            .nodes
            .iter()
            .any(|node| node.id == "production:target")
    );

    let mut include_tests = flow_query("root", 20, None);
    include_tests.include_tests = true;
    let with_tests = store.query_graph(&include_tests).unwrap();
    assert_eq!(with_tests.total_root_links, Some(2));
    assert!(with_tests.nodes.iter().any(|node| node.id == "test:target"));
    assert!(
        !with_tests
            .nodes
            .iter()
            .any(|node| node.id.starts_with("denied:"))
    );
}

#[test]
fn execution_flow_keeps_cross_language_inference_unresolved() {
    let target = indexed_file(
        "frontend/src/work.ts",
        vec![node(
            "typescript:work",
            "function",
            "work",
            "frontend/src/work.ts",
            "TypeScript",
            1,
        )],
        vec![],
    );
    let root = indexed_file(
        "backend/src/main.rs",
        vec![
            node(
                "rust:root",
                "function",
                "root",
                "backend/src/main.rs",
                "Rust",
                1,
            ),
            node(
                "rust:call",
                "callTarget",
                "work()",
                "backend/src/main.rs",
                "Rust",
                2,
            ),
        ],
        vec![edge(
            "edge:call",
            "rust:root",
            "rust:call",
            "calls",
            EvidenceKind::Declared,
        )],
    );
    let (_directory, store) = open_store(&[target, root]);
    let snapshot = store
        .query_graph(&flow_query("rust:root", 20, None))
        .unwrap();
    assert!(snapshot.nodes.iter().any(|node| node.id == "rust:call"));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.id == "typescript:work")
    );
}

#[test]
fn execution_flow_same_language_global_match_is_candidate_only() {
    let candidate = indexed_file(
        "backend/src/service.rs",
        vec![
            node(
                "candidate",
                "function",
                "work",
                "backend/src/service.rs",
                "Rust",
                1,
            ),
            node(
                "candidate:child",
                "database",
                "db",
                "backend/src/service.rs",
                "Rust",
                2,
            ),
        ],
        vec![edge(
            "edge:candidate-child",
            "candidate",
            "candidate:child",
            "queries",
            EvidenceKind::Resolved,
        )],
    );
    let root = indexed_file(
        "backend/src/main.rs",
        vec![
            node("root", "function", "root", "backend/src/main.rs", "Rust", 1),
            node(
                "call",
                "callTarget",
                "work()",
                "backend/src/main.rs",
                "Rust",
                2,
            ),
        ],
        vec![edge(
            "edge:call",
            "root",
            "call",
            "calls",
            EvidenceKind::Declared,
        )],
    );
    let (_directory, store) = open_store(&[candidate, root]);
    let snapshot = store.query_graph(&flow_query("root", 20, None)).unwrap();
    assert_eq!(
        by_id(&snapshot, "candidate").metadata["flowCandidateOnly"],
        json!(true)
    );
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.id == "candidate:child")
    );
    assert_eq!(
        snapshot
            .edges
            .iter()
            .find(|edge| edge.target == "candidate")
            .unwrap()
            .evidence,
        EvidenceKind::Inferred
    );
}

#[test]
fn execution_flow_ambiguous_same_name_scopes_leave_the_call_target_raw() {
    let definitions = indexed_file(
        "backend/src/definitions.rs",
        vec![
            node(
                "work:a",
                "function",
                "work",
                "backend/src/definitions.rs",
                "Rust",
                1,
            ),
            node(
                "work:b",
                "function",
                "work",
                "backend/src/definitions.rs",
                "Rust",
                2,
            ),
        ],
        vec![],
    );
    let root = indexed_file(
        "backend/src/main.rs",
        vec![
            node("root", "function", "root", "backend/src/main.rs", "Rust", 1),
            node(
                "call",
                "callTarget",
                "work()",
                "backend/src/main.rs",
                "Rust",
                2,
            ),
        ],
        vec![edge(
            "edge:call",
            "root",
            "call",
            "calls",
            EvidenceKind::Declared,
        )],
    );
    let (_directory, store) = open_store(&[definitions, root]);
    let snapshot = store.query_graph(&flow_query("root", 20, None)).unwrap();
    assert!(snapshot.nodes.iter().any(|node| node.id == "call"));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.id.starts_with("work:"))
    );
}

#[test]
fn execution_flow_marks_only_true_scc_edges_as_cycles() {
    let file = indexed_file(
        "backend/src/cycle.rs",
        vec![
            node("a", "function", "a", "backend/src/cycle.rs", "Rust", 1),
            node("b", "function", "b", "backend/src/cycle.rs", "Rust", 2),
            node("c", "function", "c", "backend/src/cycle.rs", "Rust", 3),
            node("d", "function", "d", "backend/src/cycle.rs", "Rust", 4),
        ],
        vec![
            edge("edge:a-b", "a", "b", "calls", EvidenceKind::Resolved),
            edge("edge:b-a", "b", "a", "calls", EvidenceKind::Resolved),
            edge("edge:a-c", "a", "c", "calls", EvidenceKind::Resolved),
            edge("edge:c-d", "c", "d", "calls", EvidenceKind::Resolved),
            edge("edge:b-d", "b", "d", "calls", EvidenceKind::Resolved),
        ],
    );
    let (_directory, store) = open_store(&[file]);
    let snapshot = store.query_graph(&flow_query("a", 20, None)).unwrap();
    for id in ["edge:a-b", "edge:b-a"] {
        assert_eq!(
            snapshot
                .edges
                .iter()
                .find(|edge| edge.id == id)
                .unwrap()
                .metadata["cyclic"],
            json!(true)
        );
    }
    for id in ["edge:a-c", "edge:c-d", "edge:b-d"] {
        assert!(
            !snapshot
                .edges
                .iter()
                .find(|edge| edge.id == id)
                .unwrap()
                .metadata
                .contains_key("cyclic")
        );
    }
}

#[test]
fn execution_flow_frontend_storage_path_is_not_a_database_lane() {
    let file = indexed_file(
        "frontend/src/storage/view.tsx",
        vec![node(
            "frontend:storage",
            "function",
            "useNeo4jD3Canvas",
            "frontend/src/storage/view.tsx",
            "TypeScript",
            1,
        )],
        vec![],
    );
    let (_directory, store) = open_store(&[file]);
    let snapshot = store
        .query_graph(&flow_query("frontend:storage", 20, None))
        .unwrap();
    assert_eq!(
        by_id(&snapshot, "frontend:storage").metadata["flowStage"],
        "frontend"
    );
}

#[test]
fn execution_flow_source_mapping_requires_a_matching_unique_range() {
    let file = indexed_file(
        "backend/src/source.rs",
        vec![node(
            "source:one",
            "function",
            "one",
            "backend/src/source.rs",
            "Rust",
            5,
        )],
        vec![],
    );
    let (_directory, mut store) = open_store(&[file]);
    assert_eq!(
        store
            .resolve_trace_source(None, "backend/src/source.rs", 5)
            .unwrap(),
        Some("source:one".into())
    );
    assert_eq!(
        store
            .resolve_trace_source(Some("source:one"), "backend/src/source.rs", 5)
            .unwrap(),
        Some("source:one".into())
    );
    assert_eq!(
        store
            .resolve_trace_source(Some("source:one"), "backend/src/source.rs", 6)
            .unwrap(),
        None
    );

    store
        .replace_all(&[indexed_file(
            "backend/src/source.rs",
            vec![
                node(
                    "source:one",
                    "function",
                    "one",
                    "backend/src/source.rs",
                    "Rust",
                    5,
                ),
                node(
                    "source:two",
                    "function",
                    "two",
                    "backend/src/source.rs",
                    "Rust",
                    5,
                ),
            ],
            vec![],
        )])
        .unwrap();
    assert_eq!(
        store
            .resolve_trace_source(None, "backend/src/source.rs", 5)
            .unwrap(),
        None
    );
}
