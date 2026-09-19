import { Code, Database, GitBranch, Warning } from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import type { SourceLocation } from "../../types";
import { ApiCatalogToolbar } from "./ApiCatalogToolbar";
import { ApiEndpointTree } from "./ApiEndpointTree";
import {
  apiMethods,
  buildApiSourceGroups,
  filterApiEndpoints,
  normalizedApiMethod,
  type ApiCatalogFilters,
  type ApiEndpointInventoryItem,
} from "./model";
import { useApiCatalog, type ApiCatalogPageLoader } from "./useApiCatalog";

interface ApiCatalogProps {
  workspaceId: string | undefined;
  workspaceGeneration: number;
  loadPage: ApiCatalogPageLoader;
  onOpenSource: (relativePath: string, source?: SourceLocation) => void;
  onTraceFlow: (endpoint: ApiEndpointInventoryItem) => void;
}

const INITIAL_FILTERS: ApiCatalogFilters = {
  query: "",
  coverage: "all",
  method: "ALL",
};

function LoadingRows() {
  return (
    <div className="api-catalog-skeleton" role="status" aria-live="polite">
      <strong>Loading the indexed API inventory</strong>
      <span>Pages appear as they are read from the local index.</span>
      {Array.from({ length: 8 }, (_, index) => <i key={index} aria-hidden="true" />)}
    </div>
  );
}

export function ApiCatalog({
  workspaceId,
  workspaceGeneration,
  loadPage,
  onOpenSource,
  onTraceFlow,
}: ApiCatalogProps) {
  const [filters, setFilters] = useState<ApiCatalogFilters>(INITIAL_FILTERS);
  const { endpoints, total, indexedTotal, counts, loading, error, retry } = useApiCatalog(
    workspaceId,
    workspaceGeneration,
    filters.query,
    loadPage,
  );
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [groupLimits, setGroupLimits] = useState<Map<string, number>>(new Map());
  const [selectedEndpointId, setSelectedEndpointId] = useState<string | null>(null);
  const filteredEndpoints = useMemo(
    () => filterApiEndpoints(endpoints, filters),
    [endpoints, filters],
  );
  const groups = useMemo(() => buildApiSourceGroups(filteredEndpoints), [filteredEndpoints]);
  const methods = useMemo(() => apiMethods(endpoints), [endpoints]);
  const selectedEndpoint = useMemo(
    () => endpoints.find((endpoint) => endpoint.id === selectedEndpointId) ?? null,
    [endpoints, selectedEndpointId],
  );
  const selectedSource = selectedEndpoint?.source ?? selectedEndpoint?.handler?.source;

  useEffect(() => {
    setSelectedEndpointId(null);
    setExpandedGroups(new Set());
    setGroupLimits(new Map());
    setFilters(INITIAL_FILTERS);
  }, [workspaceId, workspaceGeneration]);

  useEffect(() => {
    if (groups.length === 0) return;
    setExpandedGroups((current) => {
      if (current.size > 0) return current;
      const firstByRole = new Map<string, string>();
      for (const group of groups) {
        if (!firstByRole.has(group.role)) firstByRole.set(group.role, group.key);
      }
      return new Set(firstByRole.values());
    });
  }, [groups]);

  useEffect(() => {
    if (selectedEndpointId && !filteredEndpoints.some((item) => item.id === selectedEndpointId)) {
      setSelectedEndpointId(null);
    }
  }, [filteredEndpoints, selectedEndpointId]);

  const searchActive = filters.query.trim().length > 0;
  const visibleExpandedGroups = searchActive
    ? new Set(groups.map((group) => group.key))
    : expandedGroups;
  const openEndpointSource = (endpoint: ApiEndpointInventoryItem) => {
    const location = endpoint.source
      ?? endpoint.handler?.source;
    onOpenSource(location?.relativePath ?? endpoint.sourceFile, location);
  };

  if (!workspaceId) {
    return (
      <div className="api-catalog-state">
        <Database size={30} weight="thin" />
        <strong>Open a workspace to inventory its APIs</strong>
      </div>
    );
  }

  return (
    <section className="api-catalog" aria-label="API catalog" aria-busy={loading}>
      <ApiCatalogToolbar
        filters={filters}
        methods={methods}
        counts={counts}
        filteredCount={filteredEndpoints.length}
        loadedCount={endpoints.length}
        total={total}
        indexedTotal={indexedTotal}
        loading={loading}
        onFiltersChange={(next) => {
          if (next.query !== filters.query) setGroupLimits(new Map());
          setFilters(next);
        }}
        onExpandAll={() => setExpandedGroups(new Set(groups.map((group) => group.key)))}
        onCollapseAll={() => setExpandedGroups(new Set())}
      />

      <p className="api-catalog-help" id="api-catalog-help">
        Each unique operation appears once. Client coverage is attached without inflating the API total.
        Select an operation to open its source or trace its loaded execution flow.
      </p>

      {(error || counts.unknown > 0) && (
        <div className="api-catalog-notices">
          {counts.unknown > 0 && (
            <p className="api-classification-note" role="status">
              {counts.unknown} unclassified. Scan workspace to classify legacy API facts.
            </p>
          )}
          {error && (
            <div className="api-catalog-error" role="alert">
              <Warning size={16} weight="fill" />
              <span>
                <strong>API inventory is incomplete</strong>
                <small>{error}</small>
              </span>
              <button type="button" onClick={retry}>Retry</button>
            </div>
          )}
        </div>
      )}

      {loading && endpoints.length === 0 ? (
        <LoadingRows />
      ) : filteredEndpoints.length === 0 ? (
        <div className="api-catalog-state">
          <Database size={28} weight="thin" />
          <strong>{endpoints.length === 0 ? "No APIs were indexed" : "No APIs match these filters"}</strong>
          <span>{endpoints.length === 0
            ? "Scan the workspace after adding an API specification or route declaration."
            : "Clear the search or choose another coverage filter and method."}</span>
          {endpoints.length > 0 && (
            <button type="button" onClick={() => setFilters(INITIAL_FILTERS)}>Clear filters</button>
          )}
        </div>
      ) : (
        <ApiEndpointTree
          groups={groups}
          expandedGroups={visibleExpandedGroups}
          groupLimits={groupLimits}
          selectedEndpointId={selectedEndpointId}
          onToggleGroup={(key) => {
            setExpandedGroups((current) => {
              const next = new Set(current);
              if (next.has(key)) next.delete(key);
              else next.add(key);
              return next;
            });
          }}
          onShowMore={(key, count) => {
            setGroupLimits((current) => new Map(current).set(key, count));
          }}
          onSelectEndpoint={(endpoint) => setSelectedEndpointId(endpoint.id)}
          onOpenSource={openEndpointSource}
        />
      )}

      {selectedEndpoint && (
        <footer className="api-selection-bar" aria-live="polite">
          <span className="api-method" data-method={normalizedApiMethod(selectedEndpoint)}>
            {normalizedApiMethod(selectedEndpoint)}
          </span>
          <span>
            <strong>{selectedEndpoint.path ?? selectedEndpoint.label}</strong>
            <small>{selectedSource
              ? `${selectedSource.relativePath}:${selectedSource.startLine}`
              : selectedEndpoint.sourceFile}</small>
          </span>
          <span className="api-selection-actions">
            <button type="button" onClick={() => onTraceFlow(selectedEndpoint)}>
              <GitBranch size={14} /> Trace flow
            </button>
            <button type="button" onClick={() => openEndpointSource(selectedEndpoint)}>
              <Code size={14} /> Open source
            </button>
          </span>
        </footer>
      )}
    </section>
  );
}
