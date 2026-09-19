import type { GraphEdge, GraphLens, GraphNode, GraphSnapshot } from "../../types";

export type GraphLayoutMode = "flow" | "map";
export type SystemLayerId =
  | "configuration"
  | "entry"
  | "interface"
  | "application"
  | "data"
  | "external";

export interface SystemLayer {
  id: SystemLayerId;
  label: string;
  description: string;
}

export const SYSTEM_LAYERS: readonly SystemLayer[] = [
  { id: "configuration", label: "Configuration", description: "Safe manifests and service declarations" },
  { id: "entry", label: "Entry points", description: "Main, server, command, and application roots" },
  { id: "interface", label: "Interfaces & jobs", description: "APIs, webhooks, events, workers, and schedules" },
  { id: "application", label: "Application", description: "Services, functions, and domain behavior" },
  { id: "data", label: "Data", description: "Repositories, models, databases, and storage" },
  { id: "external", label: "External", description: "Outbound requests · target unverified" },
];

const validLayerIds = new Set<string>(SYSTEM_LAYERS.map((layer) => layer.id));
const CONFIGURATION_KINDS = new Set([
  "config", "configuration", "configfile", "manifest", "serviceconfiguration",
]);
const ENTRY_KINDS = new Set(["entry", "entrypoint", "main", "command", "cli"]);
const INTERFACE_KINDS = new Set([
  "api", "endpoint", "route", "router", "controller", "handler", "event", "listener",
  "subscription", "publication", "webhook", "job", "cron", "cronjob", "batch", "worker",
  "scheduler", "queue", "topic", "functionapp",
]);
const DATA_KINDS = new Set([
  "repository", "model", "entity", "schema", "table", "database", "collection", "document",
  "heading", "sentence", "record", "store", "cache", "persistence", "dao",
]);
const EXTERNAL_KINDS = new Set([
  "externalapi", "externalservice", "cloudservice", "thirdparty", "vendor", "dependency",
]);
const SENSITIVE_KINDS = new Set([
  "credential", "credentials", "secret", "environmentvariable", "envvar",
]);
const SAFE_CONFIG_BASENAMES = new Set([
  "cargo.toml", "compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml",
  "package.json", "pyproject.toml", "tauri.conf.json", "tsconfig.json",
]);

function normalizedToken(value: string): string {
  return value.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop()?.toLowerCase() ?? "";
}

function envLikeName(value: string): boolean {
  const name = basename(value.trim());
  return name === ".env" || name.startsWith(".env.") || name.endsWith(".env");
}

export function isSensitiveSystemNode(node: Pick<GraphNode, "kind" | "label" | "source">): boolean {
  return SENSITIVE_KINDS.has(normalizedToken(node.kind))
    || envLikeName(node.label)
    || (node.source ? envLikeName(node.source.relativePath) : false);
}

function isSystemOverviewNoise(node: Pick<GraphNode, "kind" | "source" | "metadata" | "evidence">): boolean {
  const kind = normalizedToken(node.kind);
  const legacyLayer = typeof node.metadata.layer === "string" ? normalizedToken(node.metadata.layer) : "";
  if (kind === "runtimeevent" || kind === "runtimespan") {
    const eventKind = typeof node.metadata.eventKind === "string" ? normalizedToken(node.metadata.eventKind) : "";
    const traceProtocol = node.metadata.traceProtocol === "AONE_TRACE_V1";
    const acceptedTraceSpan = traceProtocol && /^trace(httpserver|httpclient|function|database|external|agent|event|job)(start|end|event)$/.test(eventKind);
    const boundaryOrSpan = eventKind === "processstarted" || (traceProtocol
      ? acceptedTraceSpan
      : eventKind.startsWith("http") || eventKind.startsWith("websocket") || eventKind.includes("span"));
    return node.evidence !== "observed" || !boundaryOrSpan;
  }
  if (["calltarget", "module", "risk", "analysis"].includes(kind)) return true;
  if (legacyLayer === "runtime" || legacyLayer === "analysis") return true;
  const path = node.source?.relativePath.replace(/\\/g, "/").toLowerCase();
  return Boolean(path && (
    /(^|\/)(__tests__|tests?|e2e)(\/|$)/.test(path)
    || /\.(test|spec|e2e)\.[^/]+$/.test(path)
    || /(?:^|\/)[^/]*(?:_live)?_tests\.rs$/.test(path)
  ));
}

function explicitLayer(node: Pick<GraphNode, "metadata">): SystemLayerId | undefined {
  const value = node.metadata.systemLayer;
  return typeof value === "string" && validLayerIds.has(value)
    ? value as SystemLayerId
    : undefined;
}

function legacyDeclaredLayer(node: Pick<GraphNode, "metadata">): SystemLayerId | undefined {
  const value = typeof node.metadata.layer === "string" ? normalizedToken(node.metadata.layer) : "";
  const mapping: Record<string, SystemLayerId> = {
    configuration: "configuration", config: "configuration", entry: "entry", entrypoint: "entry",
    web: "interface", ui: "interface", api: "interface", interface: "interface",
    domain: "application", application: "application", data: "data", external: "external",
  };
  return mapping[value];
}

export function systemLayerForNode(node: Pick<GraphNode, "kind" | "label" | "source" | "metadata">): SystemLayerId {
  const declared = explicitLayer(node) ?? legacyDeclaredLayer(node);
  if (declared) return declared;

  const kind = normalizedToken(node.kind);
  if (CONFIGURATION_KINDS.has(kind)) return "configuration";
  if (ENTRY_KINDS.has(kind)) return "entry";
  if (INTERFACE_KINDS.has(kind)) return "interface";
  if (DATA_KINDS.has(kind)) return "data";
  if (EXTERNAL_KINDS.has(kind)) return "external";

  const name = node.source ? basename(node.source.relativePath) : basename(node.label);
  if (SAFE_CONFIG_BASENAMES.has(name) || /^(vite|webpack|rollup|eslint|prettier)\.config\./.test(name)) {
    return "configuration";
  }
  if (/^(main|server|cli|command|bootstrap)\.[a-z0-9]+$/.test(name)) return "entry";
  return "application";
}

function normalizedGraphKind(kind: string): string {
  return kind.replace(/[-_]/g, "").toLowerCase();
}

export function graphForLens(graph: GraphSnapshot, lens: GraphLens): GraphSnapshot {
  if (lens === "all") return graph;
  if (lens === "system") {
    const nodes = graph.nodes.filter((node) => !isSensitiveSystemNode(node) && !isSystemOverviewNoise(node));
    const nodeIds = new Set(nodes.map((node) => node.id));
    return {
      ...graph,
      nodes,
      edges: graph.edges.filter((edge) => (
        nodeIds.has(edge.source)
        && nodeIds.has(edge.target)
        && !(edge.evidence === "inferred" && normalizedGraphKind(edge.kind) === "resolvesto")
      )),
    };
  }

  let edges: GraphEdge[];
  if (lens === "architecture") {
    edges = graph.edges.filter((edge) => edge.evidence === "declared" || edge.evidence === "resolved");
  } else if (lens === "runtime") {
    const observedIds = new Set(graph.nodes.filter((node) => node.evidence === "observed").map((node) => node.id));
    edges = graph.edges.filter((edge) => edge.evidence === "observed" || observedIds.has(edge.source) || observedIds.has(edge.target));
    const nodeIds = new Set(edges.flatMap((edge) => [edge.source, edge.target]));
    for (const id of observedIds) nodeIds.add(id);
    return { ...graph, nodes: graph.nodes.filter((node) => nodeIds.has(node.id)), edges };
  } else if (lens === "dependencies") {
    const kinds = new Set(["calls", "imports", "dependson", "writes", "reads", "resolvesto"]);
    edges = graph.edges.filter((edge) => kinds.has(normalizedGraphKind(edge.kind)));
  } else {
    const kinds = new Set(["api", "calltarget", "database", "endpoint", "event", "externalapi", "function", "model", "repository", "service", "table"]);
    const ids = new Set(graph.nodes.filter((node) => kinds.has(normalizedGraphKind(node.kind))).map((node) => node.id));
    edges = graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target));
    for (const node of graph.nodes) if (kinds.has(normalizedGraphKind(node.kind))) ids.add(node.id);
    return { ...graph, nodes: graph.nodes.filter((node) => ids.has(node.id)), edges };
  }
  const ids = new Set(edges.flatMap((edge) => [edge.source, edge.target]));
  return { ...graph, nodes: graph.nodes.filter((node) => ids.has(node.id)), edges };
}

export function relatedNodeIds(edges: readonly GraphEdge[], selectedNodeId: string | null): Set<string> {
  if (!selectedNodeId) return new Set();
  const ids = new Set([selectedNodeId]);
  for (const edge of edges) {
    if (edge.source === selectedNodeId) ids.add(edge.target);
    if (edge.target === selectedNodeId) ids.add(edge.source);
  }
  return ids;
}
