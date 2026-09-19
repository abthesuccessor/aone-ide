import type { EvidenceKind, SourceLocation } from "../../types";

export type ApiEndpointRole = "producer" | "consumer" | "unknown";
export type ApiProtocol = "http" | "webSocket" | "event" | "rpc" | "unknown";

export interface ApiEndpointOwner {
  id: string;
  kind: string;
  label: string;
  source?: SourceLocation;
}

export interface ApiClientCoverage {
  count: number;
  firstSource?: SourceLocation;
}

export interface ApiEndpointGrouping {
  key: string;
  segments: string[];
  basis: "openApiTag" | "sourcePath";
}

export interface ApiEndpointInventoryItem {
  id: string;
  label: string;
  method: string;
  path: string;
  sourceFile: string;
  source?: SourceLocation;
  language?: string;
  framework?: string;
  operationId?: string;
  evidence: EvidenceKind;
  role: ApiEndpointRole;
  protocol: ApiProtocol;
  grouping: ApiEndpointGrouping;
  handler?: ApiEndpointOwner & { evidence: EvidenceKind };
  clientCoverage: ApiClientCoverage;
  occurrenceCount: number;
}

export interface ApiEndpointInventoryRequest {
  workspaceId: string;
  query?: string;
  cursor?: string;
  limit?: number;
}

export interface ApiEndpointInventoryPage {
  workspaceId: string;
  endpoints: ApiEndpointInventoryItem[];
  total: number;
  indexedTotal: number;
  returned: number;
  nextCursor?: string;
  truncated: boolean;
  counts: ApiInventoryCounts;
}


export interface ApiInventoryCounts {
  producer: number;
  consumerOnly: number;
  clientCovered: number;
  unknown: number;
}

export type ApiCoverageFilter = "all" | "server" | "client-covered" | "client-only" | "unknown";

export interface ApiCatalogFilters {
  query: string;
  coverage: ApiCoverageFilter;
  method: string;
}

export interface ApiRouteGroup {
  key: string;
  label: string;
  endpoints: ApiEndpointInventoryItem[];
}

export interface ApiSourceGroup {
  key: string;
  label: string;
  segments: string[];
  role: ApiEndpointRole;
  endpoints: ApiEndpointInventoryItem[];
  routes: ApiRouteGroup[];
}

const roleOrder: Record<ApiEndpointRole, number> = {
  producer: 0,
  consumer: 1,
  unknown: 2,
};

function compareText(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });
}

export function apiRoleLabel(role: ApiEndpointRole): string {
  if (role === "producer") return "Server endpoints";
  if (role === "consumer") return "Client requests";
  return "Unclassified API references";
}

export function apiRoleDescription(role: ApiEndpointRole): string {
  if (role === "producer") return "Routes declared by this workspace";
  if (role === "consumer") return "Outbound calls made by this workspace";
  return "API evidence without a confirmed direction";
}

export function normalizedApiMethod(endpoint: ApiEndpointInventoryItem): string {
  const method = endpoint.method.trim().toUpperCase();
  return method || "ANY";
}

function routeBranch(endpoint: ApiEndpointInventoryItem): string {
  const path = endpoint.path.trim();
  if (!path) return "Target not resolved";
  if (/^https?:\/\//i.test(path)) {
    try {
      const target = new URL(path);
      const prefix = target.pathname.split("/").filter(Boolean).slice(0, 2);
      return `${target.origin}${prefix.length > 0 ? `/${prefix.join("/")}` : ""}`;
    } catch {
      return "External HTTP target";
    }
  }
  const cleanPath = path.split(/[?#]/, 1)[0] ?? path;
  const parts = cleanPath.split("/").filter(Boolean);
  if (parts.length === 0) return "/";
  const branch: string[] = [];
  for (const part of parts) {
    if (/^[:{[]/.test(part)) break;
    branch.push(part);
    if (branch.length >= 3) break;
  }
  return branch.length > 0 ? `/${branch.join("/")}` : "/dynamic";
}

function sourceGroupLabel(endpoint: ApiEndpointInventoryItem): string {
  const segments = endpoint.grouping.segments.filter(Boolean);
  if (segments.length > 0) return segments.join(" / ");
  return endpoint.grouping.key || endpoint.sourceFile;
}

export function filterApiEndpoints(
  endpoints: readonly ApiEndpointInventoryItem[],
  filters: ApiCatalogFilters,
): ApiEndpointInventoryItem[] {
  const query = filters.query.trim().toLocaleLowerCase();
  const method = filters.method.trim().toUpperCase();
  return endpoints.filter((endpoint) => {
    if (filters.coverage === "server" && endpoint.role !== "producer") return false;
    if (filters.coverage === "client-covered" && (
      endpoint.role !== "producer" || endpoint.clientCoverage.count === 0
    )) return false;
    if (filters.coverage === "client-only" && endpoint.role !== "consumer") return false;
    if (filters.coverage === "unknown" && endpoint.role !== "unknown") return false;
    if (method !== "ALL" && normalizedApiMethod(endpoint) !== method) return false;
    if (!query) return true;
    const haystack = [
      endpoint.method,
      endpoint.path,
      endpoint.label,
      endpoint.sourceFile,
      endpoint.language,
      endpoint.handler?.label,
      endpoint.handler?.kind,
      endpoint.operationId,
      endpoint.framework,
      ...endpoint.grouping.segments,
    ].filter(Boolean).join(" ").toLocaleLowerCase();
    return haystack.includes(query);
  });
}

export function apiMethods(endpoints: readonly ApiEndpointInventoryItem[]): string[] {
  return [...new Set(endpoints.map(normalizedApiMethod))].sort(compareText);
}

export function countApiRoles(
  endpoints: readonly ApiEndpointInventoryItem[],
): Record<ApiEndpointRole, number> {
  const counts = { producer: 0, consumer: 0, unknown: 0 };
  for (const endpoint of endpoints) counts[endpoint.role] += 1;
  return counts;
}

export function buildApiSourceGroups(
  endpoints: readonly ApiEndpointInventoryItem[],
): ApiSourceGroup[] {
  const sources = new Map<string, ApiEndpointInventoryItem[]>();
  for (const endpoint of endpoints) {
    const key = `${endpoint.role}:${endpoint.grouping.key}`;
    const current = sources.get(key);
    if (current) current.push(endpoint);
    else sources.set(key, [endpoint]);
  }

  const groups: ApiSourceGroup[] = [];
  for (const [key, sourceEndpoints] of sources) {
    const first = sourceEndpoints[0];
    if (!first) continue;
    const routes = new Map<string, ApiEndpointInventoryItem[]>();
    for (const endpoint of sourceEndpoints) {
      const route = routeBranch(endpoint);
      const current = routes.get(route);
      if (current) current.push(endpoint);
      else routes.set(route, [endpoint]);
    }
    const sortedEndpoints = [...sourceEndpoints].sort((left, right) => (
      compareText(left.path || left.label, right.path || right.label)
      || compareText(normalizedApiMethod(left), normalizedApiMethod(right))
      || compareText(left.id, right.id)
    ));
    groups.push({
      key,
      label: sourceGroupLabel(first),
      segments: first.grouping.segments,
      role: first.role,
      endpoints: sortedEndpoints,
      routes: [...routes.entries()]
        .map(([routeKey, routeEndpoints]) => ({
          key: `${key}:${routeKey}`,
          label: routeKey,
          endpoints: [...routeEndpoints].sort((left, right) => (
            compareText(left.path || left.label, right.path || right.label)
            || compareText(normalizedApiMethod(left), normalizedApiMethod(right))
            || compareText(left.id, right.id)
          )),
        }))
        .sort((left, right) => compareText(left.label, right.label)),
    });
  }

  return groups.sort((left, right) => (
    roleOrder[left.role] - roleOrder[right.role]
    || compareText(left.label, right.label)
    || compareText(left.key, right.key)
  ));
}
