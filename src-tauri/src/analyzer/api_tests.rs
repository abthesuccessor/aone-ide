use serde_json::json;

use super::analyze_source;
use crate::domain::EvidenceKind;

#[test]
fn extracts_declared_utoipa_operation_with_handler_evidence() {
    let source = r#"
#[utoipa::path(
    operation_id = "findUser",
    get,
    path = "/api/v1/users/{user_id}",
    responses((status = 200, description = "ok"))
)]
pub async fn find_user() {}
"#;
    let analysis = analyze_source("workspace", "src/users.rs", source, "hash").unwrap();
    let endpoint = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "endpoint")
        .unwrap();

    assert_eq!(endpoint.label, "GET /api/v1/users/{user_id}");
    assert_eq!(endpoint.evidence, EvidenceKind::Declared);
    assert_eq!(endpoint.metadata.get("apiRole"), Some(&json!("producer")));
    assert_eq!(
        endpoint.metadata.get("apiFramework"),
        Some(&json!("Utoipa"))
    );
    assert_eq!(
        endpoint.metadata.get("operationId"),
        Some(&json!("findUser"))
    );
    assert_eq!(
        endpoint.source.as_ref().map(|source| source.start_line),
        Some(5)
    );
    assert!(analysis.edges.iter().any(|edge| {
        edge.kind == "handles"
            && edge.target == endpoint.id
            && edge.evidence == EvidenceKind::Declared
    }));
}

#[test]
fn extracts_source_only_fastapi_route_and_rejects_dynamic_decorators() {
    let source = r#"from fastapi import APIRouter
router = APIRouter()
@router.post("/login")
async def login(payload: dict):
    return authenticate(payload)

@router.get(route_path)
async def dynamic_name():
    return None

@router.get(f"/items/{item_id}")
async def formatted_path():
    return None

@router.get("/items/" + suffix)
async def joined_path():
    return None
"#;
    let analysis = analyze_source("workspace", "src/api/auth.py", source, "hash").unwrap();
    let endpoints = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "endpoint")
        .collect::<Vec<_>>();

    assert_eq!(endpoints.len(), 1);
    let endpoint = endpoints[0];
    assert_eq!(endpoint.label, "POST /login");
    assert_eq!(endpoint.evidence, EvidenceKind::Declared);
    assert_eq!(
        endpoint.metadata.get("apiFramework"),
        Some(&json!("FastAPI"))
    );
    assert_eq!(
        endpoint.metadata.get("apiServiceRoot"),
        Some(&json!("Workspace root"))
    );
    assert_eq!(endpoint.metadata.get("sourceExact"), Some(&json!(true)));
    assert_eq!(
        endpoint.source.as_ref().map(|source| source.start_line),
        Some(3)
    );

    let handler = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "function" && node.label == "login")
        .unwrap();
    let source = handler.source.as_ref().unwrap();
    assert_eq!((source.start_line, source.end_line), (4, 5));
    assert!(analysis.edges.iter().any(|edge| {
        edge.kind == "handles"
            && edge.source == handler.id
            && edge.target == endpoint.id
            && edge.evidence == EvidenceKind::Declared
    }));
    assert!(analysis.nodes.iter().all(|node| {
        node.kind != "apiService" && node.metadata.get("apiFramework") != Some(&json!("OpenAPI"))
    }));
}

#[test]
fn top_level_openapi_directory_uses_workspace_service_root() {
    let source = r#"{"openapi":"3.1.0","info":{"title":"Auth","version":"1"},"paths":{"/api/v1/login":{"post":{"operationId":"login_api_v1_login_post"}}}}"#;
    let analysis = analyze_source(
        "workspace",
        "openapi/generated/auth.openapi.json",
        source,
        "hash",
    )
    .unwrap();
    let endpoint = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "endpoint")
        .unwrap();

    assert_eq!(
        endpoint.metadata.get("apiServiceRoot"),
        Some(&json!("Workspace root"))
    );
}

#[test]
fn distinguishes_server_router_calls_from_http_client_calls() {
    let source = r#"
function install() { router.get('/api/v1/users', listUsers); }
function load() { api.get('/api/v1/users'); axios.post('/api/v1/users'); }
"#;
    let analysis = analyze_source("workspace", "src/users.ts", source, "hash").unwrap();
    let producer = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "endpoint")
        .unwrap();
    let consumers = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "api")
        .collect::<Vec<_>>();

    assert_eq!(producer.metadata.get("apiRole"), Some(&json!("producer")));
    assert_eq!(consumers.len(), 2);
    assert!(
        consumers
            .iter()
            .all(|node| node.metadata.get("apiRole") == Some(&json!("consumer")))
    );
    assert!(
        consumers
            .iter()
            .any(|node| node.label == "GET /api/v1/users")
    );
    assert!(
        consumers
            .iter()
            .any(|node| node.label == "POST /api/v1/users")
    );
}

#[test]
fn recognizes_static_connect_producer_and_consumer_calls() {
    let source = r#"
function install() { router.connect('/proxy', tunnel); }
function open() { client.connect('/proxy'); }
"#;
    let analysis = analyze_source("workspace", "src/proxy.ts", source, "hash").unwrap();
    let connect_nodes = analysis
        .nodes
        .iter()
        .filter(|node| node.metadata.get("httpMethod") == Some(&json!("CONNECT")))
        .collect::<Vec<_>>();

    assert_eq!(connect_nodes.len(), 2);
    assert!(connect_nodes.iter().any(|node| {
        node.kind == "endpoint" && node.metadata.get("apiRole") == Some(&json!("producer"))
    }));
    assert!(connect_nodes.iter().any(|node| {
        node.kind == "api" && node.metadata.get("apiRole") == Some(&json!("consumer"))
    }));
}

#[test]
fn rejects_unrelated_map_get_calls() {
    let source = "function read() { map.get('/api/v1/users'); }";
    let analysis = analyze_source("workspace", "src/map.ts", source, "hash").unwrap();

    assert!(
        analysis
            .nodes
            .iter()
            .all(|node| !matches!(node.kind.as_str(), "api" | "endpoint"))
    );
}

#[test]
fn extracts_source_only_trace_demo_routes_with_exact_inline_handler_sources() {
    let source = include_str!("../../../fixtures/trace-demo/src/server.js");
    let analysis = analyze_source("workspace", "src/server.js", source, "hash").unwrap();
    let mut endpoints = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "endpoint")
        .collect::<Vec<_>>();
    endpoints.sort_by(|left, right| left.label.cmp(&right.label));

    assert_eq!(
        endpoints
            .iter()
            .map(|endpoint| endpoint.label.as_str())
            .collect::<Vec<_>>(),
        [
            "GET /api/debug/fail",
            "GET /api/debug/stress",
            "GET /api/shipments/:shipmentId",
        ]
    );
    assert!(endpoints.iter().all(|endpoint| {
        endpoint.evidence == EvidenceKind::Inferred
            && endpoint.metadata.get("apiRole") == Some(&json!("producer"))
            && endpoint.metadata.get("sourceExact") == Some(&json!(true))
            && endpoint.metadata.get("apiTargetForm") == Some(&json!("workspaceRelative"))
            && endpoint.source.as_ref().is_some_and(|source| {
                source.relative_path == "src/server.js" && source.start_column == 12
            })
    }));

    let handlers = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "handler")
        .collect::<Vec<_>>();
    assert_eq!(handlers.len(), 3);
    assert!(handlers.iter().all(|handler| {
        handler.evidence == EvidenceKind::Declared
            && handler.metadata.get("sourceExact") == Some(&json!(true))
            && handler.metadata.get("referenceOnly") == Some(&json!(false))
    }));
    assert!(endpoints.iter().all(|endpoint| {
        analysis.edges.iter().any(|edge| {
            edge.kind == "handles"
                && edge.target == endpoint.id
                && edge.evidence == EvidenceKind::Inferred
                && handlers.iter().any(|handler| handler.id == edge.source)
        })
    }));
}

#[test]
fn extracts_fetch_defaults_and_static_request_configuration() {
    let source = r#"
async function load() {
  await fetch('/api/v1/default');
  await fetch('/api/v1/items', { method: 'POST' });
  await axios.request({ url: 'https://user:secret@example.test/v1/items?token=x', method: 'DELETE' });
  await fetch('/api/v1/dynamic', options);
  await fetch('/api/v1/shorthand', { method });
  await fetch('/api/v1/spread', { ...options });
}
"#;
    let analysis = analyze_source("workspace", "src/client.ts", source, "hash").unwrap();
    let api_nodes = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "api")
        .collect::<Vec<_>>();

    assert!(
        api_nodes
            .iter()
            .any(|node| node.label == "GET /api/v1/default")
    );
    assert!(
        api_nodes
            .iter()
            .any(|node| node.label == "POST /api/v1/items")
    );
    assert!(api_nodes.iter().any(|node| {
        node.label == "DELETE https://example.test/v1/items"
            && node.metadata.get("apiTargetForm") == Some(&json!("absoluteHttp"))
    }));
    let dynamic = api_nodes
        .iter()
        .find(|node| node.label == "fetch /api/v1/dynamic")
        .expect("dynamic fetch remains visible without a guessed method");
    assert!(!dynamic.metadata.contains_key("httpMethod"));
    for label in ["fetch /api/v1/shorthand", "fetch /api/v1/spread"] {
        let dynamic = api_nodes
            .iter()
            .find(|node| node.label == label)
            .expect("dynamic method must remain visible without a guessed method");
        assert!(!dynamic.metadata.contains_key("httpMethod"));
    }
    let serialized = serde_json::to_string(&api_nodes).unwrap();
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("token=x"));
}

#[test]
fn strips_credentials_query_and_fragment_before_graph_persistence() {
    let source = r#"
function load() {
  api.get('/x?token=RELATIVE_SECRET#fragment');
  axios.post('https://user:password@example.com:8443/x?key=ABSOLUTE_SECRET#fragment');
  fetch('https://name:pass@example.net/y?token=FETCH_SECRET#fragment');
}
"#;
    let analysis = analyze_source("workspace", "src/client.ts", source, "hash").unwrap();
    let serialized = serde_json::to_string(&analysis.nodes).unwrap();

    assert!(!serialized.contains("RELATIVE_SECRET"));
    assert!(!serialized.contains("ABSOLUTE_SECRET"));
    assert!(!serialized.contains("FETCH_SECRET"));
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("name:pass"));
    assert!(serialized.contains("GET /x"));
    assert!(serialized.contains("https://example.com:8443/x"));
    assert!(serialized.contains("https://example.net/y"));
}

#[test]
fn extracts_allowlisted_openapi_operations_without_secret_payloads() {
    let source = r#"{
  "openapi": "3.1.0",
  "info": {"title": "Accounts", "version": "1"},
  "paths": {
    "/api/v1/accounts": {
      "get": {
        "tags": ["Accounts"],
        "operationId": "listAccounts",
        "description": "must-not-persist SECRET_TOKEN_123",
        "security": [{"bearer": ["SECRET_SCOPE"]}],
        "responses": {"200": {"example": "SECRET_EXAMPLE"}}
      },
      "post": {"tags": ["Accounts"], "operationId": "createAccount"}
    }
  }
}"#;
    let analysis = analyze_source("workspace", "api/openapi.json", source, "hash").unwrap();
    let endpoints = analysis
        .nodes
        .iter()
        .filter(|node| node.kind == "endpoint")
        .collect::<Vec<_>>();

    assert_eq!(endpoints.len(), 2);
    assert!(endpoints.iter().all(|node| {
        node.evidence == EvidenceKind::Declared
            && node.metadata.get("apiFramework") == Some(&json!("OpenAPI"))
            && node.metadata.get("sourceExact") == Some(&json!(true))
    }));
    assert!(
        endpoints
            .iter()
            .any(|node| node.label == "GET /api/v1/accounts")
    );
    assert!(
        endpoints
            .iter()
            .any(|node| node.label == "POST /api/v1/accounts")
    );
    let serialized = serde_json::to_string(&analysis.nodes).unwrap();
    assert!(!serialized.contains("SECRET_TOKEN_123"));
    assert!(!serialized.contains("SECRET_SCOPE"));
    assert!(!serialized.contains("SECRET_EXAMPLE"));
}

#[test]
fn extracts_openapi_yaml_and_ignores_remote_references() {
    let source = r#"openapi: 3.1.0
info:
  title: Billing
  version: "1"
paths:
  /api/v1/invoices:
    get:
      tags: [Billing]
      operationId: listInvoices
      responses:
        "200":
          $ref: https://secret.invalid/schema.yaml
"#;
    let analysis = analyze_source("workspace", "api/openapi.yaml", source, "hash").unwrap();
    let endpoint = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "endpoint")
        .unwrap();

    assert_eq!(endpoint.label, "GET /api/v1/invoices");
    assert_eq!(
        endpoint.source.as_ref().map(|source| source.start_line),
        Some(7)
    );
    assert!(
        !serde_json::to_string(endpoint)
            .unwrap()
            .contains("secret.invalid")
    );
}

#[test]
fn resolves_only_bounded_local_openapi_path_item_references() {
    let source = r##"{
      "openapi":"3.1.0",
      "info":{"title":"Health","version":"1"},
      "paths":{"/health":{"$ref":"#/x-path-items/Health"}},
      "x-path-items":{"Health":{"get":{"tags":["Health"],"operationId":"health"}}}
    }"##;
    let analysis = analyze_source("workspace", "openapi.json", source, "hash").unwrap();
    let endpoint = analysis
        .nodes
        .iter()
        .find(|node| node.kind == "endpoint")
        .unwrap();

    assert_eq!(endpoint.label, "GET /health");
    assert_eq!(endpoint.metadata.get("sourceExact"), Some(&json!(false)));
}

#[test]
fn non_openapi_json_produces_only_its_file_fact() {
    let source = r#"{"paths":{"/secret":{"get":{"description":"not a spec"}}}}"#;
    let analysis = analyze_source("workspace", "data/config.json", source, "hash").unwrap();

    assert_eq!(analysis.nodes.len(), 1);
    assert_eq!(analysis.nodes[0].kind, "file");
}

#[test]
fn large_non_spec_yaml_skips_openapi_parsing() {
    let source = "ordinary: harmless configuration\n".repeat(32_768);
    let analysis = analyze_source("workspace", "data/config.yaml", &source, "hash").unwrap();

    assert_eq!(analysis.nodes.len(), 1);
    assert_eq!(analysis.nodes[0].kind, "file");
    assert!(!analysis.parse_errors);
}

#[test]
fn oversized_openapi_specs_are_bounded_and_honestly_truncated() {
    let paths = (0..5_100)
        .map(|index| format!(r#""/items/{index}":{{"get":{{}}}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        r#"{{"openapi":"3.1.0","info":{{"title":"Large","version":"1"}},"paths":{{{paths}}}}}"#
    );
    let analysis = analyze_source("workspace", "api/openapi.json", &source, "hash").unwrap();

    assert!(analysis.parse_errors);
    assert_eq!(analysis.fact_count, 5_000);
    assert_eq!(analysis.nodes.len(), 5_001);
    assert_eq!(
        analysis.nodes[0].metadata.get("analysisTruncated"),
        Some(&json!(true))
    );
}
