import {
  ArrowsInLineVertical,
  ArrowsOutLineVertical,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import type { ApiCatalogFilters, ApiCoverageFilter, ApiInventoryCounts } from "./model";

interface ApiCatalogToolbarProps {
  filters: ApiCatalogFilters;
  methods: string[];
  counts: ApiInventoryCounts;
  filteredCount: number;
  loadedCount: number;
  total: number;
  indexedTotal: number;
  loading: boolean;
  onFiltersChange: (filters: ApiCatalogFilters) => void;
  onExpandAll: () => void;
  onCollapseAll: () => void;
}

const roles: Array<{ value: ApiCoverageFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "server", label: "Server" },
  { value: "client-covered", label: "Client coverage" },
  { value: "client-only", label: "Client only" },
  { value: "unknown", label: "Unclassified" },
];

export function ApiCatalogToolbar({
  filters,
  methods,
  counts,
  filteredCount,
  loadedCount,
  total,
  indexedTotal,
  loading,
  onFiltersChange,
  onExpandAll,
  onCollapseAll,
}: ApiCatalogToolbarProps) {
  const roleCount = (role: ApiCoverageFilter): number => {
    if (role === "all") return total;
    if (role === "server") return counts.producer;
    if (role === "client-covered") return counts.clientCovered;
    if (role === "client-only") return counts.consumerOnly;
    return counts.unknown;
  };
  return (
    <header className="api-catalog-toolbar">
      <div className="api-catalog-summary" aria-live="polite">
        <strong>API inventory</strong>
        <span>
          {loading ? `Loading ${loadedCount} of ${total}` : `Loaded ${loadedCount} of ${total}`}
        </span>
        {indexedTotal !== total && <span>{indexedTotal} indexed total</span>}
        {filteredCount !== loadedCount && <span>{filteredCount} matching</span>}
      </div>

      <label className="api-catalog-search">
        <span className="sr-only">Search API inventory</span>
        <MagnifyingGlass size={14} aria-hidden="true" />
        <input
          type="search"
          maxLength={128}
          value={filters.query}
          placeholder="Search method, route, handler, or source"
          onChange={(event) => onFiltersChange({ ...filters, query: event.target.value })}
        />
        {filters.query && (
          <button
            type="button"
            aria-label="Clear API search"
            onClick={() => onFiltersChange({ ...filters, query: "" })}
          >
            <X size={12} />
          </button>
        )}
      </label>

      <div className="api-role-filter" role="group" aria-label="API evidence coverage">
        {roles.map((role) => (
          <button
            key={role.value}
            type="button"
            className={filters.coverage === role.value ? "is-active" : ""}
            aria-pressed={filters.coverage === role.value}
            aria-label={`${role.label} ${roleCount(role.value)}`}
            onClick={() => onFiltersChange({ ...filters, coverage: role.value })}
          >
            <span>{role.label}</span>
            <small>{roleCount(role.value)}</small>
          </button>
        ))}
      </div>

      <label className="api-method-filter">
        <span>Method</span>
        <select
          aria-label="HTTP method"
          value={filters.method}
          onChange={(event) => onFiltersChange({ ...filters, method: event.target.value })}
        >
          <option value="ALL">All</option>
          {methods.map((method) => <option value={method} key={method}>{method}</option>)}
        </select>
      </label>

      <div className="api-expand-controls" role="group" aria-label="API groups">
        <button type="button" onClick={onExpandAll} title="Expand all groups" aria-label="Expand all API groups">
          <ArrowsOutLineVertical size={13} />
          <span>Expand</span>
        </button>
        <button type="button" onClick={onCollapseAll} title="Collapse all groups" aria-label="Collapse all API groups">
          <ArrowsInLineVertical size={13} />
          <span>Collapse</span>
        </button>
      </div>
    </header>
  );
}
