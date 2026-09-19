use std::collections::BTreeMap;

use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use super::links::FlowLink;
use crate::{
    analyzer::stable_id,
    domain::{EvidenceKind, GraphEdge, GraphNode},
    error::AoneResult,
    store::{
        GraphStore,
        records::{evidence_from_db, node_from_row},
    },
};

#[derive(Clone, Copy)]
enum HandlerProjection {
    ExactRoute,
    OperationIdPrefix,
}

struct HandlerMatch {
    owner: GraphNode,
    relation_id: String,
    evidence: EvidenceKind,
    confidence: Option<f64>,
    operation_id: String,
    projection: HandlerProjection,
}

pub(super) fn endpoint_client_count(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
) -> AoneResult<usize> {
    let Some((method, path, service)) = operation_identity(endpoint) else {
        return Ok(0);
    };
    if !producer_service_is_unique(store, &method, &path, &service, include_tests)? {
        return Ok(0);
    }
    let filter = path_filter(include_tests, "file_path");
    let sql = format!(
        "SELECT COUNT(*) FROM graph_nodes
         WHERE kind = 'api'
           AND json_extract(metadata_json, '$.apiRole') = 'consumer'
           AND upper(json_extract(metadata_json, '$.httpMethod')) = ?1
           AND json_extract(metadata_json, '$.routePath') = ?2{filter}"
    );
    let count = store
        .connection
        .query_row(&sql, [&method, &path], |row| row.get::<_, i64>(0))?;
    Ok(count.max(0) as usize)
}

pub(super) fn endpoint_clients_page(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
    offset: usize,
    limit: usize,
) -> AoneResult<Vec<FlowLink>> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let Some((method, path, service)) = operation_identity(endpoint) else {
        return Ok(Vec::new());
    };
    if !producer_service_is_unique(store, &method, &path, &service, include_tests)? {
        return Ok(Vec::new());
    }
    let filter = path_filter(include_tests, "file_path");
    let sql = format!(
        "SELECT id, kind, label, source_json, language, evidence, metadata_json
         FROM graph_nodes
         WHERE kind = 'api'
           AND json_extract(metadata_json, '$.apiRole') = 'consumer'
           AND upper(json_extract(metadata_json, '$.httpMethod')) = ?1
           AND json_extract(metadata_json, '$.routePath') = ?2{filter}
         ORDER BY file_path, id LIMIT ?3 OFFSET ?4"
    );
    let mut statement = store.connection.prepare(&sql)?;
    let rows = statement.query_map(
        rusqlite::params![method, path, limit as i64, offset as i64],
        node_from_row,
    )?;
    rows.map(|row| row.map(|client| client_route_link(endpoint, client)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub(super) fn unique_producer(
    store: &GraphStore,
    client: &GraphNode,
    include_tests: bool,
) -> AoneResult<Option<GraphNode>> {
    let Some((method, path, _)) = operation_identity(client) else {
        return Ok(None);
    };
    let filter = path_filter(include_tests, "file_path");
    let service_sql = format!(
        "SELECT DISTINCT COALESCE(json_extract(metadata_json, '$.apiServiceRoot'), file_path)
         FROM graph_nodes WHERE kind = 'endpoint'
           AND json_extract(metadata_json, '$.apiRole') = 'producer'
           AND upper(json_extract(metadata_json, '$.httpMethod')) = ?1
           AND json_extract(metadata_json, '$.routePath') = ?2{filter}
         ORDER BY 1 LIMIT 2"
    );
    let mut statement = store.connection.prepare(&service_sql)?;
    let services = statement
        .query_map([&method, &path], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if services.len() != 1 {
        return Ok(None);
    }
    let sql = format!(
        "SELECT id, kind, label, source_json, language, evidence, metadata_json
         FROM graph_nodes WHERE kind = 'endpoint'
           AND json_extract(metadata_json, '$.apiRole') = 'producer'
           AND upper(json_extract(metadata_json, '$.httpMethod')) = ?1
           AND json_extract(metadata_json, '$.routePath') = ?2
           AND COALESCE(json_extract(metadata_json, '$.apiServiceRoot'), file_path) = ?3{filter}
         ORDER BY CASE json_extract(metadata_json, '$.apiFramework')
                    WHEN 'Utoipa' THEN 0 WHEN 'OpenAPI' THEN 2 ELSE 1 END,
                  file_path, id LIMIT 1"
    );
    store
        .connection
        .query_row(&sql, [&method, &path, &services[0]], node_from_row)
        .optional()
        .map_err(Into::into)
}

pub(super) fn endpoint_handler_count(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
) -> AoneResult<usize> {
    Ok(usize::from(
        unique_handler_match(store, endpoint, include_tests)?.is_some(),
    ))
}

pub(super) fn endpoint_handlers_page(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
    offset: usize,
    limit: usize,
) -> AoneResult<Vec<FlowLink>> {
    if limit == 0 || offset > 0 {
        return Ok(Vec::new());
    }
    Ok(unique_handler_match(store, endpoint, include_tests)?
        .map(|matched| vec![handler_link(endpoint, matched)])
        .unwrap_or_default())
}

fn unique_handler_match(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
) -> AoneResult<Option<HandlerMatch>> {
    let Some((method, path, service)) = operation_identity(endpoint) else {
        return Ok(None);
    };
    let filter = path_filter(include_tests, "owner.file_path");
    let sql = format!(
        "SELECT owner.id, owner.kind, owner.label, owner.source_json, owner.language,
                owner.evidence, owner.metadata_json, MIN(relation.id), MIN(relation.evidence),
                MIN(relation.confidence), MIN(operation.id)
         FROM graph_nodes AS operation
         JOIN graph_edges AS relation ON relation.target = operation.id AND relation.kind = 'handles'
         JOIN graph_nodes AS owner ON owner.id = relation.source
         WHERE operation.kind = 'endpoint'
           AND upper(json_extract(operation.metadata_json, '$.httpMethod')) = ?1
           AND json_extract(operation.metadata_json, '$.routePath') = ?2
           AND COALESCE(json_extract(operation.metadata_json, '$.apiServiceRoot'), operation.file_path) = ?3{filter}
         GROUP BY owner.id, owner.kind, owner.label, owner.source_json, owner.language,
                  owner.evidence, owner.metadata_json
         ORDER BY owner.file_path, owner.id LIMIT 2"
    );
    let exact = handler_candidates(
        store,
        &sql,
        rusqlite::params![method, path, service],
        HandlerProjection::ExactRoute,
    )?;
    if exact.len() == 1 {
        return Ok(exact.into_iter().next());
    }
    if !exact.is_empty() {
        return Ok(None);
    }
    unique_operation_id_handler(store, endpoint, include_tests)
}

fn unique_operation_id_handler(
    store: &GraphStore,
    endpoint: &GraphNode,
    include_tests: bool,
) -> AoneResult<Option<HandlerMatch>> {
    if endpoint
        .metadata
        .get("apiFramework")
        .and_then(Value::as_str)
        != Some("OpenAPI")
    {
        return Ok(None);
    }
    let Some(operation_id) = endpoint.metadata.get("operationId").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some((method, path, service)) = operation_identity(endpoint) else {
        return Ok(None);
    };
    let filter = path_filter(include_tests, "owner.file_path");
    let sql = format!(
        "SELECT owner.id, owner.kind, owner.label, owner.source_json, owner.language,
                owner.evidence, owner.metadata_json, MIN(relation.id), MIN(relation.evidence),
                MIN(relation.confidence), MIN(operation.id)
         FROM graph_nodes AS operation
         JOIN graph_edges AS relation ON relation.target = operation.id AND relation.kind = 'handles'
         JOIN graph_nodes AS owner ON owner.id = relation.source
         WHERE operation.kind = 'endpoint'
           AND owner.kind IN ('function', 'method', 'handler')
           AND json_extract(operation.metadata_json, '$.apiRole') = 'producer'
           AND json_extract(operation.metadata_json, '$.apiFramework') = 'FastAPI'
           AND upper(json_extract(operation.metadata_json, '$.httpMethod')) = ?1
           AND json_extract(operation.metadata_json, '$.routePath') <> '/'
           AND length(json_extract(operation.metadata_json, '$.routePath')) < length(?2)
           AND substr(?2, -length(json_extract(operation.metadata_json, '$.routePath')))
                 = json_extract(operation.metadata_json, '$.routePath')
           AND substr(?4, 1, length(owner.label) + 1) = owner.label || '_'
           AND COALESCE(json_extract(operation.metadata_json, '$.apiServiceRoot'), operation.file_path) = ?3{filter}
         GROUP BY owner.id, owner.kind, owner.label, owner.source_json, owner.language,
                  owner.evidence, owner.metadata_json
         ORDER BY owner.file_path, owner.id LIMIT 2"
    );
    let candidates = handler_candidates(
        store,
        &sql,
        rusqlite::params![method, path, service, operation_id],
        HandlerProjection::OperationIdPrefix,
    )?;
    Ok((candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten())
}

fn handler_candidates<P: rusqlite::Params>(
    store: &GraphStore,
    sql: &str,
    params: P,
    projection: HandlerProjection,
) -> AoneResult<Vec<HandlerMatch>> {
    let mut statement = store.connection.prepare(sql)?;
    let rows = statement.query_map(params, |row| {
        Ok(HandlerMatch {
            owner: node_from_row(row)?,
            relation_id: row.get(7)?,
            evidence: evidence_from_db(&row.get::<_, String>(8)?),
            confidence: row.get(9)?,
            operation_id: row.get(10)?,
            projection,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn handler_link(endpoint: &GraphNode, matched: HandlerMatch) -> FlowLink {
    let fallback = matches!(matched.projection, HandlerProjection::OperationIdPrefix);
    let projection_basis = if fallback {
        "unique same-service FastAPI handler inferred from the OpenAPI operationId prefix and mounted-path suffix"
    } else {
        "same service, HTTP method, and path as a source-declared handler endpoint"
    };
    let mut metadata = BTreeMap::from([
        ("sourceEdgeIds".into(), json!([matched.relation_id])),
        ("bridgedEndpointId".into(), json!(matched.operation_id)),
        ("projectionBasis".into(), json!(projection_basis)),
        ("staticOnly".into(), json!(true)),
    ]);
    if fallback {
        metadata.insert(
            "handlerMatch".into(),
            json!("operationIdPrefixAndPathSuffix"),
        );
    }
    FlowLink {
        edge: GraphEdge {
            id: stable_id("flow-handler", &[&endpoint.id, &matched.relation_id]),
            source: endpoint.id.clone(),
            target: matched.owner.id.clone(),
            kind: "handledBy".into(),
            evidence: if fallback {
                EvidenceKind::Inferred
            } else {
                matched.evidence
            },
            confidence: if fallback {
                Some(0.75)
            } else {
                matched.confidence
            },
            metadata,
        },
        node: matched.owner,
    }
}

fn producer_service_is_unique(
    store: &GraphStore,
    method: &str,
    path: &str,
    expected_service: &str,
    include_tests: bool,
) -> AoneResult<bool> {
    let filter = path_filter(include_tests, "file_path");
    let sql = format!(
        "SELECT COUNT(DISTINCT COALESCE(json_extract(metadata_json, '$.apiServiceRoot'), file_path)),
                MIN(COALESCE(json_extract(metadata_json, '$.apiServiceRoot'), file_path))
         FROM graph_nodes WHERE kind = 'endpoint'
           AND json_extract(metadata_json, '$.apiRole') = 'producer'
           AND upper(json_extract(metadata_json, '$.httpMethod')) = ?1
           AND json_extract(metadata_json, '$.routePath') = ?2{filter}"
    );
    let (count, service) = store.connection.query_row(&sql, [method, path], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    Ok(count == 1 && service.as_deref() == Some(expected_service))
}

fn operation_identity(node: &GraphNode) -> Option<(String, String, String)> {
    let method = node
        .metadata
        .get("httpMethod")?
        .as_str()?
        .to_ascii_uppercase();
    let path = node.metadata.get("routePath")?.as_str()?.to_owned();
    let service = node
        .metadata
        .get("apiServiceRoot")
        .and_then(Value::as_str)
        .or_else(|| {
            node.source
                .as_ref()
                .map(|source| source.relative_path.as_str())
        })?
        .to_owned();
    Some((method, path, service))
}

pub(super) fn route_link(client: &GraphNode, producer: GraphNode) -> FlowLink {
    FlowLink {
        node: producer.clone(),
        edge: route_edge(client, &producer),
    }
}

fn client_route_link(endpoint: &GraphNode, client: GraphNode) -> FlowLink {
    FlowLink {
        edge: route_edge(&client, endpoint),
        node: client,
    }
}

fn route_edge(client: &GraphNode, producer: &GraphNode) -> GraphEdge {
    GraphEdge {
        id: stable_id("flow-route", &[&client.id, &producer.id]),
        source: client.id.clone(),
        target: producer.id.clone(),
        kind: "routesTo".into(),
        evidence: EvidenceKind::Resolved,
        confidence: Some(1.0),
        metadata: BTreeMap::from([
            (
                "projectionBasis".into(),
                json!("unique producer service with exact HTTP method and sanitized path"),
            ),
            ("staticOnly".into(), json!(true)),
        ]),
    }
}

fn path_filter(include_tests: bool, column: &str) -> String {
    super::links::path_filter(include_tests, column)
}
