import type { GraphEdge, GraphMetadata, GraphNode } from "../types";

export type GraphLaneId = "source" | "interface" | "domain" | "data" | "external" | "runtime";

export interface GraphLane {
  id: GraphLaneId;
  label: string;
  targetX: number;
}

export const GRAPH_LANES: readonly GraphLane[] = [
  { id: "source", label: "Source", targetX: 105 },
  { id: "interface", label: "API / UI", targetX: 290 },
  { id: "domain", label: "Domain", targetX: 475 },
  { id: "data", label: "Data", targetX: 660 },
  { id: "external", label: "External", targetX: 845 },
  { id: "runtime", label: "Runtime / Analysis", targetX: 1_015 },
];

const laneById = new Map(GRAPH_LANES.map((lane) => [lane.id, lane]));

const explicitLayerLanes: Record<string, GraphLaneId> = {
  configuration: "source",
  entry: "source",
  application: "domain",
  source: "source",
  web: "interface",
  ui: "interface",
  api: "interface",
  interface: "interface",
  domain: "domain",
  data: "data",
  external: "external",
  runtime: "runtime",
  analysis: "runtime",
};

const laneKindTokens: Array<[GraphLaneId, readonly string[]]> = [
  ["runtime", ["runtime", "trace", "span", "observation", "log", "risk", "analysis", "warning"]],
  ["external", ["external", "vendor", "sdk", "dependency", "broker", "queue", "topic"]],
  ["data", ["repository", "model", "entity", "schema", "table", "database", "collection", "document", "heading", "sentence", "record", "store", "cache", "persistence", "dao"]],
  ["interface", ["api", "endpoint", "event", "listener", "subscription", "publication", "route", "router", "controller", "handler", "component", "page", "view", "screen", "hook", "web", "client", "http", "rest", "graphql"]],
  ["source", ["file", "module", "namespace", "package", "directory", "folder", "import", "export"]],
];

function normalizedToken(value: string): string {
  return value.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

function lane(id: GraphLaneId): GraphLane {
  const resolved = laneById.get(id) ?? laneById.get("domain");
  if (!resolved) throw new Error("Graph lane configuration is incomplete");
  return resolved;
}

export function graphLaneForNode(node: Pick<GraphNode, "kind" | "metadata">): GraphLane {
  const explicitValue = typeof node.metadata?.systemLayer === "string"
    ? node.metadata.systemLayer
    : node.metadata?.layer;
  const explicitLayer = typeof explicitValue === "string"
    ? explicitLayerLanes[normalizedToken(explicitValue)]
    : undefined;
  if (explicitLayer) return lane(explicitLayer);

  const kind = normalizedToken(node.kind);
  for (const [laneId, tokens] of laneKindTokens) {
    if (tokens.some((token) => kind === token || kind.includes(token))) return lane(laneId);
  }
  return lane("domain");
}

export function truncateGraphLabel(value: string, maxCharacters = 25): string {
  const normalized = value.replace(/\s+/g, " ").trim() || "Untitled";
  const characters = Array.from(normalized);
  if (characters.length <= maxCharacters) return normalized;
  if (maxCharacters <= 1) return "…";
  return `${characters.slice(0, maxCharacters - 1).join("").trimEnd()}…`;
}

export interface MetadataPresentationEntry {
  key: string;
  label: string;
  value: string;
}

export interface MetadataPresentation {
  keyFacts: MetadataPresentationEntry[];
  rawEvidence: MetadataPresentationEntry[];
}

const importantMetadataKeys = [
  "method",
  "route",
  "url",
  "host",
  "role",
  "resolver",
  "table",
  "database",
  "engine",
  "status",
  "statuscode",
  "durationms",
  "traceid",
  "requestid",
  "eventid",
  "eventapi",
  "eventdirection",
  "eventoperation",
  "staticeventname",
  "profileid",
  "confidence",
  "observationmode",
  "documentformat",
  "headinglevel",
  "documentorder",
  "communityid",
  "communitysize",
  "communityalgorithm",
  "communitybasis",
  "communitycomplete",
  "basis",
  "reason",
  "detail",
  "note",
] as const;

const importantMetadataRank = new Map<string, number>(
  importantMetadataKeys.map((key, index) => [key, index]),
);

function compareText(left: string, right: string): number {
  if (left === right) return 0;
  return left < right ? -1 : 1;
}

function humanizeMetadataKey(key: string): string {
  const words = key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/[-_]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  const acronyms: Record<string, string> = { api: "API", http: "HTTP", id: "ID", url: "URL" };
  return words.map((word, index) => {
    const normalized = word.toLowerCase();
    if (acronyms[normalized]) return acronyms[normalized];
    if (normalized === "ms") return "ms";
    return index === 0 ? `${normalized.charAt(0).toUpperCase()}${normalized.slice(1)}` : normalized;
  }).join(" ");
}

function stableJson(value: unknown, seen: WeakSet<object>, depth: number): string {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : JSON.stringify(String(value));
  if (typeof value === "boolean") return String(value);
  if (typeof value === "bigint") return JSON.stringify(String(value));
  if (typeof value === "undefined") return JSON.stringify("[undefined]");
  if (typeof value === "function" || typeof value === "symbol") return JSON.stringify(`[${typeof value}]`);
  if (depth >= 8) return JSON.stringify("[max depth]");

  const object = value as object;
  if (seen.has(object)) return JSON.stringify("[circular]");
  if (value instanceof Date) return JSON.stringify(value.toISOString());

  seen.add(object);
  let result: string;
  if (Array.isArray(value)) {
    result = `[${value.map((item) => stableJson(item, seen, depth + 1)).join(", ")}]`;
  } else {
    const record = value as Record<string, unknown>;
    result = `{${Object.keys(record)
      .sort(compareText)
      .map((key) => `${JSON.stringify(key)}: ${stableJson(record[key], seen, depth + 1)}`)
      .join(", ")}}`;
  }
  seen.delete(object);
  return result;
}

export function formatMetadataValue(value: unknown): string {
  if (typeof value === "string") return value || "(empty)";
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : String(value);
  if (typeof value === "boolean" || typeof value === "bigint") return String(value);
  if (value === null) return "null";
  if (typeof value === "undefined") return "[undefined]";
  return stableJson(value, new WeakSet<object>(), 0);
}

export function presentMetadata(metadata: GraphMetadata | undefined): MetadataPresentation {
  const entries = Object.entries(metadata ?? {}).map(([key, value]) => ({
    key,
    label: humanizeMetadataKey(key),
    value: formatMetadataValue(value),
  }));

  const keyFacts = entries
    .filter((entry) => importantMetadataRank.has(normalizedToken(entry.key)))
    .sort((left, right) => {
      const rankDifference = (importantMetadataRank.get(normalizedToken(left.key)) ?? Number.MAX_SAFE_INTEGER)
        - (importantMetadataRank.get(normalizedToken(right.key)) ?? Number.MAX_SAFE_INTEGER);
      return rankDifference || compareText(left.key, right.key);
    });
  const rawEvidence = entries
    .filter((entry) => !importantMetadataRank.has(normalizedToken(entry.key)))
    .sort((left, right) => compareText(left.key, right.key));

  return { keyFacts, rawEvidence };
}

const evidenceRank: Record<GraphEdge["evidence"], number> = {
  observed: 0,
  resolved: 1,
  declared: 2,
  inferred: 3,
};

function relatedNodeId(edge: GraphEdge, currentId: string): string {
  return edge.source === currentId ? edge.target : edge.source;
}

function compareRelations(
  left: GraphEdge,
  right: GraphEdge,
  currentId: string,
  nodes: ReadonlyMap<string, GraphNode>,
): number {
  const evidenceDifference = evidenceRank[left.evidence] - evidenceRank[right.evidence];
  if (evidenceDifference) return evidenceDifference;
  const kindDifference = compareText(normalizedToken(left.kind), normalizedToken(right.kind));
  if (kindDifference) return kindDifference;
  const leftRelated = nodes.get(relatedNodeId(left, currentId))?.label ?? relatedNodeId(left, currentId);
  const rightRelated = nodes.get(relatedNodeId(right, currentId))?.label ?? relatedNodeId(right, currentId);
  return compareText(leftRelated, rightRelated) || compareText(left.id, right.id);
}

export interface GroupedRelations {
  incoming: GraphEdge[];
  outgoing: GraphEdge[];
}

export function groupNodeRelations(
  edges: readonly GraphEdge[],
  currentId: string,
  nodes: ReadonlyMap<string, GraphNode>,
): GroupedRelations {
  const outgoing = edges
    .filter((edge) => edge.source === currentId)
    .sort((left, right) => compareRelations(left, right, currentId, nodes));
  const incoming = edges
    .filter((edge) => edge.source !== currentId && edge.target === currentId)
    .sort((left, right) => compareRelations(left, right, currentId, nodes));
  return { incoming, outgoing };
}
