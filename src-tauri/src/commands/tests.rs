use std::{
    collections::{BTreeMap, HashMap},
    fs,
    time::Duration,
};

use serde_json::json;

use tempfile::tempdir;

use super::{
    api_inventory::validate_workspace_id_for_test,
    environment::{install_provider_env, install_run_env},
    graph::apply_runtime_overlay,
    workspace::{
        commit_scan_result, current_context_for_root, selected_file_target,
        wait_for_context_analysis,
    },
};
use crate::{
    ai::SecretState,
    domain::{
        CapabilityLevel, EvidenceKind, GraphNode, GraphSnapshot, LanguageSummary, OpenFileResult,
        RuntimeEvent, WorkspaceSummary,
    },
    runner::RuntimeState,
    state::new_workspace_context,
    store::GraphStore,
};

#[test]
fn native_file_target_is_the_canonical_containing_directory() {
    let workspace = tempdir().unwrap();
    let nested = workspace.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let selected = nested.join("main.rs");
    fs::write(&selected, "fn main() {}\n").unwrap();

    let (root, relative_path) = selected_file_target(&selected).unwrap();

    assert_eq!(root, nested.canonicalize().unwrap());
    assert_eq!(relative_path, "main.rs");
}

#[test]
fn native_file_target_rejects_non_files_and_sensitive_names() {
    let workspace = tempdir().unwrap();
    let directory = workspace.path().join("directory");
    fs::create_dir(&directory).unwrap();
    let sensitive = workspace.path().join(".env");
    fs::write(&sensitive, "TOKEN=secret\n").unwrap();

    assert!(selected_file_target(&directory).is_err());
    assert!(selected_file_target(&sensitive).is_err());
}

#[test]
fn open_file_result_serializes_the_renderer_contract_in_camel_case() {
    let result = OpenFileResult {
        workspace: WorkspaceSummary {
            id: "workspace:test".into(),
            name: "test".into(),
            root_path: "/tmp/test".into(),
            file_count: 1,
            node_count: 1,
            edge_count: 0,
            languages: vec![LanguageSummary {
                language: "Rust".into(),
                file_count: 1,
                capability: CapabilityLevel::Semantic,
            }],
            last_scanned_at: "2026-08-22T00:00:00Z".into(),
        },
        relative_path: "main.rs".into(),
    };

    let serialized = serde_json::to_value(result).unwrap();
    assert_eq!(serialized["relativePath"], "main.rs");
    assert!(serialized.get("relative_path").is_none());
    assert_eq!(serialized["workspace"]["id"], "workspace:test");
}

#[test]
fn provider_configuration_never_enters_runner_environment() {
    let runtime = RuntimeState::default();
    let secrets = SecretState::new();
    let values = HashMap::from([
        ("AONE_AI_PROVIDER".into(), "anthropic".into()),
        ("OPENAI_API_KEY".into(), "provider-secret".into()),
        ("OPENAI_MODEL".into(), "gpt-5.6-luna".into()),
        ("ANTHROPIC_API_KEY".into(), "anthropic-secret".into()),
        ("ANTHROPIC_MODEL".into(), "claude-sonnet-4-20250514".into()),
        ("PORT".into(), "4310".into()),
    ]);

    let result = install_provider_env(values, &secrets).unwrap();
    assert_eq!(
        result.names,
        vec![
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_MODEL",
            "AONE_AI_PROVIDER",
            "OPENAI_API_KEY",
            "OPENAI_MODEL",
        ]
    );
    assert!(runtime.env_names().is_empty());
}

#[test]
fn provider_environment_reports_missing_keys_precisely() {
    let secrets = SecretState::new();
    install_provider_env(
        HashMap::from([("ANTHROPIC_API_KEY".into(), "working-provider-secret".into())]),
        &secrets,
    )
    .unwrap();
    let working_status = secrets.configuration_status();

    let empty_error = install_provider_env(
        HashMap::from([("OPENAI_MODEL".into(), "gpt-5.6-luna".into())]),
        &secrets,
    )
    .unwrap_err()
    .to_string();
    assert!(empty_error.contains("no supported provider key found"));
    assert_eq!(secrets.configuration_status(), working_status);

    let selected_error = install_provider_env(
        HashMap::from([
            ("AONE_AI_PROVIDER".into(), "openai".into()),
            ("ANTHROPIC_API_KEY".into(), "unselected-secret".into()),
        ]),
        &secrets,
    )
    .unwrap_err()
    .to_string();
    assert!(selected_error.contains("OPENAI_API_KEY is missing"));
    assert!(!selected_error.contains("unselected-secret"));
    assert_eq!(secrets.configuration_status(), working_status);

    let blank_error = install_provider_env(
        HashMap::from([("OPENAI_API_KEY".into(), "  \t".into())]),
        &secrets,
    )
    .unwrap_err()
    .to_string();
    assert!(blank_error.contains("no supported provider key found"));
    assert_eq!(secrets.configuration_status(), working_status);
}

#[test]
fn run_environment_filters_provider_configuration() {
    let runtime = RuntimeState::default();
    let workspace = tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    runtime.begin_workspace_switch().unwrap();
    runtime.complete_workspace_switch(root.clone());
    let values = HashMap::from([
        ("AONE_AI_PROVIDER".into(), "anthropic".into()),
        ("ANTHROPIC_API_KEY".into(), "anthropic-secret".into()),
        ("ANTHROPIC_MODEL".into(), "claude-sonnet-4-20250514".into()),
        ("OPENAI_API_KEY".into(), "provider-secret".into()),
        ("OPENAI_MODEL".into(), "gpt-5.6-luna".into()),
        ("PORT".into(), "4310".into()),
    ]);

    let result = install_run_env(values, &runtime, &root).unwrap();
    assert_eq!(result.names, vec!["PORT"]);
    assert_eq!(runtime.env_names(), vec!["PORT"]);
}

#[test]
fn active_canonical_root_is_routed_to_its_existing_context() {
    let workspace = tempdir().unwrap();
    let other_workspace = tempdir().unwrap();
    let database = tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let context = new_workspace_context(
        "workspace:current".into(),
        root.clone(),
        GraphStore::open(&database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();

    let selected = current_context_for_root(Some(context.clone()), &root).unwrap();
    assert_eq!(selected.id, context.id);
    assert!(
        current_context_for_root(
            Some(context),
            &other_workspace.path().canonicalize().unwrap(),
        )
        .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_selection_waits_for_active_analysis_instead_of_failing() {
    let workspace = tempdir().unwrap();
    let database = tempdir().unwrap();
    let context = new_workspace_context(
        "workspace:current".into(),
        workspace.path().canonicalize().unwrap(),
        GraphStore::open(&database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();
    let active = context.try_reserve_analysis().unwrap();
    let waiting = wait_for_context_analysis(context.clone());
    tokio::pin!(waiting);

    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut waiting)
            .await
            .is_err()
    );
    drop(active);

    let acquired = tokio::time::timeout(Duration::from_secs(1), &mut waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(context.try_reserve_analysis().is_err());
    drop(acquired);
    assert!(context.try_reserve_analysis().is_ok());
}

#[test]
fn committing_graph_and_search_index_reports_indeterminate_progress() {
    let database = tempdir().unwrap();
    let mut store = GraphStore::open(&database.path().join("aone.sqlite")).unwrap();
    let mut progress = Vec::new();

    commit_scan_result(&mut store, &[], |update| progress.push(update)).unwrap();

    assert_eq!(progress.len(), 1);
    assert_eq!(progress[0].phase, "committing");
    assert_eq!(progress[0].completed, 0);
    assert_eq!(progress[0].total, 0);
    assert!(progress[0].current_path.is_none());
    assert_eq!(store.counts().unwrap(), (0, 0, 0));
}

#[test]
fn api_inventory_requires_a_bounded_non_control_workspace_id() {
    assert!(validate_workspace_id_for_test("workspace:current").is_ok());
    assert!(validate_workspace_id_for_test("").is_err());
    assert!(validate_workspace_id_for_test("workspace:\nspoof").is_err());
    assert!(validate_workspace_id_for_test(&"x".repeat(201)).is_err());
}

#[test]
fn runtime_overlay_ignores_process_reported_candidate_and_preserves_static_evidence() {
    let static_node = GraphNode {
        id: "shared:handler".into(),
        kind: "function".into(),
        label: "handler".into(),
        source: None,
        language: Some("Rust".into()),
        evidence: EvidenceKind::Declared,
        metadata: BTreeMap::new(),
    };
    let mut snapshot = GraphSnapshot {
        nodes: vec![static_node],
        edges: vec![],
        truncated: false,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    };
    let candidate = RuntimeEvent {
        id: "candidate".into(),
        run_id: Some("run:a".into()),
        kind: "trace.function.event".into(),
        timestamp: "2026-08-17T00:00:00Z".into(),
        label: "reported".into(),
        evidence: EvidenceKind::Observed,
        source_node_id: None,
        trace_id: Some("trace:a".into()),
        metadata: BTreeMap::from([
            ("mappedNodeId".into(), json!("shared:handler")),
            ("mappingKind".into(), json!("processReportedSourceRange")),
        ]),
    };
    apply_runtime_overlay(&mut snapshot, &[candidate]);
    assert_eq!(snapshot.nodes[0].evidence, EvidenceKind::Declared);
    assert!(
        !snapshot.nodes[0]
            .metadata
            .contains_key("observedEventCount")
    );

    let exact = RuntimeEvent {
        id: "exact".into(),
        source_node_id: Some("shared:handler".into()),
        metadata: BTreeMap::new(),
        ..RuntimeEvent {
            id: "unused".into(),
            run_id: None,
            kind: "native.boundary".into(),
            timestamp: "2026-08-17T00:00:01Z".into(),
            label: "native".into(),
            evidence: EvidenceKind::Observed,
            source_node_id: None,
            trace_id: None,
            metadata: BTreeMap::new(),
        }
    };
    apply_runtime_overlay(&mut snapshot, &[exact]);
    assert_eq!(snapshot.nodes[0].evidence, EvidenceKind::Declared);
    assert_eq!(snapshot.nodes[0].metadata["observedEventCount"], 1);
    assert_eq!(
        snapshot.nodes[0].metadata["runtimeOverlayBasis"],
        "backend-authored exact sourceNodeId only"
    );
}

#[test]
fn old_workspace_events_cannot_overlay_same_id_in_new_workspace() {
    let runtime = RuntimeState::default();
    let first = tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    runtime.begin_workspace_switch().unwrap();
    runtime.complete_workspace_switch(first_root.clone());
    runtime.record_event(RuntimeEvent {
        id: "old".into(),
        run_id: None,
        kind: "native.boundary".into(),
        timestamp: "2026-08-17T00:00:00Z".into(),
        label: "old".into(),
        evidence: EvidenceKind::Observed,
        source_node_id: Some("shared:handler".into()),
        trace_id: None,
        metadata: BTreeMap::new(),
    });
    runtime.begin_workspace_switch().unwrap();
    assert!(
        runtime
            .list_events_for_workspace(&first_root, None)
            .is_err()
    );
    let second = tempdir().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    runtime.complete_workspace_switch(second_root.clone());
    let events = runtime
        .list_events_for_workspace(&second_root, None)
        .unwrap();
    assert!(events.is_empty());

    let mut snapshot = GraphSnapshot {
        nodes: vec![GraphNode {
            id: "shared:handler".into(),
            kind: "function".into(),
            label: "new handler".into(),
            source: None,
            language: Some("Rust".into()),
            evidence: EvidenceKind::Declared,
            metadata: BTreeMap::new(),
        }],
        edges: vec![],
        truncated: false,
        next_cursor: None,
        total_root_links: None,
        omitted_root_links: None,
    };
    apply_runtime_overlay(&mut snapshot, &events);
    assert!(
        !snapshot.nodes[0]
            .metadata
            .contains_key("observedEventCount")
    );
}
