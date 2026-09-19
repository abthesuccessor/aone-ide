use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

use super::{
    http::{endpoint_client_count, endpoint_clients_page},
    links::{FlowLink, links_for_node_page, links_for_node_page_at, path_filter},
    metadata::{decorate_node, is_excluded_node, set_child_metadata},
};
use crate::{
    domain::{GraphNode, GraphQuery, GraphSnapshot},
    error::{AoneError, AoneResult},
    graph_algorithms::{annotate_structural_communities, strongly_connected_components},
    store::{GraphStore, records::node_from_row},
};

const DEFAULT_FLOW_NODES: usize = 120;
const MAX_FLOW_NODES: usize = 500;
const DEFAULT_FLOW_DEPTH: usize = 4;
const MAX_FLOW_DEPTH: usize = 4;
const MAX_ROOT_PAGE_LINKS: usize = 100;
const MAX_DESCENDANT_LINKS: usize = 40;
const MAX_FLOW_EXPANSIONS: usize = 500;
const MAX_FLOW_LINKS_SCANNED: usize = 20_000;
const MAX_FLOW_EDGES: usize = 2_000;
const MAX_CURSOR_CHARS: usize = 96;
const MAX_ROOT_ID_CHARS: usize = 200;

pub(in crate::store) fn query_execution_flow(
    store: &GraphStore,
    query: &GraphQuery,
) -> AoneResult<GraphSnapshot> {
    validate_query(query)?;
    let root_id = &query.root_ids[0];
    let mut root = load_node(store, root_id, query.include_tests)?.ok_or_else(|| {
        AoneError::InvalidRequest("execution-flow root does not exist in this workspace".into())
    })?;
    if is_excluded_node(&root, query.include_tests) {
        return Err(AoneError::InvalidRequest(
            "execution-flow root is excluded by the production-source policy".into(),
        ));
    }
    let limit = query
        .limit
        .unwrap_or(DEFAULT_FLOW_NODES)
        .clamp(2, MAX_FLOW_NODES);
    let depth_limit = query
        .depth
        .unwrap_or(DEFAULT_FLOW_DEPTH)
        .min(MAX_FLOW_DEPTH);
    let offset = decode_cursor(query.cursor.as_deref(), query)?;

    decorate_node(&mut root, None, true);
    let base_root_total = links_for_node_page_at(store, &root, query.include_tests, 0, 0)?.total;
    let client_total = if root.kind == "endpoint" {
        endpoint_client_count(store, &root, query.include_tests)?
    } else {
        0
    };
    let total_root_links = client_total.saturating_add(base_root_total);
    if offset > total_root_links {
        return Err(AoneError::InvalidRequest(
            "execution-flow cursor is beyond the available root relationships".into(),
        ));
    }
    let root_page_capacity = limit.saturating_sub(1).min(MAX_ROOT_PAGE_LINKS);
    let root_links = root_links_page(
        store,
        &root,
        query.include_tests,
        base_root_total,
        client_total,
        offset,
        root_page_capacity,
    )?;
    let next_offset = offset.saturating_add(root_links.len());
    let next_cursor = (next_offset < total_root_links).then(|| encode_cursor(next_offset, query));
    set_child_metadata(
        &mut root,
        total_root_links,
        next_offset.min(total_root_links),
    );

    let mut nodes = HashMap::from([(root.id.clone(), root)]);
    let mut edges = HashMap::new();
    let mut visited = HashSet::from([root_id.clone()]);
    let mut queue = VecDeque::new();
    let mut links_scanned = root_links.len();
    let mut truncated = next_cursor.is_some();
    let root_links_added = add_links(
        root_id,
        1,
        root_links,
        &mut nodes,
        &mut edges,
        &mut visited,
        &mut queue,
        limit,
        &mut truncated,
    );
    set_node_children(
        &mut nodes,
        root_id,
        total_root_links,
        offset
            .saturating_add(root_links_added)
            .min(total_root_links),
    );

    let mut expansions = 0_usize;
    while let Some((node_id, node_depth)) = queue.pop_front() {
        if expansions >= MAX_FLOW_EXPANSIONS || links_scanned >= MAX_FLOW_LINKS_SCANNED {
            truncated = true;
            break;
        }
        if edges.len() >= MAX_FLOW_EDGES {
            truncated = true;
            break;
        }
        expansions += 1;
        let Some(node) = nodes.get(&node_id).cloned() else {
            continue;
        };
        if node.metadata.get("flowCandidateOnly") == Some(&json!(true)) {
            set_node_children(&mut nodes, &node_id, 0, 0);
            continue;
        }
        let page = links_for_node_page(store, &node, query.include_tests)?;
        links_scanned = links_scanned.saturating_add(page.links.len());
        if node_depth >= depth_limit {
            set_node_children(&mut nodes, &node_id, page.total, 0);
            truncated |= page.total > 0;
            continue;
        }
        let remaining = limit.saturating_sub(nodes.len());
        let selected = page
            .links
            .len()
            .min(MAX_DESCENDANT_LINKS)
            .min(remaining)
            .min(MAX_FLOW_EDGES.saturating_sub(edges.len()));
        let shown = add_links(
            &node_id,
            node_depth + 1,
            page.links.into_iter().take(selected).collect(),
            &mut nodes,
            &mut edges,
            &mut visited,
            &mut queue,
            limit,
            &mut truncated,
        );
        set_node_children(&mut nodes, &node_id, page.total, shown);
        truncated |= shown < page.total;
        if nodes.len() >= limit || edges.len() >= MAX_FLOW_EDGES {
            truncated |= !queue.is_empty();
            break;
        }
    }

    if edges.len() >= MAX_FLOW_EDGES
        && let Some(root) = nodes.get_mut(root_id)
    {
        root.metadata
            .insert("flowResponseEdgeLimit".into(), json!(MAX_FLOW_EDGES));
        root.metadata.insert(
            "flowTruncationReason".into(),
            json!("response edge limit reached; choose a narrower root"),
        );
        root.metadata
            .insert("flowOmittedEdgeCountKnown".into(), json!(false));
    }
    let mut nodes = nodes.into_values().collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        flow_stage(left)
            .cmp(flow_stage(right))
            .then_with(|| left.label.to_lowercase().cmp(&right.label.to_lowercase()))
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut edges = edges.into_values().collect::<Vec<_>>();
    edges.sort_by(|left, right| left.id.cmp(&right.id));
    let omitted = total_root_links.saturating_sub(next_offset.min(total_root_links));
    let mut snapshot = GraphSnapshot {
        nodes,
        edges,
        truncated,
        next_cursor,
        total_root_links: Some(total_root_links),
        omitted_root_links: Some(omitted),
    };
    decorate_cycles(&mut snapshot);
    annotate_structural_communities(&mut snapshot);
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)]
fn add_links(
    parent_id: &str,
    depth: usize,
    links: Vec<FlowLink>,
    nodes: &mut HashMap<String, GraphNode>,
    edges: &mut HashMap<String, crate::domain::GraphEdge>,
    visited: &mut HashSet<String>,
    queue: &mut VecDeque<(String, usize)>,
    limit: usize,
    truncated: &mut bool,
) -> usize {
    let mut added = 0_usize;
    for mut link in links {
        if edges.len() >= MAX_FLOW_EDGES {
            *truncated = true;
            break;
        }
        if nodes.len() >= limit && !nodes.contains_key(&link.node.id) {
            *truncated = true;
            break;
        }
        if edges.insert(link.edge.id.clone(), link.edge).is_none() {
            added += 1;
        }
        if visited.insert(link.node.id.clone()) {
            decorate_node(&mut link.node, Some(parent_id), false);
            queue.push_back((link.node.id.clone(), depth));
            nodes.insert(link.node.id.clone(), link.node);
        }
    }
    added
}

fn root_links_page(
    store: &GraphStore,
    root: &GraphNode,
    include_tests: bool,
    base_total: usize,
    client_total: usize,
    offset: usize,
    limit: usize,
) -> AoneResult<Vec<FlowLink>> {
    let mut page = Vec::with_capacity(limit);
    if offset < base_total {
        page.extend(
            links_for_node_page_at(
                store,
                root,
                include_tests,
                offset,
                (limit - page.len()).min(base_total - offset),
            )?
            .links,
        );
    }
    if page.len() < limit {
        let client_offset = offset.saturating_sub(base_total);
        let take = (limit - page.len()).min(client_total.saturating_sub(client_offset));
        page.extend(endpoint_clients_page(
            store,
            root,
            include_tests,
            client_offset,
            take,
        )?);
    }
    Ok(page)
}

fn decorate_cycles(snapshot: &mut GraphSnapshot) {
    let component_by_node = strongly_connected_components(snapshot)
        .into_iter()
        .enumerate()
        .flat_map(|(index, component)| component.into_iter().map(move |node_id| (node_id, index)))
        .collect::<HashMap<_, _>>();
    for edge in &mut snapshot.edges {
        let same_component = component_by_node
            .get(&edge.source)
            .is_some_and(|source| component_by_node.get(&edge.target) == Some(source));
        if edge.source == edge.target || same_component {
            edge.metadata.insert("cyclic".into(), json!(true));
        }
    }
}

fn set_node_children(
    nodes: &mut HashMap<String, GraphNode>,
    node_id: &str,
    total: usize,
    shown: usize,
) {
    if let Some(node) = nodes.get_mut(node_id) {
        set_child_metadata(node, total, shown);
    }
}

fn flow_stage(node: &GraphNode) -> &str {
    node.metadata
        .get("flowStage")
        .and_then(Value::as_str)
        .unwrap_or("other")
}

fn validate_query(query: &GraphQuery) -> AoneResult<()> {
    if query.root_ids.len() != 1 {
        return Err(AoneError::InvalidRequest(
            "executionFlow requires exactly one rootId".into(),
        ));
    }
    let root = &query.root_ids[0];
    if root.is_empty()
        || root.chars().count() > MAX_ROOT_ID_CHARS
        || root.chars().any(char::is_control)
    {
        return Err(AoneError::InvalidRequest(format!(
            "execution-flow rootId must contain 1 to {MAX_ROOT_ID_CHARS} non-control characters"
        )));
    }
    if query.query.is_some() || !query.node_kinds.is_empty() || !query.edge_kinds.is_empty() {
        return Err(AoneError::InvalidRequest(
            "executionFlow does not accept text or kind filters; choose a narrower rootId".into(),
        ));
    }
    Ok(())
}

fn encode_cursor(offset: usize, query: &GraphQuery) -> String {
    format!("{offset}.{}", cursor_hash(query))
}

fn decode_cursor(cursor: Option<&str>, query: &GraphQuery) -> AoneResult<usize> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    if cursor.is_empty()
        || cursor.chars().count() > MAX_CURSOR_CHARS
        || cursor.chars().any(char::is_control)
    {
        return Err(AoneError::InvalidRequest(
            "execution-flow cursor is invalid".into(),
        ));
    }
    let Some((offset, hash)) = cursor.split_once('.') else {
        return Err(AoneError::InvalidRequest(
            "execution-flow cursor is invalid".into(),
        ));
    };
    if hash != cursor_hash(query)
        || offset.len() > 12
        || !offset.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AoneError::InvalidRequest(
            "execution-flow cursor does not match this query".into(),
        ));
    }
    offset
        .parse::<usize>()
        .map_err(|_| AoneError::InvalidRequest("execution-flow cursor offset is invalid".into()))
}

fn cursor_hash(query: &GraphQuery) -> String {
    let value = format!(
        "{}:{}:{}:{}",
        query.root_ids[0],
        query.include_tests,
        query
            .depth
            .unwrap_or(DEFAULT_FLOW_DEPTH)
            .min(MAX_FLOW_DEPTH),
        query
            .limit
            .unwrap_or(DEFAULT_FLOW_NODES)
            .clamp(2, MAX_FLOW_NODES),
    );
    blake3::hash(value.as_bytes()).to_hex()[..12].to_owned()
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

pub(crate) fn resolve_trace_source(
    store: &GraphStore,
    requested_node_id: Option<&str>,
    relative_path: Option<&str>,
    line: Option<usize>,
) -> AoneResult<Option<String>> {
    if let Some(node_id) = requested_node_id {
        let Some(node) = load_node(store, node_id, true)? else {
            return Ok(None);
        };
        let source_matches = matches!(
            (relative_path, line, node.source.as_ref()),
            (Some(path), Some(line), Some(source))
                if source.relative_path == path
                    && line >= source.start_line
                    && line <= source.end_line
        );
        return Ok(source_matches.then_some(node.id));
    }
    let (Some(path), Some(line)) = (relative_path, line) else {
        return Ok(None);
    };
    let mut statement = store.connection.prepare(
        "SELECT id FROM graph_nodes
         WHERE kind <> 'file' AND file_path = ?1 AND aone_path_allowed(file_path) = 1
           AND CAST(json_extract(source_json, '$.startLine') AS INTEGER) <= ?2
           AND CAST(json_extract(source_json, '$.endLine') AS INTEGER) >= ?2
         ORDER BY id LIMIT 2",
    )?;
    let matches = statement
        .query_map(params![path, line as i64], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok((matches.len() == 1).then(|| matches[0].clone()))
}
