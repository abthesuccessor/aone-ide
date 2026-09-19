use serde_json::{Value, json};

use crate::{domain::GraphNode, scanner::is_hard_denied};

pub(super) fn decorate_node(node: &mut GraphNode, parent_id: Option<&str>, is_root: bool) {
    let stage = flow_stage(node);
    node.metadata
        .insert("flowStage".into(), Value::String(stage.into()));
    node.metadata
        .insert("flowGroup".into(), Value::String(flow_group(node)));
    node.metadata.insert("flowRoot".into(), json!(is_root));
    node.metadata.insert("flowStaticOnly".into(), json!(true));
    node.metadata.insert(
        "flowProjectionBasis".into(),
        Value::String("indexed static source facts".into()),
    );
    if let Some(parent_id) = parent_id {
        node.metadata
            .insert("flowParentId".into(), Value::String(parent_id.to_owned()));
    }
}

pub(super) fn set_child_metadata(node: &mut GraphNode, total: usize, shown: usize) {
    node.metadata.insert("flowChildCount".into(), json!(total));
    node.metadata
        .insert("flowExpandable".into(), json!(total > shown));
    node.metadata.insert(
        "flowOmittedChildCount".into(),
        json!(total.saturating_sub(shown)),
    );
}

pub(super) fn is_excluded_node(node: &GraphNode, include_tests: bool) -> bool {
    node.source.as_ref().is_some_and(|source| {
        is_sensitive_path(&source.relative_path)
            || (!include_tests && is_test_path(&source.relative_path))
    })
}

pub(super) fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let padded = format!("/{lower}");
    padded.starts_with("/test/")
        || padded.starts_with("/tests/")
        || padded.starts_with("/e2e/")
        || padded.contains("/test/")
        || padded.contains("/tests/")
        || padded.contains("/e2e/")
        || padded.contains("/__tests__/")
        || padded.contains("/fixtures/")
        || lower.contains(".test.")
        || lower.contains(".spec.")
        || lower.rsplit('/').next().is_some_and(|name| {
            name.contains("_test.")
                || name.contains("_tests.")
                || name.contains("_live_test.")
                || name.contains("_live_tests.")
        })
}

fn is_sensitive_path(path: &str) -> bool {
    is_hard_denied(std::path::Path::new(path))
}

fn flow_stage(node: &GraphNode) -> &'static str {
    let kind = node.kind.to_ascii_lowercase();
    let label = node.label.to_ascii_lowercase();
    let path = node
        .source
        .as_ref()
        .map(|source| source.relative_path.to_ascii_lowercase())
        .unwrap_or_default();
    let role = node
        .metadata
        .get("apiRole")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let route = node
        .metadata
        .get("routePath")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if kind == "event" {
        return "event";
    }
    let backend_path =
        path_component_matches(&path, &["backend", "server", "servers", "api", "src-tauri"]);
    let frontend_path = path_component_matches(
        &path,
        &["frontend", "web", "ui", "components", "pages", "views"],
    ) || path.ends_with(".tsx");
    if matches!(
        kind.as_str(),
        "database"
            | "repository"
            | "model"
            | "entity"
            | "schema"
            | "table"
            | "store"
            | "cache"
            | "dao"
    ) || label.contains("repository")
        || (backend_path
            && path_component_matches(&path, &["repository", "repositories", "storage"]))
    {
        return "data";
    }
    if kind == "agent"
        || label.contains("agent")
        || path_component_matches(&path, &["agent", "agents"])
    {
        return "agent";
    }
    if kind == "api" && role == "consumer" && route.starts_with("http") {
        return "external";
    }
    if kind == "api" && role == "consumer" && frontend_path {
        return "frontend";
    }
    if kind == "endpoint" || kind == "api" || kind == "apiservice" || kind == "apigroup" {
        return "api";
    }
    if frontend_path {
        return "frontend";
    }
    if matches!(
        kind.as_str(),
        "service" | "trait" | "implementation" | "method" | "class" | "utility"
    ) || label.contains("service")
        || path_component_matches(&path, &["service", "services", "utility", "utilities"])
    {
        return "service";
    }
    if matches!(
        kind.as_str(),
        "function" | "controller" | "handler" | "entrypoint"
    ) {
        return "backend";
    }
    "other"
}

fn flow_group(node: &GraphNode) -> String {
    let Some(path) = node
        .source
        .as_ref()
        .map(|source| source.relative_path.as_str())
    else {
        return "Workspace".into();
    };
    if let Some((root, _)) = path.split_once("/src/") {
        return root.to_owned();
    }
    path.split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("Workspace")
        .to_owned()
}

fn path_component_matches(path: &str, candidates: &[&str]) -> bool {
    path.split(['/', '_', '-', '.'])
        .any(|part| candidates.contains(&part))
}
