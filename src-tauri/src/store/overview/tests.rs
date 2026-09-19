use std::{collections::BTreeMap, path::Path};

use serde_json::json;
use tempfile::tempdir;

use super::classify::{SystemLayer, eligible_node, placement};
use crate::{
    analyzer::{analyze_source, language_support},
    domain::{EvidenceKind, GraphNode, GraphProjection, GraphQuery, SourceLocation, WorkspaceFile},
    scanner::IndexedFile,
    store::GraphStore,
};

fn indexed_file(path: &str, source: &str) -> IndexedFile {
    let hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let analysis = analyze_source("overview-workspace", path, source, &hash).unwrap();
    IndexedFile {
        file: WorkspaceFile {
            id: crate::analyzer::stable_id("file", &["overview-workspace", path]),
            relative_path: path.into(),
            language: language_support(Path::new(path)).name.into(),
            capability: language_support(Path::new(path)).capability,
            size_bytes: source.len() as u64,
            content_hash: hash,
            modified_at: "0".into(),
            parse_errors: analysis.parse_errors,
        },
        source: source.into(),
        analysis,
    }
}

fn overview_query() -> GraphQuery {
    GraphQuery {
        projection: GraphProjection::SystemOverview,
        limit: Some(180),
        depth: Some(2),
        ..GraphQuery::default()
    }
}

#[test]
fn overview_beats_alphabetic_dead_files_and_is_diverse_deterministic_and_safe() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let mut files = (0..130)
        .map(|index| indexed_file(&format!("aaa/dead/{index:03}.txt"), ""))
        .collect::<Vec<_>>();
    files.extend([
        indexed_file(".env", "API_KEY=must-not-enter-overview"),
        indexed_file("package.json", "{\"scripts\":{\"dev\":\"vite\"}}"),
        indexed_file(
            "src/main.ts",
            "function main() { startServer(); } function startServer() { app.get('/health', health); } main();",
        ),
        indexed_file(
            "src/user.ts",
            "class UserService { load() { return fetch('https://example.invalid/users'); } } class UserRepository { all() { return prisma.user.findMany(); } }",
        ),
        indexed_file("src/cron/reindex.ts", "export function reindex() {}"),
        indexed_file("src/commands/sync.ts", "export function sync() {}"),
        indexed_file("src/user_tests.rs", "fn main() {}"),
        indexed_file("e2e/main.ts", "function main() { app.get('/e2e', test); }"),
    ]);
    store.replace_all(&files).unwrap();

    let first = store.query_graph(&overview_query()).unwrap();
    let second = store.query_graph(&overview_query()).unwrap();
    assert!(first.nodes.len() <= 60);
    assert!(first.edges.len() <= 120);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert!(!first.nodes.iter().any(|node| {
        let path = node
            .source
            .as_ref()
            .map(|source| source.relative_path.as_str())
            .unwrap_or(&node.label);
        path == ".env"
            || path.contains("aaa/dead")
            || path.contains("_tests.")
            || path.starts_with("e2e/")
    }));
    assert!(
        first
            .nodes
            .iter()
            .all(|node| !matches!(node.kind.as_str(), "callTarget" | "module"))
    );

    let layers = first
        .nodes
        .iter()
        .filter_map(|node| {
            node.metadata
                .get("systemLayer")
                .and_then(|value| value.as_str())
        })
        .collect::<std::collections::HashSet<_>>();
    for expected in [
        "configuration",
        "entry",
        "interface",
        "application",
        "data",
        "external",
    ] {
        assert!(layers.contains(expected), "missing {expected}: {first:#?}");
    }
    assert!(first.nodes.iter().all(|node| {
        node.metadata.contains_key("systemLayer")
            && node.metadata.contains_key("systemLayerBasis")
            && matches!(
                node.metadata.get("systemLayerEvidence"),
                Some(value) if value == &json!("exact") || value == &json!("inferred")
            )
    }));
    let outbound = first.nodes.iter().find(|node| node.kind == "api").unwrap();
    assert_eq!(outbound.evidence, EvidenceKind::Inferred);
    assert_eq!(
        outbound.metadata.get("systemLayerEvidence"),
        Some(&json!("inferred"))
    );
}

#[test]
fn overview_roots_produce_connected_detail_without_file_first_seeds() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let mut files = (0..130)
        .map(|index| indexed_file(&format!("aaa/dead/{index:03}.txt"), ""))
        .collect::<Vec<_>>();
    files.extend([
        indexed_file("package.json", "{\"scripts\":{\"dev\":\"vite\"}}"),
        indexed_file(
            "src/main.ts",
            "function main() { startServer(); } function startServer() { app.get('/health', health); } main();",
        ),
        indexed_file(
            "src/user.ts",
            "export class UserService { load() { return loadUser(); } } export function loadUser() {}",
        ),
    ]);
    store.replace_all(&files).unwrap();

    let overview = store.query_graph(&overview_query()).unwrap();
    let detail = store
        .query_graph(&GraphQuery {
            projection: GraphProjection::Neighborhood,
            root_ids: overview
                .nodes
                .iter()
                .take(50)
                .map(|node| node.id.clone())
                .collect(),
            depth: Some(4),
            limit: Some(500),
            ..GraphQuery::default()
        })
        .unwrap();

    assert!(
        detail.edges.iter().any(|edge| matches!(
            edge.kind.as_str(),
            "calls" | "imports" | "dependsOn" | "writes" | "reads" | "resolvesTo"
        )),
        "{detail:#?}"
    );
    assert!(detail.nodes.iter().all(|node| {
        node.source
            .as_ref()
            .is_none_or(|source| !source.relative_path.starts_with("aaa/dead/"))
    }));
}

#[test]
fn overview_reports_quota_truncation_and_excludes_inferred_resolution_edges() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let services = (0..14)
        .map(|index| format!("class Feature{index:02}Service {{ run() {{}} }}"))
        .collect::<Vec<_>>()
        .join("\n");
    store
        .replace_all(&[
            indexed_file("src/services.ts", &services),
            indexed_file("src/caller.ts", "function caller() { loadUser(); }"),
            indexed_file("src/target.ts", "export function loadUser() {}"),
        ])
        .unwrap();

    let neighborhood = store
        .query_graph(&GraphQuery {
            limit: Some(500),
            depth: Some(4),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(
        neighborhood
            .edges
            .iter()
            .any(|edge| { edge.kind == "resolvesTo" && edge.evidence == EvidenceKind::Inferred })
    );

    let overview = store.query_graph(&overview_query()).unwrap();
    assert!(overview.truncated);
    assert!(
        !overview
            .edges
            .iter()
            .any(|edge| { edge.kind == "resolvesTo" && edge.evidence == EvidenceKind::Inferred })
    );
    assert!(!overview.nodes.iter().any(|node| node.kind == "callTarget"));
}

#[test]
fn source_less_files_and_execution_paths_are_classified_without_false_claims() {
    let source_less_manifest = GraphNode {
        id: "manifest".into(),
        kind: "file".into(),
        label: "package.json".into(),
        source: None,
        language: Some("JSON".into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::new(),
    };
    assert!(eligible_node(&source_less_manifest));
    assert_eq!(
        placement(&source_less_manifest).layer,
        SystemLayer::Configuration
    );

    let source_less_env = GraphNode {
        label: ".env.production".into(),
        id: "env".into(),
        ..source_less_manifest.clone()
    };
    assert!(!eligible_node(&source_less_env));

    for path in [
        "src/job/run.rs",
        "src/workers/run.rs",
        "src/cron/run.rs",
        "src/batch/run.rs",
        "src/queue/run.rs",
        "src/commands/run.rs",
        "src/tasks/run.rs",
    ] {
        let node = GraphNode {
            id: path.into(),
            kind: "function".into(),
            label: "run".into(),
            source: Some(SourceLocation {
                relative_path: path.into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 4,
            }),
            language: Some("Rust".into()),
            evidence: EvidenceKind::Declared,
            metadata: BTreeMap::new(),
        };
        let role = placement(&node);
        assert_eq!(role.layer, SystemLayer::Interface, "{path}");
        assert_eq!(role.evidence, "inferred");
        assert!(role.basis.contains("not proven"));
    }
}

#[test]
fn raw_job_call_targets_cannot_starve_interface_or_data_seeds() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    let calls = (0..40)
        .map(|index| format!("aaa{index:02}();"))
        .collect::<Vec<_>>()
        .join(" ");
    store
        .replace_all(&[
            indexed_file(
                "src/jobs/noise.ts",
                &format!(
                    "export function jobDefinition() {{ {calls} app.get('/visible-job', handler); }}"
                ),
            ),
            indexed_file(
                "src/features/users/repository.rs",
                "fn persist_user() { noisy_one(); noisy_two(); }",
            ),
        ])
        .unwrap();

    let snapshot = store
        .query_graph(&GraphQuery {
            projection: GraphProjection::SystemOverview,
            depth: Some(0),
            limit: Some(60),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(snapshot.nodes.iter().any(|node| {
        node.kind == "endpoint" && node.metadata.get("systemLayer") == Some(&json!("interface"))
    }));
    assert!(snapshot.nodes.iter().any(|node| {
        node.label == "jobDefinition"
            && node.metadata.get("systemLayer") == Some(&json!("interface"))
    }));
    assert!(snapshot.nodes.iter().any(|node| {
        node.label == "persist_user" && node.metadata.get("systemLayer") == Some(&json!("data"))
    }));
    assert!(snapshot.nodes.iter().all(|node| node.kind != "callTarget"));
}

#[test]
fn production_entry_files_rank_ahead_of_migrations_examples_and_evaluations() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("examples/demo/src/main.rs", "fn main() {}"),
            indexed_file("evaluation/runner/src/main.rs", "fn main() {}"),
            indexed_file("migration/tool/src/main.rs", "fn main() {}"),
            indexed_file("tyson-backend/src/main.rs", "fn main() {}"),
            indexed_file("tyson-frontend/src/main.tsx", "function main() {}"),
            indexed_file("tyson-platform-admin/src/main.tsx", "function main() {}"),
        ])
        .unwrap();

    let snapshot = store
        .query_graph(&GraphQuery {
            projection: GraphProjection::SystemOverview,
            depth: Some(0),
            limit: Some(3),
            ..GraphQuery::default()
        })
        .unwrap();
    let entry_paths = snapshot
        .nodes
        .iter()
        .filter(|node| node.metadata.get("systemLayer") == Some(&json!("entry")))
        .filter_map(|node| {
            node.source
                .as_ref()
                .map(|source| source.relative_path.as_str())
        })
        .collect::<Vec<_>>();
    for expected in [
        "tyson-backend/src/main.rs",
        "tyson-frontend/src/main.tsx",
        "tyson-platform-admin/src/main.tsx",
    ] {
        assert!(entry_paths.contains(&expected), "entries: {entry_paths:#?}");
    }
    assert!(!entry_paths.iter().any(|path| path.contains("evaluation")));
}

#[test]
fn api_placement_requires_a_non_local_absolute_static_target() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_file(&indexed_file(
            "src/api.ts",
            "function requests() { fetch('/api/users'); fetch('http://localhost:3000/health'); fetch('https://vendor.example/v1'); }",
        ))
        .unwrap();

    let snapshot = store
        .query_graph(&GraphQuery {
            projection: GraphProjection::SystemOverview,
            depth: Some(0),
            limit: Some(60),
            ..GraphQuery::default()
        })
        .unwrap();
    let apis = snapshot
        .nodes
        .iter()
        .filter(|node| node.kind == "api")
        .collect::<Vec<_>>();
    assert_eq!(apis.len(), 3);
    for node in apis {
        let layer = node.metadata.get("systemLayer");
        if node.label.contains("vendor.example") {
            assert_eq!(layer, Some(&json!("external")));
            assert_eq!(
                node.metadata.get("systemLayerEvidence"),
                Some(&json!("inferred"))
            );
        } else {
            assert_eq!(layer, Some(&json!("interface")));
            assert!(
                node.metadata
                    .get("systemLayerBasis")
                    .and_then(|value| value.as_str())
                    .is_some_and(|basis| basis.contains("target unverified"))
            );
        }
    }
}

#[test]
fn overview_uses_structural_roles_without_test_name_or_canvas_guesses() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file(
                "src/policy.rs",
                "fn policy_requires_approval() { enforce_policy(); }",
            ),
            indexed_file(
                "src/repositories/user_repository.rs",
                "fn persist_user() { write_record(); }",
            ),
            indexed_file(
                "src/features/storage/useCanvas.ts",
                "export function currentGraphPoint() { return 1; } canvas.on('tick', draw);",
            ),
            indexed_file("evaluation/cli.py", "def helper():\n    return 1\n"),
        ])
        .unwrap();

    let snapshot = store
        .query_graph(&GraphQuery {
            projection: GraphProjection::SystemOverview,
            depth: Some(0),
            limit: Some(60),
            ..GraphQuery::default()
        })
        .unwrap();
    assert!(snapshot.nodes.iter().any(|node| {
        node.label == "policy_requires_approval"
            && node.metadata.get("systemLayer") == Some(&json!("application"))
    }));
    assert!(snapshot.nodes.iter().any(|node| {
        node.label == "persist_user" && node.metadata.get("systemLayer") == Some(&json!("data"))
    }));
    assert!(!snapshot.nodes.iter().any(|node| {
        node.label == "currentGraphPoint"
            && node.metadata.get("systemLayer") == Some(&json!("data"))
    }));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.kind == "event" && node.label == "tick")
    );
    assert!(!snapshot.nodes.iter().any(|node| {
        node.source
            .as_ref()
            .is_some_and(|source| source.relative_path.starts_with("evaluation/"))
    }));
}
