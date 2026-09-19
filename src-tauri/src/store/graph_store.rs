use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, functions::FunctionFlags, params, params_from_iter};
use serde_json::Value;

use super::outbox::OutboxRecord;
use super::{
    bulk_persistence::replace_workspace_files,
    limits::{
        DEFAULT_WORKSPACE_STORE_LIMITS, GRAPH_EDGE_MULTIPLIER, MAX_GRAPH_DEPTH, MAX_GRAPH_NODES,
        MAX_GRAPH_ROOTS, MAX_SEED_NODES, WorkspaceStoreLimits,
    },
    persistence::{
        enforce_workspace_limits, insert_indexed_file, rebuild_resolution_edges,
        validate_indexed_file_limits, validate_workspace_limits,
    },
    records::{
        SCHEMA, capability_from_db, count, edge_from_row, escape_like, file_from_row, node_from_row,
    },
};
use crate::{
    domain::{
        CapabilityLevel, GraphEdge, GraphNode, GraphProjection, GraphQuery, GraphSnapshot,
        WorkspaceFile,
    },
    error::{AoneError, AoneResult},
    graph_algorithms::{annotate_structural_communities, strongly_connected_components},
    scanner::IndexedFile,
};

pub(super) const MAX_FILE_LIST_RESULTS: usize = 5_000;
pub(super) const MAX_FILE_SEARCH_RESULTS: usize = 200;
pub(super) const MAX_FILE_SEARCH_QUERY_CHARS: usize = 256;
const MAX_GRAPH_QUERY_CHARS: usize = 256;
const MAX_GRAPH_KIND_FILTERS: usize = 32;
const MAX_GRAPH_KIND_CHARS: usize = 64;

pub struct GraphStore {
    pub(super) connection: Connection,
}

impl GraphStore {
    pub fn open(path: &std::path::Path) -> AoneResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "busy_timeout", 5_000_i64)?;
        connection.execute_batch(SCHEMA)?;
        // The outbox shares the workspace database so events and the facts
        // they describe commit together.
        connection.execute_batch(super::outbox::SCHEMA)?;
        connection.create_scalar_function(
            "aone_path_allowed",
            1,
            FunctionFlags::SQLITE_DETERMINISTIC,
            |context| {
                let path = context.get::<String>(0)?;
                Ok(!crate::scanner::is_hard_denied(std::path::Path::new(&path)))
            },
        )?;
        let store = Self { connection };
        super::search::rebuild_graph_search(&store)?;
        Ok(store)
    }

    pub fn replace_all(&mut self, files: &[IndexedFile]) -> AoneResult<()> {
        validate_workspace_limits(files, DEFAULT_WORKSPACE_STORE_LIMITS)?;
        let transaction = self.connection.transaction()?;
        replace_workspace_files(&transaction, files)?;
        enforce_workspace_limits(&transaction, DEFAULT_WORKSPACE_STORE_LIMITS)?;
        rebuild_resolution_edges(&transaction)?;
        super::bulk_persistence::restore_bulk_indexes_and_search_triggers(&transaction)?;
        super::outbox::record_change(&transaction, "workspace.replaced", files.len())?;
        transaction.commit()?;
        Ok(())
    }

    pub fn replace_file(&mut self, file: &IndexedFile) -> AoneResult<()> {
        self.replace_file_with_limits(file, DEFAULT_WORKSPACE_STORE_LIMITS)
    }

    pub(super) fn replace_file_with_limits(
        &mut self,
        file: &IndexedFile,
        limits: WorkspaceStoreLimits,
    ) -> AoneResult<()> {
        validate_indexed_file_limits(file, limits)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM files WHERE relative_path = ?1",
            params![file.file.relative_path],
        )?;
        insert_indexed_file(&transaction, file)?;
        enforce_workspace_limits(&transaction, limits)?;
        rebuild_resolution_edges(&transaction)?;
        super::outbox::record_change(&transaction, "workspace.file_indexed", 1)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_file(&mut self, relative_path: &str) -> AoneResult<()> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "DELETE FROM files WHERE relative_path = ?1",
            params![relative_path],
        )?;
        rebuild_resolution_edges(&transaction)?;
        super::outbox::record_change(&transaction, "workspace.file_removed", 1)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_files_under(&mut self, relative_directory: &str) -> AoneResult<usize> {
        let escaped = escape_like(relative_directory.trim_end_matches('/'));
        let pattern = format!("{escaped}/%");
        let transaction = self.connection.transaction()?;
        let removed = transaction.execute(
            "DELETE FROM files WHERE relative_path LIKE ?1 ESCAPE '\\'",
            params![pattern],
        )?;
        rebuild_resolution_edges(&transaction)?;
        super::outbox::record_change(&transaction, "workspace.files_removed", removed)?;
        transaction.commit()?;
        Ok(removed)
    }

    /// Durable events from committed mutations, oldest first. They stay pending
    /// until acknowledged, so delivery is at-least-once.
    pub fn pending_events(&self, limit: usize) -> AoneResult<Vec<OutboxRecord>> {
        super::outbox::pending(&self.connection, limit)
    }

    /// Confirms every event up to and including `sequence` was handled.
    pub fn acknowledge_events(&self, sequence: i64) -> AoneResult<usize> {
        super::outbox::mark_delivered(&self.connection, sequence)
    }

    #[cfg(test)]
    pub(super) fn connection_for_tests(&self) -> &rusqlite::Connection {
        &self.connection
    }

    pub fn counts(&self) -> AoneResult<(usize, usize, usize)> {
        let file_count = count(&self.connection, "files")?;
        let node_count = count(&self.connection, "graph_nodes")?;
        let edge_count = count(&self.connection, "graph_edges")?;
        Ok((file_count, node_count, edge_count))
    }

    pub fn list_files(&self, query: Option<&str>, limit: usize) -> AoneResult<Vec<WorkspaceFile>> {
        let query = validated_file_query(query)?;
        let limit = limit.clamp(
            1,
            if query.is_some() {
                MAX_FILE_SEARCH_RESULTS
            } else {
                MAX_FILE_LIST_RESULTS
            },
        );
        let sql = if query.is_some() {
            "SELECT id, relative_path, language, capability, size_bytes, content_hash, modified_at, parse_errors
             FROM files WHERE relative_path LIKE ?2 ESCAPE '\\' COLLATE NOCASE
             ORDER BY CASE WHEN relative_path = ?1 COLLATE NOCASE THEN 0 ELSE 1 END,
                      relative_path COLLATE NOCASE
             LIMIT ?3"
        } else {
            "SELECT id, relative_path, language, capability, size_bytes, content_hash, modified_at, parse_errors
             FROM files ORDER BY relative_path LIMIT ?1"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = if let Some(query) = query {
            let pattern = format!("%{}%", escape_like(query));
            statement.query_map(params![query, pattern, limit as i64], file_from_row)?
        } else {
            statement.query_map(params![limit as i64], file_from_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_file(&self, relative_path: &str) -> AoneResult<Option<WorkspaceFile>> {
        self.connection
            .query_row(
                "SELECT id, relative_path, language, capability, size_bytes, content_hash, modified_at, parse_errors
                 FROM files WHERE relative_path = ?1",
                params![relative_path],
                file_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn language_counts(&self) -> AoneResult<Vec<(String, CapabilityLevel, usize)>> {
        let mut statement = self.connection.prepare(
            "SELECT language, capability, COUNT(*) FROM files
             GROUP BY language, capability ORDER BY COUNT(*) DESC, language",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                capability_from_db(&row.get::<_, String>(1)?),
                row.get::<_, i64>(2)? as usize,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn query_graph(&self, query: &GraphQuery) -> AoneResult<GraphSnapshot> {
        validate_graph_query(query)?;
        if query.projection == GraphProjection::SystemOverview {
            return super::overview::query_system_overview(self, query);
        }
        if query.projection == GraphProjection::ExecutionFlow {
            return super::execution_flow::query_execution_flow(self, query);
        }
        let limit = query.limit.unwrap_or(250).clamp(1, MAX_GRAPH_NODES);
        let depth = query.depth.unwrap_or(1).min(MAX_GRAPH_DEPTH);
        let root_limit = MAX_GRAPH_ROOTS.min(limit);
        let roots = query
            .root_ids
            .iter()
            .take(root_limit)
            .cloned()
            .collect::<Vec<_>>();

        let (seed_nodes, seed_truncated) = if roots.is_empty() {
            self.search_nodes(
                query.query.as_deref(),
                &query.node_kinds,
                limit.min(MAX_SEED_NODES),
            )?
        } else {
            (self.load_nodes(&roots)?, query.root_ids.len() > root_limit)
        };

        let mut nodes = seed_nodes
            .into_iter()
            .map(|node| (node.id.clone(), node))
            .collect::<HashMap<_, _>>();
        let mut frontier = nodes.keys().cloned().collect::<Vec<_>>();
        let mut edge_map = HashMap::new();
        let mut truncated = seed_truncated;

        for _ in 0..depth {
            if frontier.is_empty() || nodes.len() >= limit {
                truncated |= nodes.len() >= limit;
                break;
            }
            let (edges, edge_query_truncated) = self.edges_touching(
                &frontier,
                &query.edge_kinds,
                limit.saturating_mul(GRAPH_EDGE_MULTIPLIER),
            )?;
            truncated |= edge_query_truncated;
            let mut candidates = Vec::new();
            for edge in edges {
                let neighbor = if nodes.contains_key(&edge.source) {
                    &edge.target
                } else {
                    &edge.source
                };
                if !nodes.contains_key(neighbor) {
                    candidates.push(neighbor.clone());
                }
                edge_map.insert(edge.id.clone(), edge);
            }
            candidates.sort();
            candidates.dedup();
            let remaining = limit.saturating_sub(nodes.len());
            if candidates.len() > remaining {
                candidates.truncate(remaining);
                truncated = true;
            }
            let next_nodes = self.load_nodes(&candidates)?;
            frontier.clear();
            for node in next_nodes {
                frontier.push(node.id.clone());
                nodes.insert(node.id.clone(), node);
            }
        }

        let selected = nodes.keys().cloned().collect::<HashSet<_>>();
        let mut edges = edge_map
            .into_values()
            .filter(|edge| selected.contains(&edge.source) && selected.contains(&edge.target))
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| left.id.cmp(&right.id));
        let edge_limit = limit.saturating_mul(GRAPH_EDGE_MULTIPLIER);
        if edges.len() > edge_limit {
            edges.truncate(edge_limit);
            truncated = true;
        }
        let mut nodes = nodes.into_values().collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));

        let mut snapshot = GraphSnapshot {
            nodes,
            edges,
            truncated,
            next_cursor: None,
            total_root_links: None,
            omitted_root_links: None,
        };
        for (component_index, component) in strongly_connected_components(&snapshot)
            .into_iter()
            .enumerate()
        {
            for node_id in component {
                if let Some(node) = snapshot.nodes.iter_mut().find(|node| node.id == node_id) {
                    node.metadata.insert(
                        "stronglyConnectedComponent".into(),
                        Value::from(component_index as u64),
                    );
                }
            }
        }
        annotate_structural_communities(&mut snapshot);
        Ok(snapshot)
    }

    pub fn list_api_endpoints(
        &self,
        request: &crate::domain::ListApiEndpointsRequest,
    ) -> AoneResult<crate::domain::ApiEndpointPage> {
        super::api_inventory::list_api_endpoints(self, request)
    }

    pub(crate) fn resolve_trace_source(
        &self,
        requested_node_id: Option<&str>,
        relative_path: &str,
        line: usize,
    ) -> AoneResult<Option<String>> {
        super::execution_flow::resolve_trace_source(
            self,
            requested_node_id,
            Some(relative_path),
            Some(line),
        )
    }

    fn search_nodes(
        &self,
        query: Option<&str>,
        kinds: &[String],
        limit: usize,
    ) -> AoneResult<(Vec<GraphNode>, bool)> {
        let pattern = query
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("%{}%", escape_like(value.trim())));
        let mut clauses = Vec::new();
        let mut values = Vec::new();
        if let Some(pattern) = pattern {
            clauses.push("(label LIKE ? ESCAPE '\\' COLLATE NOCASE OR file_path LIKE ? ESCAPE '\\' COLLATE NOCASE)".to_string());
            values.push(pattern.clone());
            values.push(pattern);
        }
        if !kinds.is_empty() {
            clauses.push(format!(
                "kind IN ({})",
                std::iter::repeat_n("?", kinds.len())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            values.extend(kinds.iter().cloned());
        }
        let where_clause = if clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", clauses.join(" AND "))
        };
        let sql = format!(
            "SELECT id, kind, label, source_json, language, evidence, metadata_json
             FROM graph_nodes{where_clause} ORDER BY CASE kind WHEN 'file' THEN 0 ELSE 1 END, label LIMIT ?"
        );
        // Fetch one extra row so callers can distinguish an exact fit from a
        // bounded projection that omitted additional matching seeds.
        values.push(limit.saturating_add(1).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), node_from_row)?;
        let mut nodes = rows.collect::<Result<Vec<_>, _>>()?;
        let truncated = nodes.len() > limit;
        nodes.truncate(limit);
        Ok((nodes, truncated))
    }

    pub(super) fn load_nodes(&self, ids: &[String]) -> AoneResult<Vec<GraphNode>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, kind, label, source_json, language, evidence, metadata_json
             FROM graph_nodes WHERE id IN ({placeholders})"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(ids.iter()), node_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn edges_touching(
        &self,
        ids: &[String],
        kinds: &[String],
        limit: usize,
    ) -> AoneResult<(Vec<GraphEdge>, bool)> {
        if ids.is_empty() {
            return Ok((Vec::new(), false));
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut sql = format!(
            "SELECT id, source, target, kind, evidence, confidence, metadata_json
             FROM graph_edges WHERE (source IN ({placeholders}) OR target IN ({placeholders}))"
        );
        let mut values = ids.iter().chain(ids.iter()).cloned().collect::<Vec<_>>();
        if !kinds.is_empty() {
            sql.push_str(&format!(
                " AND kind IN ({})",
                std::iter::repeat_n("?", kinds.len())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            values.extend(kinds.iter().cloned());
        }
        sql.push_str(" ORDER BY id LIMIT ?");
        values.push(limit.saturating_add(1).to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), edge_from_row)?;
        let mut edges = rows.collect::<Result<Vec<_>, _>>()?;
        let truncated = edges.len() > limit;
        edges.truncate(limit);
        Ok((edges, truncated))
    }
}

fn validated_file_query(query: Option<&str>) -> AoneResult<Option<&str>> {
    let Some(query) = query else {
        return Ok(None);
    };
    if query.chars().count() > MAX_FILE_SEARCH_QUERY_CHARS {
        return Err(AoneError::InvalidRequest(format!(
            "workspace file search query exceeds {MAX_FILE_SEARCH_QUERY_CHARS} characters"
        )));
    }
    if query.chars().any(char::is_control) {
        return Err(AoneError::InvalidRequest(
            "workspace file search query cannot contain control characters".into(),
        ));
    }
    let query = query.trim();
    Ok((!query.is_empty()).then_some(query))
}

fn validate_graph_query(query: &GraphQuery) -> AoneResult<()> {
    if query.projection != GraphProjection::ExecutionFlow
        && (query.cursor.is_some() || query.include_tests)
    {
        return Err(AoneError::InvalidRequest(
            "cursor and includeTests are supported only by executionFlow".into(),
        ));
    }
    if let Some(text) = query.query.as_deref() {
        if text.chars().count() > MAX_GRAPH_QUERY_CHARS {
            return Err(AoneError::InvalidRequest(format!(
                "graph query exceeds {MAX_GRAPH_QUERY_CHARS} characters"
            )));
        }
        if text.chars().any(char::is_control) {
            return Err(AoneError::InvalidRequest(
                "graph query cannot contain control characters".into(),
            ));
        }
    }
    for (name, filters) in [
        ("nodeKinds", query.node_kinds.as_slice()),
        ("edgeKinds", query.edge_kinds.as_slice()),
    ] {
        if filters.len() > MAX_GRAPH_KIND_FILTERS {
            return Err(AoneError::InvalidRequest(format!(
                "{name} exceeds {MAX_GRAPH_KIND_FILTERS} entries"
            )));
        }
        if filters.iter().any(|value| {
            value.chars().count() > MAX_GRAPH_KIND_CHARS || value.chars().any(char::is_control)
        }) {
            return Err(AoneError::InvalidRequest(format!(
                "{name} entries must be at most {MAX_GRAPH_KIND_CHARS} non-control characters"
            )));
        }
    }
    Ok(())
}
