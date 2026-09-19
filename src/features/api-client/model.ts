import type { ApiHeader, ApiRequest } from "../../types";

export type ApiClientProtocol = "rest" | "graphql";

export interface ApiClientDraft {
  protocol: ApiClientProtocol;
  method: string;
  url: string;
  headers: string;
  restBody: string;
  graphqlBody: string;
  graphqlVariables: string;
}

export interface ApiClientTarget {
  id: string;
  method: string;
  path: string;
  protocol?: string;
  handler?: { id: string };
}

const BODYLESS_METHODS = new Set(["GET", "HEAD", "OPTIONS"]);
const JSON_CONTENT_TYPE_HEADER = "content-type: application/json";
const NEUTRAL_GRAPHQL_QUERY = "query {\n  __typename\n}";

export const DEFAULT_API_CLIENT_DRAFT: ApiClientDraft = {
  protocol: "rest",
  method: "GET",
  url: "http://127.0.0.1:3000/",
  headers: "",
  restBody: "",
  graphqlBody: NEUTRAL_GRAPHQL_QUERY,
  graphqlVariables: "{}",
};

export function createApiClientDraft(
  initial: Partial<ApiClientDraft> = {},
): ApiClientDraft {
  return { ...DEFAULT_API_CLIENT_DRAFT, ...initial };
}

export function parseApiHeaders(value: string): ApiHeader[] {
  const result: ApiHeader[] = [];
  for (const line of value.split("\n")) {
    const colon = line.indexOf(":");
    if (colon <= 0) continue;
    const name = line.slice(0, colon).trim();
    const headerValue = line.slice(colon + 1).trim();
    if (name) result.push({ name, value: headerValue });
  }
  return result;
}

export function buildApiRequest(
  draft: ApiClientDraft,
  debugActive: boolean,
): ApiRequest {
  const unresolvedPlaceholders = findUnresolvedRoutePlaceholders(draft.url);
  if (unresolvedPlaceholders.length > 0) {
    throw new Error(`Resolve route parameters before sending: ${unresolvedPlaceholders.join(", ")}`);
  }
  const method = draft.protocol === "graphql"
    ? "POST"
    : draft.method.trim().toUpperCase() || "GET";
  let body = BODYLESS_METHODS.has(method) ? undefined : draft.restBody.trim() || undefined;
  if (draft.protocol === "graphql") {
    const variables = draft.graphqlVariables.trim()
      ? JSON.parse(draft.graphqlVariables) as unknown
      : undefined;
    body = JSON.stringify({
      query: draft.graphqlBody,
      ...(variables === undefined ? {} : { variables }),
    });
  }
  return {
    method,
    url: draft.url,
    headers: parseApiHeaders(draft.headers),
    body,
    timeoutMs: debugActive ? 60_000 : 15_000,
  };
}

function routePortion(value: string): string {
  try {
    const parsed = new URL(value);
    const route = `${parsed.pathname}${parsed.search}${parsed.hash}`;
    try {
      return decodeURIComponent(route);
    } catch {
      return route;
    }
  } catch {
    return value;
  }
}

export function findUnresolvedRoutePlaceholders(url: string): string[] {
  const route = routePortion(url);
  const placeholders = new Set<string>();
  for (const match of route.matchAll(/\{[A-Za-z_][A-Za-z0-9_-]*(?::[^{}\/]+)?\}/g)) {
    placeholders.add(match[0]);
  }
  for (const match of route.matchAll(/(?:^|[/?&=])(:[A-Za-z_][A-Za-z0-9_]*)/g)) {
    const placeholder = match[1];
    if (placeholder) placeholders.add(placeholder);
  }
  return [...placeholders];
}

function targetUrl(currentUrl: string, path: string): string {
  const trimmed = path.trim();
  if (!trimmed) return currentUrl;
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  try {
    const origin = new URL(currentUrl).origin;
    return `${origin}/${trimmed.replace(/^\/+/, "")}`;
  } catch {
    return trimmed;
  }
}

function targetProtocol(target: ApiClientTarget): ApiClientProtocol {
  return target.protocol?.toLocaleLowerCase() === "graphql" ? "graphql" : "rest";
}

export function applyEndpointToDraft(
  draft: ApiClientDraft,
  target: ApiClientTarget,
): ApiClientDraft {
  const protocol = targetProtocol(target);
  const targetMethod = target.method.trim().toUpperCase() || "GET";
  const method = protocol === "graphql" ? "POST" : targetMethod;
  const bodyless = BODYLESS_METHODS.has(method);
  return {
    ...draft,
    protocol,
    method,
    url: targetUrl(draft.url, target.path),
    headers: bodyless ? "" : JSON_CONTENT_TYPE_HEADER,
    restBody: bodyless ? "" : "{}",
    graphqlBody: NEUTRAL_GRAPHQL_QUERY,
    graphqlVariables: "{}",
  };
}

export interface ParsedApiBody {
  kind: "empty" | "json" | "text";
  value: unknown;
}

export function parseApiBody(value: string | undefined): ParsedApiBody {
  const text = value?.trim() ?? "";
  if (!text) return { kind: "empty", value: null };
  try {
    return { kind: "json", value: JSON.parse(text) as unknown };
  } catch {
    return { kind: "text", value };
  }
}
