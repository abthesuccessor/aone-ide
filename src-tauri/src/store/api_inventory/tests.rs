use std::{collections::HashSet, path::Path};

use tempfile::tempdir;

use super::super::GraphStore;
use crate::{
    analyzer::{analyze_source, language_support, stable_id},
    domain::{ApiEndpointRole, EvidenceKind, ListApiEndpointsRequest, WorkspaceFile},
    scanner::IndexedFile,
};

mod fastapi;

fn indexed_file(path: &str, source: &str) -> IndexedFile {
    let hash = blake3::hash(source.as_bytes()).to_hex().to_string();
    let support = language_support(Path::new(path));
    let analysis = analyze_source("workspace", path, source, &hash).unwrap();
    IndexedFile {
        file: WorkspaceFile {
            id: stable_id("file", &["workspace", path]),
            relative_path: path.into(),
            language: support.name.into(),
            capability: support.capability,
            size_bytes: source.len() as u64,
            content_hash: hash,
            modified_at: "0".into(),
            parse_errors: analysis.parse_errors,
        },
        source: source.into(),
        analysis,
    }
}

fn request(cursor: Option<String>, limit: usize) -> ListApiEndpointsRequest {
    ListApiEndpointsRequest {
        workspace_id: "workspace".into(),
        query: None,
        cursor,
        limit: Some(limit),
    }
}

#[test]
fn cursor_pagination_reaches_more_than_five_hundred_unique_operations() {
    let routes = (0..601)
        .map(|index| format!("router.get('/api/v1/items/{index:03}', handler);"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("function install() {{\n{routes}\n}}");
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("service/src/routes.ts", &source)])
        .unwrap();

    let mut cursor = None;
    let mut ids = Vec::new();
    let mut pages = 0;
    loop {
        let page = store.list_api_endpoints(&request(cursor, 200)).unwrap();
        pages += 1;
        assert_eq!(page.total, 601);
        assert_eq!(page.indexed_total, 601);
        assert_eq!(page.counts.producer, 601);
        ids.extend(page.endpoints.iter().map(|endpoint| endpoint.id.clone()));
        if !page.truncated {
            assert!(page.next_cursor.is_none());
            break;
        }
        cursor = page.next_cursor;
    }

    assert_eq!(pages, 4);
    assert_eq!(ids.len(), 601);
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), 601);
}

#[test]
fn merges_spec_handler_and_client_provenance_into_one_operation() {
    let openapi = r#"{
      "openapi":"3.1.0",
      "info":{"title":"Accounts","version":"1"},
      "paths":{"/api/v1/users":{"get":{"tags":["Users"],"operationId":"listUsers"}}}
    }"#;
    let handler = r#"#[utoipa::path(get, path = "/api/v1/users")]
      async fn list_users() {}"#;
    let client = "function load() { api.get('/api/v1/users'); }";
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("service/openapi.json", openapi),
            indexed_file("service/src/users.rs", handler),
            indexed_file("web/src/users.ts", client),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.counts.producer, 1);
    assert_eq!(page.counts.consumer_only, 0);
    assert_eq!(page.counts.client_covered, 1);
    let endpoint = &page.endpoints[0];
    assert_eq!(endpoint.role, ApiEndpointRole::Producer);
    assert_eq!(endpoint.framework.as_deref(), Some("OpenAPI"));
    assert_eq!(endpoint.operation_id.as_deref(), Some("listUsers"));
    assert_eq!(endpoint.grouping.segments, ["Accounts", "Users"]);
    assert_eq!(endpoint.client_coverage.count, 1);
    assert!(endpoint.client_coverage.first_source.is_some());
    assert_eq!(
        endpoint
            .handler
            .as_ref()
            .map(|handler| handler.label.as_str()),
        Some("list_users")
    );
    assert_eq!(endpoint.occurrence_count, 3);

    let mut handler_query = request(None, 100);
    handler_query.query = Some("list_users".into());
    let handler_page = store.list_api_endpoints(&handler_query).unwrap();
    assert_eq!(handler_page.total, 1);

    for term in ["Accounts", "OpenAPI", "JSON"] {
        let mut metadata_query = request(None, 100);
        metadata_query.query = Some(term.into());
        assert_eq!(
            store.list_api_endpoints(&metadata_query).unwrap().total,
            1,
            "server search should include {term} metadata"
        );
    }
}

#[test]
fn same_method_and_path_in_two_specs_remain_two_service_operations() {
    let spec = |title: &str| {
        format!(
            r#"{{"openapi":"3.1.0","info":{{"title":"{title}","version":"1"}},
                 "paths":{{"/health":{{"get":{{"tags":["Health"]}}}}}}}}"#
        )
    };
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("service-a/openapi.json", &spec("Service A")),
            indexed_file("service-b/openapi.json", &spec("Service B")),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 2);
    assert_eq!(page.counts.producer, 2);
    assert_eq!(
        page.endpoints
            .iter()
            .map(|endpoint| endpoint.grouping.segments[0].as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["Service A", "Service B"])
    );
}

#[test]
fn code_only_producer_does_not_collapse_into_another_services_spec() {
    let spec = r#"{"openapi":"3.1.0","info":{"title":"Service A","version":"1"},
                   "paths":{"/health":{"get":{"tags":["Health"]}}}}"#;
    let code_only = "function routes() { router.get('/health', health); }";
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("service-a/openapi.json", spec),
            indexed_file("service-b/src/routes.ts", code_only),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 2);
    assert_eq!(page.counts.producer, 2);
    assert_eq!(
        page.endpoints
            .iter()
            .map(|endpoint| endpoint.source_file.as_str())
            .collect::<HashSet<_>>(),
        HashSet::from(["service-a/openapi.json", "service-b/src/routes.ts"])
    );
}

#[test]
fn source_only_inventory_merges_producer_and_fetch_without_an_openapi_document() {
    let routes = r#"
function listUsers(_request, response) { response.end('[]'); }
function install() { usersRouter.get('/api/v1/users', listUsers); }
"#;
    let clients = r#"
async function load() {
  await fetch('/api/v1/users');
  await fetch('https://user:secret@payments.example/v1/charges?token=hidden', { method: 'POST' });
}
"#;
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("service/src/routes.ts", routes),
            indexed_file("web/src/client.ts", clients),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 2);
    assert_eq!(page.indexed_total, 2);
    assert_eq!(page.counts.producer, 1);
    assert_eq!(page.counts.consumer_only, 1);
    assert_eq!(page.counts.client_covered, 1);
    let producer = page
        .endpoints
        .iter()
        .find(|endpoint| endpoint.path == "/api/v1/users")
        .expect("source route must be inventoried");
    assert_eq!(producer.role, ApiEndpointRole::Producer);
    assert_eq!(producer.method, "GET");
    assert_eq!(producer.source_file, "service/src/routes.ts");
    assert_eq!(producer.framework, None);
    assert_eq!(producer.client_coverage.count, 1);
    assert_eq!(producer.occurrence_count, 2);
    let handler = producer
        .handler
        .as_ref()
        .expect("static source handler reference must be retained");
    assert_eq!(handler.label, "listUsers");
    assert_eq!(handler.kind, "handlerReference");
    assert_eq!(handler.evidence, EvidenceKind::Inferred);
    assert_eq!(
        handler
            .source
            .as_ref()
            .map(|source| source.relative_path.as_str()),
        Some("service/src/routes.ts")
    );

    let external = page
        .endpoints
        .iter()
        .find(|endpoint| endpoint.path.starts_with("https://"))
        .expect("sanitized external client operation must be inventoried");
    assert_eq!(external.role, ApiEndpointRole::Consumer);
    assert_eq!(external.method, "POST");
    assert_eq!(external.path, "https://payments.example/v1/charges");
    assert_eq!(external.framework.as_deref(), Some("Fetch"));
    let serialized = serde_json::to_string(&page).unwrap();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("token=hidden"));
}

#[test]
fn trace_demo_source_only_routes_have_inventory_handlers() {
    let source = include_str!("../../../../fixtures/trace-demo/src/server.js");
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("src/server.js", source)])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 3);
    assert_eq!(page.counts.producer, 3);
    assert_eq!(page.counts.consumer_only, 0);
    assert!(page.endpoints.iter().all(|endpoint| {
        endpoint.role == ApiEndpointRole::Producer
            && endpoint.method == "GET"
            && endpoint.source_file == "src/server.js"
            && endpoint.handler.as_ref().is_some_and(|handler| {
                handler.kind == "handler"
                    && handler.label == "inline handler"
                    && handler.evidence == EvidenceKind::Inferred
                    && handler.source.as_ref().is_some_and(|source| {
                        source.relative_path == "src/server.js"
                            && source.end_line > source.start_line
                    })
            })
    }));

    let mut handler_query = request(None, 100);
    handler_query.query = Some("inline handler".into());
    assert_eq!(
        store.list_api_endpoints(&handler_query).unwrap().total,
        3,
        "source-only handlers must participate in complete-index search"
    );
}

#[test]
fn query_filter_has_exact_total_and_query_bound_cursor() {
    let source = "function routes() { router.get('/api/v1/graph/status', status); router.post('/api/v1/graph/reconcile', reconcile); }";
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("service/src/graph.ts", source)])
        .unwrap();
    let mut filtered = request(None, 1);
    filtered.query = Some("graph".into());

    let page = store.list_api_endpoints(&filtered).unwrap();

    assert_eq!(page.total, 2);
    assert_eq!(page.indexed_total, 2);
    assert!(page.next_cursor.is_some());
    let mut wrong_query = request(page.next_cursor, 1);
    wrong_query.query = Some("reconcile".into());
    assert!(store.list_api_endpoints(&wrong_query).is_err());

    let mut exact = request(None, 100);
    exact.query = Some("graph/status".into());
    let exact_page = store.list_api_endpoints(&exact).unwrap();
    assert_eq!(exact_page.total, 1);
    assert_eq!(exact_page.endpoints[0].label, "GET /api/v1/graph/status");
}

#[test]
fn openapi_tag_search_filters_before_catalog_paging() {
    let paths = (0..205)
        .map(|index| {
            let tag = if index == 204 {
                "NeedleTag"
            } else {
                "CommonTag"
            };
            format!(r#""/items/{index}":{{"get":{{"tags":["{tag}"],"operationId":"op{index}"}}}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");
    let spec = format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"Catalog","version":"1"}},"paths":{{{paths}}}}}"#
    );
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("service/openapi.json", &spec)])
        .unwrap();

    let unfiltered = store.list_api_endpoints(&request(None, 200)).unwrap();
    assert_eq!(unfiltered.total, 205);
    assert!(unfiltered.truncated);

    let mut filtered = request(None, 200);
    filtered.query = Some("NeedleTag".into());
    let page = store.list_api_endpoints(&filtered).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.endpoints[0].grouping.segments[1], "NeedleTag");
}

#[test]
fn test_and_e2e_client_calls_do_not_inflate_the_production_catalog() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file(
                "web/src/client.ts",
                "function load() { api.get('/api/v1/real'); }",
            ),
            indexed_file(
                "web/src/contest.ts",
                "function contest() { api.get('/api/v1/contest'); }",
            ),
            indexed_file(
                "web/e2e/client.spec.ts",
                "function testOnly() { api.post('/api/v1/test-only'); }",
            ),
            indexed_file(
                "web/src/legacy_test.ts",
                "function testOnly() { api.post('/api/v1/legacy-test'); }",
            ),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 2);
    assert!(
        page.endpoints
            .iter()
            .any(|endpoint| endpoint.path == "/api/v1/real")
    );
    assert!(
        page.endpoints
            .iter()
            .any(|endpoint| endpoint.path == "/api/v1/contest")
    );
}

#[test]
fn exposes_sanitized_absolute_client_operations_without_secrets() {
    let source = r#"function load() {
      axios.get('https://user:password@api.stripe.com:8443/v1/customers?api_key=SECRET#x');
    }"#;
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("web/src/stripe.ts", source)])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.counts.consumer_only, 1);
    assert_eq!(page.counts.client_covered, 0);
    assert_eq!(
        page.endpoints[0].path,
        "https://api.stripe.com:8443/v1/customers"
    );
    let serialized = serde_json::to_string(&page).unwrap();
    assert!(!serialized.contains("SECRET"));
    assert!(!serialized.contains("password"));
}

#[test]
fn connect_producer_and_consumer_are_merged_searchable_and_pageable() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file(
                "service/src/proxy.ts",
                "function install() { router.connect('/proxy', tunnel); }",
            ),
            indexed_file(
                "web/src/client.ts",
                "function open() { client.connect('/proxy'); }",
            ),
        ])
        .unwrap();
    let mut query = request(None, 1);
    query.query = Some("CONNECT".into());

    let page = store.list_api_endpoints(&query).unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.returned, 1);
    assert!(!page.truncated);
    assert_eq!(page.counts.producer, 1);
    assert_eq!(page.counts.client_covered, 1);
    assert_eq!(page.endpoints[0].method, "CONNECT");
    assert_eq!(page.endpoints[0].path, "/proxy");
    assert_eq!(page.endpoints[0].client_coverage.count, 1);
}

#[test]
fn legacy_endpoint_without_role_is_unknown_until_rescan() {
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file(
            "legacy/src/routes.ts",
            "function routes() { router.get('/legacy', handler); }",
        )])
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE graph_nodes SET metadata_json = json_remove(metadata_json, '$.apiRole') WHERE kind = 'endpoint'",
            [],
        )
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.endpoints[0].role, ApiEndpointRole::Unknown);
    assert_eq!(page.counts.producer, 0);
    assert_eq!(page.counts.consumer_only, 0);
    assert_eq!(page.counts.unknown, 1);
}
