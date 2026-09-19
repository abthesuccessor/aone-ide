use tempfile::tempdir;

use super::{GraphStore, indexed_file, request};
use crate::domain::{ApiEndpointRole, EvidenceKind};

#[test]
fn source_only_fastapi_route_has_exact_inventory_handler() {
    let source = r#"from fastapi import APIRouter
router = APIRouter()

@router.post("/login")
async def login(payload: dict):
    return authenticate(payload)
"#;
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[indexed_file("src/api/auth.py", source)])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();

    assert_eq!(page.total, 1);
    let endpoint = &page.endpoints[0];
    assert_eq!(endpoint.role, ApiEndpointRole::Producer);
    assert_eq!(endpoint.path, "/login");
    assert_eq!(endpoint.framework.as_deref(), Some("FastAPI"));
    let handler = endpoint.handler.as_ref().unwrap();
    assert_eq!(handler.label, "login");
    assert_eq!(handler.evidence, EvidenceKind::Declared);
    assert_eq!(
        handler.source.as_ref().map(|source| {
            (
                source.relative_path.as_str(),
                source.start_line,
                source.end_line,
            )
        }),
        Some(("src/api/auth.py", 5, 6))
    );
}

#[test]
fn openapi_mount_prefix_uses_only_the_unique_inferred_fastapi_handler() {
    let specification = r#"{"openapi":"3.1.0","info":{"title":"Auth","version":"1"},"paths":{"/api/v1/login":{"post":{"operationId":"login_api_v1_login_post"}}}}"#;
    let source = r#"from fastapi import APIRouter
router = APIRouter()
@router.post("/login")
async def login():
    return None
"#;
    let directory = tempdir().unwrap();
    let mut store = GraphStore::open(&directory.path().join("aone.sqlite")).unwrap();
    store
        .replace_all(&[
            indexed_file("openapi/auth.openapi.json", specification),
            indexed_file("src/api/auth.py", source),
        ])
        .unwrap();

    let page = store.list_api_endpoints(&request(None, 100)).unwrap();
    let endpoint = page
        .endpoints
        .iter()
        .find(|endpoint| endpoint.path == "/api/v1/login")
        .unwrap();
    let handler = endpoint.handler.as_ref().unwrap();
    assert_eq!(handler.label, "login");
    assert_eq!(handler.evidence, EvidenceKind::Inferred);
    assert_eq!(
        handler
            .source
            .as_ref()
            .map(|source| (source.relative_path.as_str(), source.start_line)),
        Some(("src/api/auth.py", 4))
    );

    store
        .replace_all(&[
            indexed_file("openapi/auth.openapi.json", specification),
            indexed_file("src/api/auth.py", source),
            indexed_file("src/legacy/auth.py", source),
        ])
        .unwrap();
    let ambiguous = store.list_api_endpoints(&request(None, 100)).unwrap();
    let openapi = ambiguous
        .endpoints
        .iter()
        .find(|endpoint| endpoint.path == "/api/v1/login")
        .unwrap();
    assert!(openapi.handler.is_none());
}
