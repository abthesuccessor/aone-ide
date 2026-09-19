import type { GraphEdge, GraphNode, GraphSnapshot } from "../../types";

export type FlowStageId = "trigger" | "api" | "backend" | "service" | "data" | "external";

export interface FlowStage {
  id: FlowStageId;
  label: string;
  description: string;
}

export const FLOW_STAGES: readonly FlowStage[] = [
  { id: "trigger", label: "Configuration / Entry", description: "Configuration, UI triggers, entry files, schedules, and runtime events" },
  { id: "api", label: "API", description: "Routes, endpoints, webhooks, events, and subscriptions" },
  { id: "backend", label: "Handler", description: "Controllers, handlers, application roots, and orchestration" },
  { id: "service", label: "Service / Logic", description: "Methods, traits, services, workers, tools, and agents" },
  { id: "data", label: "Data", description: "Repositories, models, databases, queues, and storage" },
  { id: "external", label: "External", description: "Third-party and cloud boundaries; target identity may be unverified" },
];

const stageIds = new Set<string>(FLOW_STAGES.map((stage) => stage.id));
const FRONTEND_KINDS = new Set(["button", "client", "component", "frontend", "page", "screen", "ui", "view"]);
const API_KINDS = new Set([
  "api", "endpoint", "event", "functionapp", "graphql", "listener", "publication", "route",
  "subscription", "topic", "webhook",
]);
const BACKEND_KINDS = new Set([
  "application", "backend", "command", "controller", "entry", "entrypoint", "handler", "main",
  "orchestrator", "router", "server",
]);
const SERVICE_KINDS = new Set([
  "agent", "batch", "class", "cron", "cronjob", "function", "job", "method", "scheduler",
  "service", "tool", "trait", "utility", "worker",
]);
const DATA_KINDS = new Set([
  "cache", "collection", "dao", "database", "document", "entity", "heading", "model", "persistence",
  "queue", "record", "repository", "schema", "store", "table",
  "sentence",
]);
const EXTERNAL_KINDS = new Set([
  "cloudservice", "dependency", "externalapi", "externalservice", "sdk", "thirdparty", "vendor",
]);

const stageAliases: Record<string, FlowStageId> = {
  trigger: "trigger", frontend: "trigger", ui: "trigger", source: "trigger", configuration: "trigger",
  api: "api", interface: "api", event: "api", web: "api",
  backend: "backend", entry: "backend", entrypoint: "backend", controller: "backend",
  service: "service", agent: "service", application: "service", domain: "service", other: "service",
  data: "data", repository: "data", database: "data",
  external: "external", outbound: "external",
};

function normalizedToken(value: string): string {
  return value.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

function metadataString(node: Pick<GraphNode, "metadata">, key: string): string | undefined {
  const value = node.metadata[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function sourcePath(node: Pick<GraphNode, "source">): string {
  return node.source?.relativePath.replace(/\\/g, "/").toLowerCase() ?? "";
}

function explicitStage(node: Pick<GraphNode, "metadata">): FlowStageId | undefined {
  for (const key of ["flowStage", "traceStage", "systemLayer", "layer"]) {
    const value = metadataString(node, key);
    if (!value) continue;
    const normalized = normalizedToken(value);
    if (stageIds.has(normalized)) return normalized as FlowStageId;
    if (stageAliases[normalized]) return stageAliases[normalized];
  }
  return undefined;
}

function looksLikeFrontend(node: Pick<GraphNode, "kind" | "label" | "source">): boolean {
  const path = sourcePath(node);
  const kind = normalizedToken(node.kind);
  if (FRONTEND_KINDS.has(kind)) return true;
  if (/\.(jsx|tsx|vue|svelte)$/.test(path) && !/(server|backend|api|route)/.test(path)) return true;
  return /(^|\/)(frontend|client|ui|views?|pages?|screens?|components?|admin)(\/|$)/.test(path)
    || /\b(click|submit|refresh|button)\b/i.test(node.label);
}

export function flowStageForNode(
  node: Pick<GraphNode, "kind" | "label" | "source" | "metadata">,
): FlowStageId {
  const declared = explicitStage(node);
  if (declared) return declared;
  const kind = normalizedToken(node.kind);
  if (kind === "runtimeevent" || kind === "runtimespan") return "trigger";
  if (looksLikeFrontend(node)) return "trigger";
  if (API_KINDS.has(kind) || /^(get|post|put|patch|delete|options|head|connect|trace)\s+\//i.test(node.label)) return "api";
  if (DATA_KINDS.has(kind)) return "data";
  if (EXTERNAL_KINDS.has(kind)) return "external";
  if (BACKEND_KINDS.has(kind)) return "backend";
  if (SERVICE_KINDS.has(kind)) return "service";
  return "service";
}

function humanize(value: string): string {
  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[-_.]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^./, (character) => character.toUpperCase()) || "Other";
}

export function flowGroupForNode(node: Pick<GraphNode, "kind" | "source" | "metadata">): string {
  for (const key of ["flowGroup", "group", "module", "package", "serviceName", "namespace"]) {
    const value = metadataString(node, key);
    if (value) return humanize(value);
  }
  const path = node.source?.relativePath.replace(/\\/g, "/") ?? "";
  const parts = path.split("/").filter(Boolean);
  if (parts.length > 1) {
    const fileIndex = parts.length - 1;
    const root = parts[0] ?? "Source";
    const generic = new Set(["src", "lib", "app", "source", "packages"]);
    const scope = parts.slice(1, fileIndex).find((part) => !generic.has(part.toLowerCase()));
    return humanize(scope && root !== "src" ? `${root} / ${scope}` : (scope ?? root));
  }
  return humanize(node.kind);
}

function degreeByNode(edges: readonly GraphEdge[]): Map<string, number> {
  const degree = new Map<string, number>();
  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }
  return degree;
}

function compareText(left: string, right: string): number {
  return left === right ? 0 : left < right ? -1 : 1;
}

function entryScore(node: GraphNode, degree: ReadonlyMap<string, number>): number {
  const stage = flowStageForNode(node);
  const kind = normalizedToken(node.kind);
  let score = degree.get(node.id) ?? 0;
  if (stage === "api") score += 80;
  if (stage === "trigger") score += 55;
  if (kind === "runtimeevent") score += 120;
  if (node.evidence === "observed") score += 40;
  if (node.source) score += 4;
  return score;
}

export function flowEntryCandidates(graph: GraphSnapshot): GraphNode[] {
  const degree = degreeByNode(graph.edges);
  return [...graph.nodes]
    .filter((node) => {
      const stage = flowStageForNode(node);
      return stage === "trigger" || stage === "api" || normalizedToken(node.kind) === "runtimeevent";
    })
    .sort((left, right) => entryScore(right, degree) - entryScore(left, degree)
      || compareText(left.label.toLowerCase(), right.label.toLowerCase())
      || compareText(left.id, right.id));
}

export function traceScopeNodeIds(graph: GraphSnapshot, rootNodeId: string | null): Set<string> {
  if (!rootNodeId || !graph.nodes.some((node) => node.id === rootNodeId)) {
    return new Set(graph.nodes.map((node) => node.id));
  }
  const outgoing = new Map<string, string[]>();
  const incoming = new Map<string, string[]>();
  for (const edge of graph.edges) {
    outgoing.set(edge.source, [...(outgoing.get(edge.source) ?? []), edge.target]);
    incoming.set(edge.target, [...(incoming.get(edge.target) ?? []), edge.source]);
  }
  const visited = new Set([rootNodeId]);
  const walk = (links: ReadonlyMap<string, string[]>, maxDepth: number) => {
    const seen = new Set([rootNodeId]);
    const queue: Array<[string, number]> = [[rootNodeId, 0]];
    while (queue.length > 0) {
      const [nodeId, depth] = queue.shift()!;
      if (depth >= maxDepth) continue;
      for (const next of links.get(nodeId) ?? []) {
        if (seen.has(next)) continue;
        seen.add(next);
        visited.add(next);
        queue.push([next, depth + 1]);
      }
    }
  };
  walk(outgoing, 12);
  walk(incoming, 4);

  const runtimeNodes = new Map(graph.nodes
    .filter((node) => node.evidence === "observed" && ["runtimeevent", "runtimespan"].includes(normalizedToken(node.kind)))
    .map((node) => [node.id, node]));
  const attached = new Set<string>();
  for (const edge of graph.edges) {
    const sourceRuntime = runtimeNodes.has(edge.source);
    const targetRuntime = runtimeNodes.has(edge.target);
    if (sourceRuntime === targetRuntime) continue;
    const runtimeId = sourceRuntime ? edge.source : edge.target;
    const staticId = sourceRuntime ? edge.target : edge.source;
    if (visited.has(staticId)) attached.add(runtimeId);
  }
  const spanLinks = new Map<string, string[]>();
  for (const edge of graph.edges) {
    if (edge.evidence !== "observed" || normalizedToken(edge.kind) !== "parentspan") continue;
    if (!runtimeNodes.has(edge.source) || !runtimeNodes.has(edge.target)) continue;
    spanLinks.set(edge.source, [...(spanLinks.get(edge.source) ?? []), edge.target]);
    spanLinks.set(edge.target, [...(spanLinks.get(edge.target) ?? []), edge.source]);
  }
  const runtimeSeen = new Set<string>();
  const runtimeQueue = [...attached].sort(compareText);
  while (runtimeQueue.length > 0 && runtimeSeen.size < 80) {
    const runtimeId = runtimeQueue.shift()!;
    if (runtimeSeen.has(runtimeId)) continue;
    runtimeSeen.add(runtimeId);
    visited.add(runtimeId);
    for (const neighbor of [...(spanLinks.get(runtimeId) ?? [])].sort(compareText)) {
      if (!runtimeSeen.has(neighbor)) runtimeQueue.push(neighbor);
    }
  }
  return visited;
}

export function observedFlowNodeIds(graph: GraphSnapshot): Set<string> {
  return new Set(graph.nodes.filter((node) => node.evidence === "observed").map((node) => node.id));
}

export function latestObservedFlowNodeId(graph: GraphSnapshot): string | undefined {
  return graph.nodes
    .filter((node) => node.evidence === "observed" && typeof node.metadata.timestamp === "string")
    .sort((left, right) => {
      const timestampOrder = String(right.metadata.timestamp).localeCompare(String(left.metadata.timestamp));
      return timestampOrder || compareText(right.id, left.id);
    })[0]?.id;
}

export function searchFlowNodes(graph: GraphSnapshot, query: string, limit = 20): GraphNode[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return [];
  const degree = degreeByNode(graph.edges);
  return graph.nodes
    .filter((node) => {
      const source = node.source?.relativePath ?? "";
      return `${node.label}\n${node.kind}\n${source}\n${flowGroupForNode(node)}`.toLowerCase().includes(normalized);
    })
    .sort((left, right) => entryScore(right, degree) - entryScore(left, degree)
      || compareText(left.label.toLowerCase(), right.label.toLowerCase()))
    .slice(0, Math.max(1, limit));
}
