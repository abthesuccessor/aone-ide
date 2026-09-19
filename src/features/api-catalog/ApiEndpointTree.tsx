import {
  CaretDown,
  CaretRight,
  Code,
  GitBranch,
  type Icon,
} from "@phosphor-icons/react";
import type { KeyboardEvent } from "react";
import {
  apiRoleDescription,
  apiRoleLabel,
  normalizedApiMethod,
  type ApiEndpointInventoryItem,
  type ApiEndpointRole,
  type ApiSourceGroup,
} from "./model";

interface ApiEndpointTreeProps {
  groups: ApiSourceGroup[];
  expandedGroups: ReadonlySet<string>;
  groupLimits: ReadonlyMap<string, number>;
  selectedEndpointId: string | null;
  onToggleGroup: (key: string) => void;
  onShowMore: (key: string, count: number) => void;
  onSelectEndpoint: (endpoint: ApiEndpointInventoryItem) => void;
  onOpenSource: (endpoint: ApiEndpointInventoryItem) => void;
}

const ROLE_ICON: Record<ApiEndpointRole, Icon> = {
  producer: GitBranch,
  consumer: CaretRight,
  unknown: Code,
};

function focusEndpoint(event: KeyboardEvent<HTMLButtonElement>): void {
  if (!["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
  const rows = Array.from(
    event.currentTarget.closest(".api-catalog-scroll")
      ?.querySelectorAll<HTMLButtonElement>("[data-api-endpoint]") ?? [],
  );
  if (rows.length === 0) return;
  event.preventDefault();
  const index = rows.indexOf(event.currentTarget);
  const next = event.key === "Home"
    ? rows[0]
    : event.key === "End"
      ? rows[rows.length - 1]
      : rows[(index + (event.key === "ArrowUp" ? -1 : 1) + rows.length) % rows.length];
  next?.focus();
}

function EndpointRow({
  endpoint,
  selected,
  onSelect,
  onOpenSource,
}: {
  endpoint: ApiEndpointInventoryItem;
  selected: boolean;
  onSelect: () => void;
  onOpenSource: () => void;
}) {
  const method = normalizedApiMethod(endpoint);
  const path = endpoint.path.trim() || "Target not resolved";
  const sourceRange = endpoint.source
    ?? endpoint.handler?.source;
  const source = sourceRange
    ? `${sourceRange.relativePath}:${sourceRange.startLine}`
    : endpoint.sourceFile;
  const direction = endpoint.role === "producer"
    ? `server endpoint${endpoint.clientCoverage.count > 0 ? " with client coverage" : ""}`
    : endpoint.role === "consumer"
      ? "client request"
      : "unclassified API reference";
  return (
    <button
      type="button"
      className={`api-endpoint-row${selected ? " is-selected" : ""}`}
      data-api-endpoint={endpoint.id}
      data-method={method}
      aria-pressed={selected}
      aria-label={`${method} ${path}, ${direction}, ${source}`}
      title={`${endpoint.label}\n${source}\nDouble-click or Command-Enter to open source`}
      onClick={onSelect}
      onDoubleClick={onOpenSource}
      onKeyDown={(event) => {
        focusEndpoint(event);
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          onOpenSource();
        }
      }}
    >
      <span className="api-method" data-method={method}>{method}</span>
      <span className="api-operation-main">
        <strong>{path}</strong>
        <span>{endpoint.label}</span>
      </span>
      <span className="api-owner">
        <strong>{endpoint.handler?.label ?? endpoint.operationId ?? "Handler not linked"}</strong>
        <span>{endpoint.handler?.kind ?? endpoint.framework ?? endpoint.language ?? "Unknown owner"}</span>
      </span>
      <span className="api-coverage">
        <small>{endpoint.role === "producer"
          ? "Server declared"
          : endpoint.role === "consumer" ? "Client only" : "Direction unknown"}</small>
        {endpoint.role === "producer" && endpoint.clientCoverage.count > 0 && (
          <small className="has-client-coverage">Client coverage {endpoint.clientCoverage.count}</small>
        )}
      </span>
      <span className="api-source">
        <span>{sourceRange?.relativePath ?? endpoint.sourceFile}</span>
        <small>{sourceRange ? `line ${sourceRange.startLine}` : "file source"}</small>
      </span>
      <Code className="api-open-hint" size={14} aria-hidden="true" />
    </button>
  );
}

function SourceGroup({
  group,
  expanded,
  visibleLimit,
  selectedEndpointId,
  onToggle,
  onShowMore,
  onSelectEndpoint,
  onOpenSource,
}: {
  group: ApiSourceGroup;
  expanded: boolean;
  visibleLimit: number;
  selectedEndpointId: string | null;
  onToggle: () => void;
  onShowMore: (count: number) => void;
  onSelectEndpoint: (endpoint: ApiEndpointInventoryItem) => void;
  onOpenSource: (endpoint: ApiEndpointInventoryItem) => void;
}) {
  let remaining = visibleLimit;
  return (
    <article className="api-source-group" data-role={group.role}>
      <button
        type="button"
        className="api-source-toggle"
        aria-expanded={expanded}
        aria-label={`${group.label}, ${group.endpoints.length} operations`}
        onClick={onToggle}
      >
        {expanded ? <CaretDown size={13} /> : <CaretRight size={13} />}
        <span className="api-source-title">
          <strong>{group.label}</strong>
          <small>{group.segments.length > 0 ? group.segments.join(" / ") : "Workspace source"}</small>
        </span>
        <span className="api-group-count">{group.endpoints.length}</span>
      </button>
      {expanded && (
        <div className="api-route-groups">
          {group.routes.map((route) => {
            const shown = route.endpoints.slice(0, Math.max(remaining, 0));
            remaining -= shown.length;
            if (shown.length === 0) return null;
            return (
              <section className="api-route-group" key={route.key}>
                <div className="api-route-heading">
                  <GitBranch size={12} aria-hidden="true" />
                  <strong>{route.label}</strong>
                  <span>{route.endpoints.length} operations</span>
                </div>
                <div className="api-endpoint-list">
                  {shown.map((endpoint) => (
                    <EndpointRow
                      key={endpoint.id}
                      endpoint={endpoint}
                      selected={endpoint.id === selectedEndpointId}
                      onSelect={() => onSelectEndpoint(endpoint)}
                      onOpenSource={() => onOpenSource(endpoint)}
                    />
                  ))}
                </div>
              </section>
            );
          })}
          {visibleLimit < group.endpoints.length && (
            <button
              type="button"
              className="api-show-more"
              onClick={() => onShowMore(Math.min(group.endpoints.length, visibleLimit + 80))}
            >
              Show next {Math.min(80, group.endpoints.length - visibleLimit)} operations
            </button>
          )}
        </div>
      )}
    </article>
  );
}

export function ApiEndpointTree({
  groups,
  expandedGroups,
  groupLimits,
  selectedEndpointId,
  onToggleGroup,
  onShowMore,
  onSelectEndpoint,
  onOpenSource,
}: ApiEndpointTreeProps) {
  const groupsByRole = new Map<ApiEndpointRole, ApiSourceGroup[]>();
  for (const group of groups) {
    const current = groupsByRole.get(group.role);
    if (current) current.push(group);
    else groupsByRole.set(group.role, [group]);
  }
  return (
    <div className="api-catalog-scroll" role="region" aria-label="API endpoint inventory">
      {(["producer", "consumer", "unknown"] as const).map((role) => {
        const roleGroups = groupsByRole.get(role) ?? [];
        if (roleGroups.length === 0) return null;
        const Icon = ROLE_ICON[role];
        const count = roleGroups.reduce((total, group) => total + group.endpoints.length, 0);
        return (
          <section className="api-role-section" key={role} aria-labelledby={`api-role-${role}`}>
            <header className="api-role-heading">
              <Icon size={15} aria-hidden="true" />
              <span>
                <strong id={`api-role-${role}`}>{apiRoleLabel(role)}</strong>
                <small>{apiRoleDescription(role)}</small>
              </span>
              <strong className="api-role-count">{count}</strong>
            </header>
            <div className="api-source-groups">
              {roleGroups.map((group) => (
                <SourceGroup
                  key={group.key}
                  group={group}
                  expanded={expandedGroups.has(group.key)}
                  visibleLimit={groupLimits.get(group.key) ?? 80}
                  selectedEndpointId={selectedEndpointId}
                  onToggle={() => onToggleGroup(group.key)}
                  onShowMore={(count) => onShowMore(group.key, count)}
                  onSelectEndpoint={onSelectEndpoint}
                  onOpenSource={onOpenSource}
                />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}
