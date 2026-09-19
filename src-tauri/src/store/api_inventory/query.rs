use std::path::Path;

use rusqlite::{OptionalExtension, Row, named_params};
use serde::Deserialize;

use super::super::{
    GraphStore,
    records::{escape_like, evidence_from_db},
};
use crate::{
    domain::{
        ApiClientCoverage, ApiEndpoint, ApiEndpointGrouping, ApiEndpointHandler, ApiEndpointPage,
        ApiEndpointRole, ApiGroupingBasis, ApiInventoryCounts, ApiProtocol,
        ListApiEndpointsRequest, SourceLocation,
    },
    error::{AoneError, AoneResult},
};

const DEFAULT_PAGE_SIZE: usize = 100;
pub(super) const MAX_PAGE_SIZE: usize = 200;
const MAX_QUERY_CHARS: usize = 128;
const MAX_CURSOR_CHARS: usize = 200;

const CATALOG_CTE: &str = r#"
WITH facts AS (
  SELECT n.id, n.label, n.file_path, n.source_json, n.language, n.evidence,
         n.metadata_json,
         COALESCE(json_extract(n.metadata_json, '$.apiRole'), 'unknown') AS role,
         COALESCE(json_extract(n.metadata_json, '$.protocol'), 'http') AS protocol,
         COALESCE(upper(json_extract(n.metadata_json, '$.httpMethod')),
                  upper(substr(n.label, 1, instr(n.label, ' ') - 1))) AS method,
         COALESCE(json_extract(n.metadata_json, '$.routePath'),
                  trim(substr(n.label, instr(n.label, ' ') + 1))) AS route_path,
         json_extract(n.metadata_json, '$.apiFramework') AS framework,
         json_extract(n.metadata_json, '$.operationId') AS operation_id,
         json_extract(n.metadata_json, '$.specTitle') AS spec_title,
         json_extract(n.metadata_json, '$.apiGroup') AS api_group,
         COALESCE(json_extract(n.metadata_json, '$.apiServiceRoot'),
                  CASE WHEN instr(n.file_path, '/src/') > 0
                       THEN substr(n.file_path, 1, instr(n.file_path, '/src/') - 1)
                       WHEN n.file_path LIKE 'src/%' THEN 'Workspace root'
                       WHEN instr(n.file_path, '/') > 0
                       THEN substr(n.file_path, 1, instr(n.file_path, '/') - 1)
                       ELSE 'Workspace root' END) AS source_service_key
  FROM graph_nodes AS n
  WHERE n.kind IN ('endpoint', 'api')
    AND lower(n.file_path) NOT LIKE 'test/%'
    AND lower(n.file_path) NOT LIKE 'tests/%'
    AND lower(n.file_path) NOT LIKE 'e2e/%'
    AND lower(n.file_path) NOT LIKE '%/test/%'
    AND lower(n.file_path) NOT LIKE '%/tests/%'
    AND lower(n.file_path) NOT LIKE '%/e2e/%'
    AND lower(n.file_path) NOT LIKE '%/__tests__/%'
    AND lower(n.file_path) NOT LIKE '%/fixtures/%'
    AND lower(n.file_path) NOT LIKE '%.test.%'
    AND lower(n.file_path) NOT LIKE '%.spec.%'
    AND lower(n.file_path) NOT GLOB '*_test.*'
), eligible_base AS (
  SELECT * FROM facts
  WHERE protocol = 'http'
    AND method IN ('CONNECT','DELETE','GET','HEAD','OPTIONS','PATCH','POST','PUT','TRACE')
    AND length(route_path) <= 1024
    AND (
      (route_path LIKE '/%' AND route_path NOT LIKE '//%')
      OR (
        role = 'consumer'
        AND json_extract(metadata_json, '$.targetSanitized') = 1
        AND (route_path LIKE 'http://%' OR route_path LIKE 'https://%')
      )
    )
    AND instr(route_path, '?') = 0
    AND instr(route_path, '#') = 0
), producer_matches AS (
  SELECT protocol, method, route_path, COUNT(DISTINCT source_service_key) AS service_count,
         MIN(source_service_key) AS service_key
  FROM eligible_base
  WHERE role = 'producer'
  GROUP BY protocol, method, route_path
), eligible AS (
  SELECT base.*,
         CASE
           WHEN base.role = 'producer' THEN base.source_service_key
           WHEN matched.service_count = 1 THEN matched.service_key
           ELSE base.source_service_key
         END AS service_key
  FROM eligible_base AS base
  LEFT JOIN producer_matches AS matched
    ON matched.protocol = base.protocol AND matched.method = base.method
   AND matched.route_path = base.route_path
), ranked AS (
  SELECT eligible.*,
         row_number() OVER operation_order AS operation_rank,
         sum(CASE WHEN role = 'producer' THEN 1 ELSE 0 END) OVER operation_group AS producer_count,
         sum(CASE WHEN role = 'consumer' THEN 1 ELSE 0 END) OVER operation_group AS consumer_count,
         sum(CASE WHEN role = 'unknown' THEN 1 ELSE 0 END) OVER operation_group AS unknown_count,
         count(*) OVER operation_group AS occurrence_count
  FROM eligible
  WINDOW operation_group AS (PARTITION BY service_key, protocol, method, route_path),
         operation_order AS (
           PARTITION BY service_key, protocol, method, route_path
           ORDER BY CASE framework WHEN 'OpenAPI' THEN 0 WHEN 'Utoipa' THEN 1 ELSE 2 END,
                    CASE role WHEN 'producer' THEN 0 ELSE 1 END,
                    file_path, id
         )
), canonical AS (
  SELECT * FROM ranked WHERE operation_rank = 1
), filtered AS (
  SELECT * FROM canonical
  WHERE :pattern IS NULL
     OR label LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR method LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR route_path LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR operation_id LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR file_path LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR language LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR framework LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR spec_title LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR api_group LIKE :pattern ESCAPE '\' COLLATE NOCASE
     OR EXISTS (
       SELECT 1 FROM eligible AS handler
       JOIN graph_edges AS relation ON relation.target = handler.id
         AND relation.kind = 'handles'
       JOIN graph_nodes AS owner ON owner.id = relation.source
       WHERE handler.service_key = canonical.service_key
         AND handler.protocol = canonical.protocol AND handler.method = canonical.method
         AND handler.route_path = canonical.route_path AND handler.role = 'producer'
         AND (owner.label LIKE :pattern ESCAPE '\' COLLATE NOCASE
              OR owner.kind LIKE :pattern ESCAPE '\' COLLATE NOCASE)
     )
)
"#;

pub(in crate::store) fn list_api_endpoints(
    store: &GraphStore,
    request: &ListApiEndpointsRequest,
) -> AoneResult<ApiEndpointPage> {
    let query = validated_query(request.query.as_deref())?;
    let pattern = query.map(|value| format!("%{}%", escape_like(value)));
    let cursor_id = decode_cursor(request.cursor.as_deref(), query)?;
    let limit = request
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    if let Some(cursor_id) = cursor_id.as_deref() {
        ensure_cursor_exists(store, cursor_id, pattern.as_deref())?;
    }
    let (indexed_total, total, counts) = catalog_counts(store, pattern.as_deref())?;
    let mut endpoints = catalog_page(
        store,
        cursor_id.as_deref(),
        pattern.as_deref(),
        limit.saturating_add(1),
    )?;
    let truncated = endpoints.len() > limit;
    endpoints.truncate(limit);
    let next_cursor = truncated
        .then(|| {
            endpoints
                .last()
                .map(|endpoint| encode_cursor(&endpoint.id, query))
        })
        .flatten();
    let returned = endpoints.len();
    Ok(ApiEndpointPage {
        workspace_id: request.workspace_id.clone(),
        endpoints,
        total,
        indexed_total,
        returned,
        next_cursor,
        truncated,
        counts,
    })
}

fn catalog_counts(
    store: &GraphStore,
    pattern: Option<&str>,
) -> AoneResult<(usize, usize, ApiInventoryCounts)> {
    let sql = format!(
        "{CATALOG_CTE}
         SELECT (SELECT COUNT(*) FROM canonical), COUNT(*),
                COALESCE(SUM(producer_count > 0), 0),
                COALESCE(SUM(producer_count = 0 AND consumer_count > 0), 0),
                COALESCE(SUM(producer_count > 0 AND consumer_count > 0), 0),
                COALESCE(SUM(producer_count = 0 AND consumer_count = 0), 0)
         FROM filtered"
    );
    let values =
        store
            .connection
            .query_row(&sql, named_params! { ":pattern": pattern }, |row| {
                Ok((
                    usize_value(row, 0)?,
                    usize_value(row, 1)?,
                    ApiInventoryCounts {
                        producer: usize_value(row, 2)?,
                        consumer_only: usize_value(row, 3)?,
                        client_covered: usize_value(row, 4)?,
                        unknown: usize_value(row, 5)?,
                    },
                ))
            })?;
    Ok(values)
}

fn ensure_cursor_exists(
    store: &GraphStore,
    cursor_id: &str,
    pattern: Option<&str>,
) -> AoneResult<()> {
    let sql = format!("{CATALOG_CTE} SELECT 1 FROM filtered WHERE id = :cursor LIMIT 1");
    let exists = store
        .connection
        .query_row(
            &sql,
            named_params! { ":pattern": pattern, ":cursor": cursor_id },
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        return Err(AoneError::InvalidRequest(
            "API inventory cursor is stale or does not match this query".into(),
        ));
    }
    Ok(())
}

fn catalog_page(
    store: &GraphStore,
    cursor_id: Option<&str>,
    pattern: Option<&str>,
    limit: usize,
) -> AoneResult<Vec<ApiEndpoint>> {
    let sql = format!(
        "{CATALOG_CTE}
         SELECT c.id, c.label, c.method, c.route_path, c.file_path, c.source_json,
                c.language, c.evidence, c.framework, c.operation_id, c.spec_title, c.api_group,
                c.producer_count, c.consumer_count, c.unknown_count, c.occurrence_count,
                (SELECT source_json FROM eligible AS client
                 WHERE client.service_key = c.service_key AND client.protocol = c.protocol
                   AND client.method = c.method
                   AND client.route_path = c.route_path AND client.role = 'consumer'
                 ORDER BY client.file_path, client.id LIMIT 1),
                COALESCE(
                  (SELECT json_object(
                      'id', owner.id, 'kind', owner.kind, 'label', owner.label,
                      'source', json(owner.source_json), 'evidence', relation.evidence)
                   FROM eligible AS handler
                   JOIN graph_edges AS relation ON relation.target = handler.id
                     AND relation.kind = 'handles'
                   JOIN graph_nodes AS owner ON owner.id = relation.source
                   WHERE handler.service_key = c.service_key AND handler.protocol = c.protocol
                     AND handler.method = c.method
                     AND handler.route_path = c.route_path AND handler.role = 'producer'
                   ORDER BY handler.file_path, handler.id LIMIT 1),
                  (SELECT CASE WHEN COUNT(DISTINCT owner.id) = 1 THEN json_object(
                      'id', MIN(owner.id), 'kind', MIN(owner.kind), 'label', MIN(owner.label),
                      'source', json(MIN(owner.source_json)), 'evidence', 'inferred') END
                   FROM eligible AS handler
                   JOIN graph_edges AS relation ON relation.target = handler.id
                     AND relation.kind = 'handles'
                   JOIN graph_nodes AS owner ON owner.id = relation.source
                   WHERE c.framework = 'OpenAPI' AND c.operation_id IS NOT NULL
                     AND owner.kind IN ('function', 'method', 'handler')
                     AND handler.service_key = c.service_key AND handler.protocol = c.protocol
                     AND handler.method = c.method AND handler.role = 'producer'
                     AND handler.framework = 'FastAPI' AND handler.route_path <> '/'
                     AND length(handler.route_path) < length(c.route_path)
                     AND substr(c.route_path, -length(handler.route_path)) = handler.route_path
                     AND substr(c.operation_id, 1, length(owner.label) + 1) = owner.label || '_')
                )
         FROM filtered AS c
         WHERE :cursor IS NULL OR c.id > :cursor
         ORDER BY c.id LIMIT :limit"
    );
    let mut statement = store.connection.prepare(&sql)?;
    let rows = statement.query_map(
        named_params! {
            ":pattern": pattern,
            ":cursor": cursor_id,
            ":limit": limit as i64,
        },
        endpoint_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn endpoint_from_row(row: &Row<'_>) -> rusqlite::Result<ApiEndpoint> {
    let source_file = row.get::<_, String>(4)?;
    let source = source_from_json(&row.get::<_, String>(5)?);
    let spec_title = row.get::<_, Option<String>>(10)?;
    let api_group = row.get::<_, Option<String>>(11)?;
    let producer_count = usize_value(row, 12)?;
    let consumer_count = usize_value(row, 13)?;
    let unknown_count = usize_value(row, 14)?;
    let occurrence_count = usize_value(row, 15)?;
    let first_client_source = row
        .get::<_, Option<String>>(16)?
        .and_then(|value| source_from_json(&value));
    let handler = row
        .get::<_, Option<String>>(17)?
        .and_then(|value| serde_json::from_str::<HandlerRow>(&value).ok())
        .map(ApiEndpointHandler::from);
    Ok(ApiEndpoint {
        id: row.get(0)?,
        label: row.get(1)?,
        method: row.get(2)?,
        path: row.get(3)?,
        role: match (producer_count, consumer_count, unknown_count) {
            (count, _, _) if count > 0 => ApiEndpointRole::Producer,
            (_, count, _) if count > 0 => ApiEndpointRole::Consumer,
            _ => ApiEndpointRole::Unknown,
        },
        protocol: ApiProtocol::Http,
        source_file: source_file.clone(),
        source,
        language: row.get(6)?,
        evidence: evidence_from_db(&row.get::<_, String>(7)?),
        framework: row.get(8)?,
        operation_id: row.get(9)?,
        grouping: grouping(&source_file, spec_title, api_group),
        handler,
        client_coverage: ApiClientCoverage {
            count: consumer_count,
            first_source: first_client_source,
        },
        occurrence_count,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandlerRow {
    id: String,
    kind: String,
    label: String,
    source: Option<SourceLocation>,
    evidence: String,
}

impl From<HandlerRow> for ApiEndpointHandler {
    fn from(value: HandlerRow) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            label: value.label,
            source: value.source,
            evidence: evidence_from_db(&value.evidence),
        }
    }
}

fn grouping(
    source_file: &str,
    spec_title: Option<String>,
    api_group: Option<String>,
) -> ApiEndpointGrouping {
    if let (Some(title), Some(group)) = (spec_title, api_group) {
        let segments = vec![title, group];
        return ApiEndpointGrouping {
            key: segments.join(" / "),
            segments,
            basis: ApiGroupingBasis::OpenApiTag,
        };
    }
    let mut segments = Path::new(source_file)
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Vec<_>>();
    if segments.is_empty() {
        segments.push("Workspace root".into());
    }
    ApiEndpointGrouping {
        key: segments.join(" / "),
        segments,
        basis: ApiGroupingBasis::SourcePath,
    }
}

fn source_from_json(value: &str) -> Option<SourceLocation> {
    serde_json::from_str::<Option<SourceLocation>>(value)
        .ok()
        .flatten()
}

fn usize_value(row: &Row<'_>, index: usize) -> rusqlite::Result<usize> {
    Ok(row.get::<_, i64>(index)?.max(0) as usize)
}

fn validated_query(query: Option<&str>) -> AoneResult<Option<&str>> {
    let Some(query) = query else {
        return Ok(None);
    };
    if query.chars().count() > MAX_QUERY_CHARS || query.chars().any(char::is_control) {
        return Err(AoneError::InvalidRequest(format!(
            "API inventory query must be at most {MAX_QUERY_CHARS} non-control characters"
        )));
    }
    let query = query.trim();
    Ok((!query.is_empty()).then_some(query))
}

fn encode_cursor(id: &str, query: Option<&str>) -> String {
    format!("{id}.{}", query_hash(query))
}

fn decode_cursor(cursor: Option<&str>, query: Option<&str>) -> AoneResult<Option<String>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    if cursor.chars().count() > MAX_CURSOR_CHARS || cursor.chars().any(char::is_control) {
        return Err(AoneError::InvalidRequest(
            "API inventory cursor is invalid".into(),
        ));
    }
    let Some((id, hash)) = cursor.rsplit_once('.') else {
        return Err(AoneError::InvalidRequest(
            "API inventory cursor is invalid".into(),
        ));
    };
    if id.is_empty() || hash != query_hash(query) {
        return Err(AoneError::InvalidRequest(
            "API inventory cursor does not match this query".into(),
        ));
    }
    Ok(Some(id.into()))
}

fn query_hash(query: Option<&str>) -> String {
    let normalized = query.unwrap_or_default().trim().to_ascii_lowercase();
    blake3::hash(normalized.as_bytes()).to_hex()[..12].to_owned()
}
