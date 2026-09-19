use std::collections::{BTreeSet, HashSet};

use serde_json::json;

use super::{by_id, edge, flow_query, indexed_file, metadata, node, open_store};
use crate::domain::EvidenceKind;

fn tyson_flow(client_count: usize) -> crate::scanner::IndexedFile {
    let mut openapi = node(
        "endpoint:openapi",
        "endpoint",
        "GET /api/v1/demands",
        "tyson-backend/openapi.json",
        "OpenAPI",
        10,
    );
    metadata(
        &mut openapi,
        &[
            ("apiRole", json!("producer")),
            ("httpMethod", json!("GET")),
            ("routePath", json!("/api/v1/demands")),
            ("apiServiceRoot", json!("tyson-backend")),
            ("apiFramework", json!("OpenAPI")),
        ],
    );
    let mut utoipa = node(
        "endpoint:utoipa",
        "endpoint",
        "GET /api/v1/demands",
        "tyson-backend/src/api/demands.rs",
        "Rust",
        20,
    );
    metadata(
        &mut utoipa,
        &[
            ("apiRole", json!("producer")),
            ("httpMethod", json!("GET")),
            ("routePath", json!("/api/v1/demands")),
            ("apiServiceRoot", json!("tyson-backend")),
            ("apiFramework", json!("Utoipa")),
        ],
    );
    let handler = node(
        "handler:list-demands",
        "function",
        "list_demands",
        "tyson-backend/src/api/demands.rs",
        "Rust",
        21,
    );
    let service = node(
        "service:demand",
        "trait",
        "DemandService",
        "tyson-backend/src/services/demand.rs",
        "Rust",
        30,
    );
    let repository = node(
        "repository:demand",
        "repository",
        "DemandRepository",
        "tyson-backend/src/repositories/demand.rs",
        "Rust",
        40,
    );
    let database = node(
        "database:postgres",
        "database",
        "PostgreSQL",
        "tyson-backend/src/repositories/demand.rs",
        "SQL",
        50,
    );
    let mut nodes = vec![openapi, utoipa, handler, service, repository, database];
    for index in 0..client_count {
        let mut client = node(
            &format!("client:{index:03}"),
            "api",
            "GET /api/v1/demands",
            &format!("tyson-frontend/src/api/client_{index:03}.ts"),
            "TypeScript",
            index + 1,
        );
        metadata(
            &mut client,
            &[
                ("apiRole", json!("consumer")),
                ("httpMethod", json!("GET")),
                ("routePath", json!("/api/v1/demands")),
            ],
        );
        nodes.push(client);
    }
    indexed_file(
        "tyson-backend/src/api/flow-fixture.rs",
        nodes,
        vec![
            edge(
                "edge:handles",
                "handler:list-demands",
                "endpoint:utoipa",
                "handles",
                EvidenceKind::Declared,
            ),
            edge(
                "edge:handler-service",
                "handler:list-demands",
                "service:demand",
                "calls",
                EvidenceKind::Resolved,
            ),
            edge(
                "edge:service-repository",
                "service:demand",
                "repository:demand",
                "calls",
                EvidenceKind::Resolved,
            ),
            edge(
                "edge:repository-database",
                "repository:demand",
                "database:postgres",
                "queries",
                EvidenceKind::Resolved,
            ),
        ],
    )
}

fn fastapi_prefix_flow(ambiguous: bool) -> crate::scanner::IndexedFile {
    let mut specification = node(
        "endpoint:openapi-auth",
        "endpoint",
        "POST /api/v1/login",
        "openapi/auth.openapi.json",
        "JSON",
        10,
    );
    metadata(
        &mut specification,
        &[
            ("apiRole", json!("producer")),
            ("httpMethod", json!("POST")),
            ("routePath", json!("/api/v1/login")),
            ("apiServiceRoot", json!("Workspace root")),
            ("apiFramework", json!("OpenAPI")),
            ("operationId", json!("login_api_v1_login_post")),
        ],
    );
    let mut source_endpoint = node(
        "endpoint:fastapi-auth",
        "endpoint",
        "POST /login",
        "src/api/auth.py",
        "Python",
        12,
    );
    metadata(
        &mut source_endpoint,
        &[
            ("apiRole", json!("producer")),
            ("httpMethod", json!("POST")),
            ("routePath", json!("/login")),
            ("apiServiceRoot", json!("Workspace root")),
            ("apiFramework", json!("FastAPI")),
        ],
    );
    let handler = node(
        "handler:login",
        "function",
        "login",
        "src/api/auth.py",
        "Python",
        13,
    );
    let database = node(
        "database:session",
        "database",
        "session lookup",
        "src/api/auth.py",
        "Python",
        14,
    );
    let mut nodes = vec![specification, source_endpoint, handler, database];
    let mut edges = vec![
        edge(
            "edge:fastapi-handles",
            "handler:login",
            "endpoint:fastapi-auth",
            "handles",
            EvidenceKind::Declared,
        ),
        edge(
            "edge:login-database",
            "handler:login",
            "database:session",
            "queries",
            EvidenceKind::Inferred,
        ),
    ];
    if ambiguous {
        let mut duplicate = node(
            "endpoint:fastapi-auth-duplicate",
            "endpoint",
            "POST /login",
            "src/legacy/auth.py",
            "Python",
            20,
        );
        duplicate.metadata = nodes[1].metadata.clone();
        nodes.push(duplicate);
        nodes.push(node(
            "handler:login-duplicate",
            "function",
            "login",
            "src/legacy/auth.py",
            "Python",
            21,
        ));
        edges.push(edge(
            "edge:fastapi-handles-duplicate",
            "handler:login-duplicate",
            "endpoint:fastapi-auth-duplicate",
            "handles",
            EvidenceKind::Declared,
        ));
    }
    indexed_file("src/api/auth.py", nodes, edges)
}

#[test]
fn execution_flow_tyson_openapi_root_reaches_unique_handler_and_nested_data() {
    let (_directory, store) = open_store(&[tyson_flow(2)]);
    let snapshot = store
        .query_graph(&flow_query("endpoint:openapi", 120, None))
        .unwrap();

    for id in [
        "endpoint:openapi",
        "handler:list-demands",
        "service:demand",
        "repository:demand",
        "database:postgres",
        "client:000",
        "client:001",
    ] {
        assert!(
            snapshot.nodes.iter().any(|node| node.id == id),
            "missing {id}"
        );
    }
    assert_eq!(
        by_id(&snapshot, "handler:list-demands").metadata["flowStage"],
        "backend"
    );
    assert_eq!(
        by_id(&snapshot, "service:demand").metadata["flowStage"],
        "service"
    );
    assert_eq!(
        by_id(&snapshot, "repository:demand").metadata["flowStage"],
        "data"
    );
    assert_eq!(
        by_id(&snapshot, "client:000").metadata["flowStage"],
        "frontend"
    );
    let handled = snapshot
        .edges
        .iter()
        .find(|edge| edge.kind == "handledBy" && edge.source == "endpoint:openapi")
        .unwrap();
    assert_eq!(handled.source, "endpoint:openapi");
    assert_eq!(handled.target, "handler:list-demands");
    assert_eq!(handled.metadata["sourceEdgeIds"], json!(["edge:handles"]));
}

#[test]
fn execution_flow_infers_unique_fastapi_handler_from_operation_id_and_path_suffix() {
    let (_directory, store) = open_store(&[fastapi_prefix_flow(false)]);
    let snapshot = store
        .query_graph(&flow_query("endpoint:openapi-auth", 120, None))
        .unwrap();

    assert!(snapshot.nodes.iter().any(|node| node.id == "handler:login"));
    assert!(
        snapshot
            .nodes
            .iter()
            .any(|node| node.id == "database:session")
    );
    let handled = snapshot
        .edges
        .iter()
        .find(|edge| edge.kind == "handledBy")
        .unwrap();
    assert_eq!(handled.source, "endpoint:openapi-auth");
    assert_eq!(handled.target, "handler:login");
    assert_eq!(handled.evidence, EvidenceKind::Inferred);
    assert_eq!(handled.metadata["staticOnly"], json!(true));
    assert_eq!(
        handled.metadata["handlerMatch"],
        json!("operationIdPrefixAndPathSuffix")
    );
}

#[test]
fn execution_flow_rejects_ambiguous_operation_id_handler_fallback() {
    let (_directory, store) = open_store(&[fastapi_prefix_flow(true)]);
    let snapshot = store
        .query_graph(&flow_query("endpoint:openapi-auth", 120, None))
        .unwrap();

    assert!(!snapshot.edges.iter().any(|edge| edge.kind == "handledBy"));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.id.starts_with("handler:login"))
    );
}

#[test]
fn execution_flow_pages_clients_after_handler_and_reports_cumulative_omissions() {
    let (_directory, store) = open_store(&[tyson_flow(205)]);
    let mut cursor = None;
    let mut client_ids = BTreeSet::new();
    let mut page_count = 0;
    loop {
        let snapshot = store
            .query_graph(&flow_query("endpoint:openapi", 120, cursor.clone()))
            .unwrap();
        page_count += 1;
        assert!(snapshot.nodes.len() <= 120);
        assert!(snapshot.edges.len() <= 2_000);
        if page_count == 1 {
            assert!(
                snapshot
                    .nodes
                    .iter()
                    .any(|node| node.id == "handler:list-demands")
            );
            assert!(
                snapshot
                    .nodes
                    .iter()
                    .any(|node| node.id == "service:demand")
            );
        }
        client_ids.extend(
            snapshot
                .nodes
                .iter()
                .filter(|node| node.id.starts_with("client:"))
                .map(|node| node.id.clone()),
        );
        let root = by_id(&snapshot, "endpoint:openapi");
        assert_eq!(snapshot.total_root_links, Some(206));
        assert_eq!(
            root.metadata["flowOmittedChildCount"],
            json!(snapshot.omitted_root_links.unwrap())
        );
        cursor = snapshot.next_cursor.clone();
        if cursor.is_none() {
            assert_eq!(snapshot.omitted_root_links, Some(0));
            assert_eq!(root.metadata["flowExpandable"], json!(false));
            break;
        }
        assert!(page_count < 5);
    }
    assert_eq!(page_count, 3);
    assert_eq!(client_ids.len(), 205);
}

#[test]
fn execution_flow_rejects_cursor_reuse_and_out_of_range_offsets() {
    let (_directory, store) = open_store(&[tyson_flow(205)]);
    let first = store
        .query_graph(&flow_query("endpoint:openapi", 120, None))
        .unwrap();
    let cursor = first.next_cursor.unwrap();

    let mismatch = store
        .query_graph(&flow_query("endpoint:openapi", 119, Some(cursor.clone())))
        .unwrap_err();
    assert!(mismatch.to_string().contains("does not match"));

    let (_, hash) = cursor.split_once('.').unwrap();
    let out_of_range = store
        .query_graph(&flow_query(
            "endpoint:openapi",
            120,
            Some(format!("999999.{hash}")),
        ))
        .unwrap_err();
    assert!(out_of_range.to_string().contains("beyond"));
}

#[test]
fn execution_flow_ambiguous_http_handlers_are_not_presented_as_exact() {
    let mut file = tyson_flow(0);
    file.analysis.nodes.push(node(
        "handler:duplicate",
        "function",
        "duplicate",
        "tyson-backend/src/api/duplicate.rs",
        "Rust",
        70,
    ));
    file.analysis.edges.push(edge(
        "edge:duplicate-handles",
        "handler:duplicate",
        "endpoint:utoipa",
        "handles",
        EvidenceKind::Declared,
    ));
    file.analysis.fact_count = file.analysis.nodes.len() + file.analysis.edges.len();
    let (_directory, store) = open_store(&[file]);
    let snapshot = store
        .query_graph(&flow_query("endpoint:openapi", 120, None))
        .unwrap();
    assert!(!snapshot.edges.iter().any(|edge| edge.kind == "handledBy"));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.id.starts_with("handler:"))
    );
}

#[test]
fn execution_flow_is_stable_across_repeated_queries() {
    let (_directory, store) = open_store(&[tyson_flow(8)]);
    let query = flow_query("endpoint:openapi", 120, None);
    let first = serde_json::to_value(store.query_graph(&query).unwrap()).unwrap();
    let second = serde_json::to_value(store.query_graph(&query).unwrap()).unwrap();
    assert_eq!(first, second);
    let edge_ids = first["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        edge_ids.iter().copied().collect::<HashSet<_>>().len(),
        edge_ids.len()
    );
}
