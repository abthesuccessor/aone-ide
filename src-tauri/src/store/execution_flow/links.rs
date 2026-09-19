use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use super::{
    http::{endpoint_handler_count, endpoint_handlers_page, route_link, unique_producer},
    metadata::is_excluded_node,
};
use crate::{
    analyzer::stable_id,
    domain::{EvidenceKind, GraphEdge, GraphNode},
    error::AoneResult,
    store::{
        GraphStore,
        records::{edge_from_row, node_from_row},
    },
};

#[derive(Clone)]
pub(super) struct FlowLink {
    pub(super) node: GraphNode,
    pub(super) edge: GraphEdge,
}

pub(super) struct FlowLinkPage {
    pub(super) links: Vec<FlowLink>,
    pub(super) total: usize,
}

pub(super) fn links_for_node_page(
    store: &GraphStore,
    node: &GraphNode,
    include_tests: bool,
) -> AoneResult<FlowLinkPage> {
    links_for_node_page_at(store, node, include_tests, 0, 40)
}

pub(super) fn links_for_node_page_at(
    store: &GraphStore,
    node: &GraphNode,
    include_tests: bool,
    offset: usize,
    limit: usize,
) -> AoneResult<FlowLinkPage> {
    let outgoing_total = outgoing_count(store, node, include_tests)?;
    let reverse_total = reverse_entry_count(store, node, include_tests)?;
    let handler_total = if node.kind == "endpoint" {
        endpoint_handler_count(store, node, include_tests)?
    } else {
        0
    };
    let producer = if node.kind == "api"
        && node.metadata.get("apiRole").and_then(Value::as_str) == Some("consumer")
    {
        unique_producer(store, node, include_tests)?
    } else {
        None
    };
    let total = outgoing_total
        .saturating_add(reverse_total)
        .saturating_add(handler_total)
        .saturating_add(usize::from(producer.is_some()));
    let mut links = Vec::with_capacity(limit.min(total.saturating_sub(offset)));
    if offset < handler_total && links.len() < limit {
        links.extend(endpoint_handlers_page(
            store,
            node,
            include_tests,
            offset,
            (limit - links.len()).min(handler_total - offset),
        )?);
    }
    let outgoing_offset = offset.saturating_sub(handler_total);
    if outgoing_offset < outgoing_total && links.len() < limit {
        links.extend(outgoing_links(
            store,
            node,
            include_tests,
            outgoing_offset,
            (limit - links.len()).min(outgoing_total - outgoing_offset),
        )?);
    }
    let reverse_offset = offset.saturating_sub(handler_total.saturating_add(outgoing_total));
    if reverse_offset < reverse_total && links.len() < limit {
        links.extend(reverse_entry_links(
            store,
            node,
            include_tests,
            reverse_offset,
            (limit - links.len()).min(reverse_total - reverse_offset),
        )?);
    }
    if links.len() < limit
        && offset < total
        && offset.saturating_add(links.len())
            >= handler_total
                .saturating_add(outgoing_total)
                .saturating_add(reverse_total)
        && let Some(producer) = producer
    {
        links.push(route_link(node, producer));
    }
    Ok(FlowLinkPage { links, total })
}

fn outgoing_links(
    store: &GraphStore,
    owner: &GraphNode,
    include_tests: bool,
    offset: usize,
    limit: usize,
) -> AoneResult<Vec<FlowLink>> {
    let filter = path_filter(include_tests, "target.file_path");
    let sql = format!(
        "SELECT edge.id, edge.source, edge.target, edge.kind, edge.evidence,
                edge.confidence, edge.metadata_json
         FROM graph_edges AS edge JOIN graph_nodes AS target ON target.id = edge.target
         WHERE edge.source = ?1
           AND edge.kind IN ('calls','requests','queries','accesses','emits','listensTo'){filter}
         ORDER BY edge.id LIMIT ?2 OFFSET ?3"
    );
    let mut statement = store.connection.prepare(&sql)?;
    let edges = statement
        .query_map(
            params![owner.id, limit as i64, offset as i64],
            edge_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut links = Vec::with_capacity(edges.len());
    for edge in edges {
        let Some(child) = load_node(store, &edge.target, include_tests)? else {
            continue;
        };
        let (target, projected) = if edge.kind == "calls" && child.kind == "callTarget" {
            collapse_call(store, owner, child, edge, include_tests)?
        } else {
            (child, edge)
        };
        if !is_excluded_node(&target, include_tests) {
            links.push(FlowLink {
                node: target,
                edge: projected,
            });
        }
    }
    Ok(links)
}

fn outgoing_count(store: &GraphStore, owner: &GraphNode, include_tests: bool) -> AoneResult<usize> {
    let filter = path_filter(include_tests, "target.file_path");
    let sql = format!(
        "SELECT COUNT(*) FROM graph_edges AS edge
         JOIN graph_nodes AS target ON target.id = edge.target
         WHERE edge.source = ?1
           AND edge.kind IN ('calls','requests','queries','accesses','emits','listensTo'){filter}"
    );
    let count = store
        .connection
        .query_row(&sql, params![owner.id], |row| row.get::<_, i64>(0))?;
    Ok(count.max(0) as usize)
}

fn collapse_call(
    store: &GraphStore,
    owner: &GraphNode,
    call_target: GraphNode,
    call_edge: GraphEdge,
    include_tests: bool,
) -> AoneResult<(GraphNode, GraphEdge)> {
    let mut statement = store.connection.prepare(
        "SELECT id, source, target, kind, evidence, confidence, metadata_json
         FROM graph_edges WHERE source = ?1 AND kind = 'resolvesTo' ORDER BY id LIMIT 2",
    )?;
    let resolutions = statement
        .query_map(params![call_target.id], edge_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    if resolutions.len() != 1 {
        return Ok((call_target, call_edge));
    }
    let resolution = &resolutions[0];
    let Some(target) = load_node(store, &resolution.target, include_tests)? else {
        return Ok((call_target, call_edge));
    };
    if is_excluded_node(&target, include_tests) {
        return Ok((call_target, call_edge));
    }
    if resolution.evidence == EvidenceKind::Inferred
        && owner.language.as_deref() != target.language.as_deref()
    {
        return Ok((call_target, call_edge));
    }
    let mut target = target;
    if resolution.evidence == EvidenceKind::Inferred {
        target
            .metadata
            .insert("flowCandidateOnly".into(), json!(true));
        target.metadata.insert(
            "flowCandidateReason".into(),
            json!("workspace-global identifier match; lexical binding was not proven"),
        );
    }
    let evidence = combined_evidence(call_edge.evidence, resolution.evidence);
    let confidence = match (call_edge.confidence, resolution.confidence) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (value, None) | (None, value) => value,
    };
    let mut metadata = BTreeMap::from([
        ("sourceEdgeIds".into(), json!([call_edge.id, resolution.id])),
        (
            "projectionBasis".into(),
            json!("collapsed calls plus resolvesTo static facts"),
        ),
        ("staticOnly".into(), json!(true)),
    ]);
    if let Some(basis) = resolution.metadata.get("basis") {
        metadata.insert("resolutionBasis".into(), basis.clone());
    }
    Ok((
        target.clone(),
        GraphEdge {
            id: stable_id("flow-call", &[&call_edge.id, &resolution.id]),
            source: owner.id.clone(),
            target: target.id,
            kind: "calls".into(),
            evidence,
            confidence,
            metadata,
        },
    ))
}

fn reverse_entry_links(
    store: &GraphStore,
    node: &GraphNode,
    include_tests: bool,
    offset: usize,
    limit: usize,
) -> AoneResult<Vec<FlowLink>> {
    let reverse_kind = match node.kind.as_str() {
        "api" => Some(("requests", "requestedBy")),
        "event"
            if node.metadata.get("eventDirection").and_then(Value::as_str)
                == Some("subscription") =>
        {
            Some(("listensTo", "handledBy"))
        }
        _ => None,
    };
    let Some((stored_kind, projected_kind)) = reverse_kind else {
        return Ok(Vec::new());
    };
    let filter = path_filter(include_tests, "owner.file_path");
    let sql = format!(
        "SELECT edge.id, edge.source, edge.target, edge.kind, edge.evidence,
                edge.confidence, edge.metadata_json
         FROM graph_edges AS edge JOIN graph_nodes AS owner ON owner.id = edge.source
         WHERE edge.target = ?1 AND edge.kind = ?2{filter}
         ORDER BY edge.id LIMIT ?3 OFFSET ?4"
    );
    let mut statement = store.connection.prepare(&sql)?;
    let edges = statement
        .query_map(
            params![node.id, stored_kind, limit as i64, offset as i64],
            edge_from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let mut links = Vec::new();
    for stored in edges {
        let Some(owner) = load_node(store, &stored.source, include_tests)? else {
            continue;
        };
        if is_excluded_node(&owner, include_tests) {
            continue;
        }
        links.push(FlowLink {
            node: owner.clone(),
            edge: GraphEdge {
                id: stable_id("flow-reverse", &[&stored.id]),
                source: node.id.clone(),
                target: owner.id,
                kind: projected_kind.into(),
                evidence: stored.evidence,
                confidence: stored.confidence,
                metadata: BTreeMap::from([
                    ("sourceEdgeIds".into(), json!([stored.id])),
                    ("projectionDirection".into(), json!("reversed")),
                    ("projectionBasis".into(), json!("stored handler ownership")),
                    ("staticOnly".into(), json!(true)),
                ]),
            },
        });
    }
    Ok(links)
}

fn reverse_entry_count(
    store: &GraphStore,
    node: &GraphNode,
    include_tests: bool,
) -> AoneResult<usize> {
    let stored_kind = match node.kind.as_str() {
        "api" => Some("requests"),
        "event"
            if node.metadata.get("eventDirection").and_then(Value::as_str)
                == Some("subscription") =>
        {
            Some("listensTo")
        }
        _ => None,
    };
    let Some(stored_kind) = stored_kind else {
        return Ok(0);
    };
    let filter = path_filter(include_tests, "owner.file_path");
    let sql = format!(
        "SELECT COUNT(*) FROM graph_edges AS edge
         JOIN graph_nodes AS owner ON owner.id = edge.source
         WHERE edge.target = ?1 AND edge.kind = ?2{filter}"
    );
    let count = store
        .connection
        .query_row(&sql, params![node.id, stored_kind], |row| {
            row.get::<_, i64>(0)
        })?;
    Ok(count.max(0) as usize)
}

fn load_node(store: &GraphStore, id: &str, include_tests: bool) -> AoneResult<Option<GraphNode>> {
    let filter = path_filter(include_tests, "file_path");
    let sql = format!(
        "SELECT id, kind, label, source_json, language, evidence, metadata_json
         FROM graph_nodes WHERE id = ?1{filter}"
    );
    store
        .connection
        .query_row(&sql, params![id], node_from_row)
        .optional()
        .map_err(Into::into)
}

fn combined_evidence(left: EvidenceKind, right: EvidenceKind) -> EvidenceKind {
    if left == EvidenceKind::Inferred || right == EvidenceKind::Inferred {
        EvidenceKind::Inferred
    } else if left == EvidenceKind::Resolved || right == EvidenceKind::Resolved {
        EvidenceKind::Resolved
    } else {
        EvidenceKind::Declared
    }
}

pub(super) fn path_filter(include_tests: bool, column: &str) -> String {
    let mut filter = format!(" AND aone_path_allowed({column}) = 1");
    if !include_tests {
        filter.push_str(&format!(
            " AND lower({column}) NOT LIKE 'test/%'
              AND lower({column}) NOT LIKE 'tests/%'
              AND lower({column}) NOT LIKE 'e2e/%'
              AND lower({column}) NOT LIKE '%/test/%'
              AND lower({column}) NOT LIKE '%/tests/%'
              AND lower({column}) NOT LIKE '%/e2e/%'
              AND lower({column}) NOT LIKE '%/__tests__/%'
              AND lower({column}) NOT LIKE '%/fixtures/%'
              AND lower({column}) NOT LIKE '%.test.%'
              AND lower({column}) NOT LIKE '%.spec.%'
              AND lower({column}) NOT GLOB '*_test.*'
              AND lower({column}) NOT GLOB '*_tests.*'
              AND lower({column}) NOT GLOB '*_live_test.*'
              AND lower({column}) NOT GLOB '*_live_tests.*'"
        ));
    }
    filter
}
