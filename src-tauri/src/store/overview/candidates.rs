use std::collections::{BTreeMap, HashSet};

use rusqlite::params_from_iter;

use super::classify::{SystemLayer, eligible_node, placement};
use crate::{
    domain::{GraphNode, GraphQuery},
    error::AoneResult,
    store::{
        GraphStore,
        limits::MAX_GRAPH_ROOTS,
        records::{escape_like, node_from_row},
    },
};

const CANDIDATE_SCAN_LIMIT: usize = 256;
const CONNECTIVITY_BATCH_SIZE: usize = 800;
const SEED_QUOTAS: [(SystemLayer, usize); 6] = [
    (SystemLayer::Configuration, 4),
    (SystemLayer::Entry, 4),
    (SystemLayer::Interface, 7),
    (SystemLayer::Application, 7),
    (SystemLayer::Data, 5),
    (SystemLayer::External, 3),
];

struct LayerCandidates {
    nodes: Vec<GraphNode>,
    truncated: bool,
}

pub(super) fn select_diverse_seeds(
    store: &GraphStore,
    query: &GraphQuery,
    node_limit: usize,
) -> AoneResult<(Vec<GraphNode>, bool)> {
    let mut by_layer = BTreeMap::<SystemLayer, LayerCandidates>::new();
    for (layer, _) in SEED_QUOTAS {
        by_layer.insert(layer, load_layer_candidates(store, query, layer)?);
    }
    let connected = connected_candidate_ids(
        store,
        by_layer
            .values()
            .flat_map(|candidates| candidates.nodes.iter().map(|node| node.id.clone())),
    )?;
    let mut truncated = by_layer.values().any(|candidates| candidates.truncated);
    let mut selected_by_layer = BTreeMap::<SystemLayer, Vec<GraphNode>>::new();
    for (layer, quota) in SEED_QUOTAS {
        let mut candidates = by_layer
            .remove(&layer)
            .map_or_else(Vec::new, |value| value.nodes);
        candidates.retain(|node| eligible_node(node) && placement(node).layer == layer);
        candidates = connected_first(candidates, &connected);
        if layer == SystemLayer::Interface {
            candidates = diversify_interface(candidates);
        }
        if layer == SystemLayer::Entry {
            candidates = one_entry_per_project(candidates);
        }
        if candidates.len() > quota {
            candidates.truncate(quota);
            truncated = true;
        }
        selected_by_layer.insert(layer, candidates);
    }

    let seed_limit = node_limit.min(MAX_GRAPH_ROOTS);
    let mut selected = Vec::new();
    let mut positions = BTreeMap::<SystemLayer, usize>::new();
    while selected.len() < seed_limit {
        let mut advanced = false;
        for layer in SystemLayer::ALL {
            let position = positions.entry(layer).or_default();
            let Some(node) = selected_by_layer
                .get(&layer)
                .and_then(|nodes| nodes.get(*position))
            else {
                continue;
            };
            selected.push(node.clone());
            *position += 1;
            advanced = true;
            if selected.len() == seed_limit {
                break;
            }
        }
        if !advanced {
            break;
        }
    }
    let available = selected_by_layer.values().map(Vec::len).sum::<usize>();
    truncated |= available > selected.len();
    Ok((selected, truncated))
}

fn load_layer_candidates(
    store: &GraphStore,
    query: &GraphQuery,
    layer: SystemLayer,
) -> AoneResult<LayerCandidates> {
    let mut clauses = vec![format!("({})", layer_predicate(layer))];
    clauses.extend([
        "kind NOT IN ('callTarget','module')".into(),
        format!("{} = 0", test_path_penalty("graph_nodes.file_path")),
        auxiliary_path_clause(),
        "lower(file_path) <> '.env' AND lower(file_path) NOT LIKE '%/.env' AND lower(file_path) NOT LIKE '%/.env.%' AND lower(file_path) NOT LIKE '%credential%' AND lower(file_path) NOT LIKE '%secret%'".into(),
    ]);
    if !matches!(layer, SystemLayer::Configuration | SystemLayer::Entry) {
        clauses.push("kind <> 'file'".into());
    }
    if layer == SystemLayer::Entry {
        clauses.push("('/' || lower(file_path) || '/') NOT LIKE '%/migration/%' AND ('/' || lower(file_path) || '/') NOT LIKE '%/migrations/%' AND ('/' || lower(file_path) || '/') NOT LIKE '%/migrate/%'".into());
    }
    let mut values = Vec::new();
    add_user_filters(&mut clauses, &mut values, query);
    let sql = format!(
        "SELECT id, kind, label, source_json, language, evidence, metadata_json
         FROM graph_nodes WHERE {}
         ORDER BY {}, COALESCE(CAST(json_extract(source_json, '$.startLine') AS INTEGER), 2147483647), lower(label), id LIMIT ?",
        clauses.join(" AND "),
        layer_order(layer)
    );
    values.push(CANDIDATE_SCAN_LIMIT.saturating_add(1).to_string());
    let mut statement = store.connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), node_from_row)?;
    let mut nodes = rows.collect::<Result<Vec<_>, _>>()?;
    let truncated = nodes.len() > CANDIDATE_SCAN_LIMIT;
    nodes.truncate(CANDIDATE_SCAN_LIMIT);
    Ok(LayerCandidates { nodes, truncated })
}

fn add_user_filters(clauses: &mut Vec<String>, values: &mut Vec<String>, query: &GraphQuery) {
    if let Some(text) = query
        .query
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let pattern = format!("%{}%", escape_like(text.trim()));
        clauses.push("(label LIKE ? ESCAPE '\\' COLLATE NOCASE OR file_path LIKE ? ESCAPE '\\' COLLATE NOCASE)".into());
        values.extend([pattern.clone(), pattern]);
    }
    if !query.node_kinds.is_empty() {
        clauses.push(format!(
            "kind IN ({})",
            std::iter::repeat_n("?", query.node_kinds.len())
                .collect::<Vec<_>>()
                .join(",")
        ));
        values.extend(query.node_kinds.iter().cloned());
    }
}

fn connected_candidate_ids(
    store: &GraphStore,
    ids: impl IntoIterator<Item = String>,
) -> AoneResult<HashSet<String>> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    let mut connected = HashSet::new();
    for batch in ids.chunks(CONNECTIVITY_BATCH_SIZE) {
        let placeholders = std::iter::repeat_n("?", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id FROM graph_nodes WHERE id IN ({placeholders}) AND
             (EXISTS(SELECT 1 FROM graph_edges e WHERE e.source = graph_nodes.id) OR
              EXISTS(SELECT 1 FROM graph_edges e WHERE e.target = graph_nodes.id))"
        );
        let mut statement = store.connection.prepare(&sql)?;
        connected.extend(
            statement
                .query_map(params_from_iter(batch.iter()), |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    Ok(connected)
}

fn connected_first(nodes: Vec<GraphNode>, connected: &HashSet<String>) -> Vec<GraphNode> {
    let (mut connected_nodes, disconnected): (Vec<_>, Vec<_>) = nodes
        .into_iter()
        .partition(|node| connected.contains(&node.id));
    connected_nodes.extend(disconnected);
    connected_nodes
}

fn diversify_interface(nodes: Vec<GraphNode>) -> Vec<GraphNode> {
    let categories = [(3, "endpoint"), (2, "execution"), (1, "api"), (1, "event")];
    let mut selected = Vec::new();
    let mut used = HashSet::new();
    for (limit, category) in categories {
        for node in nodes
            .iter()
            .filter(|node| interface_category(node) == category)
            .take(limit)
        {
            used.insert(node.id.clone());
            selected.push(node.clone());
        }
    }
    selected.extend(nodes.into_iter().filter(|node| !used.contains(&node.id)));
    selected
}

fn interface_category(node: &GraphNode) -> &'static str {
    match node.kind.as_str() {
        "endpoint" | "controller" => "endpoint",
        "api" | "external-api" | "externalapi" => "api",
        "event" => "event",
        _ => "execution",
    }
}

fn layer_predicate(layer: SystemLayer) -> String {
    match layer {
        SystemLayer::Configuration => "kind = 'file' AND (lower(file_path) IN ('package.json','cargo.toml','pyproject.toml','go.mod','pom.xml','build.gradle','build.gradle.kts','settings.gradle','settings.gradle.kts','tsconfig.json','jsconfig.json','tauri.conf.json','docker-compose.yml','docker-compose.yaml','compose.yml','compose.yaml','cmakelists.txt','makefile','.editorconfig','config.toml','config.json','config.yml','config.yaml','application.yml','application.yaml') OR lower(file_path) LIKE '%/package.json' OR lower(file_path) LIKE '%/cargo.toml' OR lower(file_path) LIKE '%/pyproject.toml' OR lower(file_path) LIKE '%/go.mod' OR lower(file_path) LIKE '%/tsconfig.json' OR lower(file_path) LIKE '%/tauri.conf.json' OR lower(file_path) LIKE '%/vite.config.%' OR lower(file_path) LIKE '%/next.config.%')".into(),
        SystemLayer::Entry => "(kind IN ('function','method') AND lower(trim(label)) = 'main') OR (kind = 'file' AND (lower(file_path) LIKE '%/main.rs' OR lower(file_path) LIKE '%/main.go' OR lower(file_path) LIKE '%/main.py' OR lower(file_path) LIKE '%/main.kt' OR lower(file_path) LIKE '%/main.ts' OR lower(file_path) LIKE '%/main.tsx' OR lower(file_path) LIKE '%/main.js' OR lower(file_path) LIKE '%/main.jsx' OR lower(file_path) LIKE '%/server.ts' OR lower(file_path) LIKE '%/server.js' OR lower(file_path) LIKE '%/app.ts' OR lower(file_path) LIKE '%/app.js' OR lower(file_path) IN ('main.rs','main.go','main.py','main.kt','main.ts','main.tsx','main.js','main.jsx','server.ts','server.js','app.ts','app.js','index.ts','index.tsx','index.js','index.jsx','src/index.ts','src/index.tsx','src/index.js','src/index.jsx')))".into(),
        SystemLayer::Interface => format!(
            "kind IN ('endpoint','controller') OR ({}) OR ({}) OR ({})",
            relative_or_local_api_clause(),
            execution_path_clause(),
            meaningful_event_clause()
        ),
        SystemLayer::Application => format!(
            "kind IN ('service','function','method','class','interface','trait','struct','enum','implementation','object','type') AND NOT ({}) AND NOT ({})",
            execution_path_clause(),
            data_path_clause()
        ),
        SystemLayer::Data => format!(
            "kind IN ('repository','model','database','table','schema','entity') OR ({})",
            data_path_clause()
        ),
        SystemLayer::External => format!("({})", absolute_non_local_api_clause()),
    }
}

fn execution_path_clause() -> String {
    path_segments_clause(&[
        "job",
        "jobs",
        "worker",
        "workers",
        "cron",
        "batch",
        "background",
        "queue",
        "queues",
        "command",
        "commands",
        "task",
        "tasks",
    ])
}

fn strong_data_path_clause() -> String {
    let segments = path_segments_clause(&[
        "data",
        "database",
        "databases",
        "repository",
        "repositories",
        "persistence",
        "model",
        "models",
        "schema",
        "schemas",
        "migration",
        "migrations",
    ]);
    format!(
        "{segments} OR lower(file_path) LIKE '%/repository.%' OR lower(file_path) LIKE 'repository.%' OR lower(file_path) LIKE '%/model.%' OR lower(file_path) LIKE 'model.%' OR lower(file_path) LIKE '%_repository.%' OR lower(file_path) LIKE '%-repository.%' OR lower(file_path) LIKE '%_model.%' OR lower(file_path) LIKE '%-model.%'"
    )
}

fn data_path_clause() -> String {
    format!(
        "({}) OR ((('/' || lower(file_path) || '/') LIKE '%/storage/%') AND kind IN ('class','service','repository','model','interface','trait','struct','enum','implementation','object','type'))",
        strong_data_path_clause()
    )
}

fn path_segments_clause(segments: &[&str]) -> String {
    segments
        .iter()
        .map(|segment| format!("('/' || lower(file_path) || '/') LIKE '%/{segment}/%'"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn meaningful_event_clause() -> String {
    "kind = 'event' AND COALESCE(json_extract(metadata_json, '$.eventApi'), '') <> 'DOM' AND label NOT LIKE '.%' AND lower(label) NOT IN ('abort','change','click','drag','end','mousemove','start','tick','zoom') AND lower(file_path) NOT LIKE '%canvas%' AND lower(file_path) NOT LIKE '%pointcloud%'".into()
}

fn relative_or_local_api_clause() -> String {
    format!(
        "kind IN ('api','external-api','externalapi') AND NOT ({})",
        absolute_non_local_target_clause()
    )
}

fn absolute_non_local_api_clause() -> String {
    format!(
        "kind IN ('api','external-api','externalapi') AND ({})",
        absolute_non_local_target_clause()
    )
}

fn absolute_non_local_target_clause() -> &'static str {
    "(lower(label) LIKE '%http://%' OR lower(label) LIKE '%https://%') AND lower(label) NOT LIKE '%http://localhost%' AND lower(label) NOT LIKE '%https://localhost%' AND lower(label) NOT LIKE '%http://127.%' AND lower(label) NOT LIKE '%https://127.%' AND lower(label) NOT LIKE '%http://0.0.0.0%' AND lower(label) NOT LIKE '%https://0.0.0.0%' AND lower(label) NOT LIKE '%http://[::1]%' AND lower(label) NOT LIKE '%https://[::1]%'"
}

fn layer_order(layer: SystemLayer) -> String {
    match layer {
        SystemLayer::Entry => "CASE WHEN kind = 'file' AND ('/' || lower(file_path)) LIKE '%/src/main.%' THEN 0 WHEN kind = 'file' THEN 1 WHEN kind IN ('function','method') AND ('/' || lower(file_path)) LIKE '%/src/main.%' THEN 2 ELSE 3 END".into(),
        SystemLayer::Interface => format!(
            "CASE WHEN kind IN ('endpoint','controller') THEN 0 WHEN ({}) THEN 1 WHEN kind IN ('api','external-api','externalapi') THEN 2 WHEN kind = 'event' THEN 3 ELSE 4 END",
            execution_path_clause()
        ),
        SystemLayer::Data => format!(
            "CASE WHEN kind IN ('repository','model','database','table','schema','entity') THEN 0 WHEN ({}) THEN 1 ELSE 2 END",
            strong_data_path_clause()
        ),
        _ => "CASE WHEN 1 = 1 THEN 0 ELSE 0 END".into(),
    }
}

fn one_entry_per_project(nodes: Vec<GraphNode>) -> Vec<GraphNode> {
    let mut roots = HashSet::new();
    nodes
        .into_iter()
        .filter(|node| roots.insert(project_root(node)))
        .collect()
}

fn project_root(node: &GraphNode) -> String {
    let path = node
        .source
        .as_ref()
        .map(|source| source.relative_path.as_str())
        .unwrap_or(&node.label)
        .replace('\\', "/");
    path.split_once("/src/")
        .map(|(root, _)| root)
        .unwrap_or_else(|| path.split('/').next().unwrap_or("."))
        .to_owned()
}

fn auxiliary_path_clause() -> String {
    path_segments_clause(&[
        "evaluation",
        "evaluations",
        "eval",
        "example",
        "examples",
        "benchmark",
        "benchmarks",
    ])
    .split(" OR ")
    .map(|clause| format!("NOT ({clause})"))
    .collect::<Vec<_>>()
    .join(" AND ")
}

fn test_path_penalty(column: &str) -> String {
    format!(
        "CASE WHEN lower({column}) LIKE '%/tests/%' OR lower({column}) LIKE 'tests/%' OR lower({column}) LIKE '%/test/%' OR lower({column}) LIKE 'test/%' OR lower({column}) LIKE '%/e2e/%' OR lower({column}) LIKE 'e2e/%' OR lower({column}) LIKE '%/__tests__/%' OR lower({column}) LIKE '__tests__/%' OR lower({column}) LIKE '%.test.%' OR lower({column}) LIKE '%.spec.%' OR lower({column}) LIKE '%_test.%' OR lower({column}) LIKE '%_tests.%' OR lower({column}) LIKE '%-test.%' OR lower({column}) LIKE '%-tests.%' THEN 1 ELSE 0 END"
    )
}
