import type { ApiEndpointInventoryItem } from "../api-catalog/model";
import type { EvidenceKind, GraphEdge, GraphNode, GraphSnapshot, SourceLocation } from "../../types";

export interface KnowledgeCallNode {
  id: string;
  label: string;
  kind: string;
  evidence: EvidenceKind;
  relation?: string;
  source?: SourceLocation;
  children: KnowledgeCallNode[];
  cycle?: boolean;
  truncated?: boolean;
}

const MAX_CALL_DEPTH = 12;
const MAX_CALL_NODES = 180;
const MAX_CHILDREN = 32;
const FLOW_RELATIONS = new Set([
  "calls", "invokes", "handles", "dispatches", "publishes", "subscribes",
  "reads", "writes", "queries", "mapsTo", "resolvesTo", "uses", "imports",
  "dependsOn", "returns", "produces", "consumes", "parentSpan", "observedAt",
  "correlatesTo",
]);

function compareText(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });
}

function relationIsUseful(edge: GraphEdge): boolean {
  if (FLOW_RELATIONS.has(edge.kind)) return true;
  const normalized = edge.kind.replace(/[^a-z]/gi, "").toLowerCase();
  return [
    "call", "invoke", "handle", "read", "write", "query", "map", "resolve",
    "use", "import", "depend", "publish", "subscribe", "return", "consume",
  ].some((token) => normalized.includes(token));
}

function endpointRoot(endpoint: ApiEndpointInventoryItem, graph: GraphSnapshot): GraphNode | undefined {
  const exact = graph.nodes.find((node) => node.id === endpoint.id)
    ?? graph.nodes.find((node) => node.id === endpoint.handler?.id);
  if (exact) return exact;
  const source = endpoint.source ?? endpoint.handler?.source;
  if (source) {
    const atSource = graph.nodes.filter((node) => node.source?.relativePath === source.relativePath);
    const containing = atSource.find((node) => {
      if (!node.source) return false;
      return node.source.startLine <= source.startLine && node.source.endLine >= source.startLine;
    });
    if (containing) return containing;
  }
  const labels = new Set([
    endpoint.label.toLowerCase(),
    endpoint.handler?.label.toLowerCase(),
    endpoint.operationId?.toLowerCase(),
  ].filter((value): value is string => Boolean(value)));
  return graph.nodes.find((node) => labels.has(node.label.toLowerCase()));
}

function fallbackTree(endpoint: ApiEndpointInventoryItem): KnowledgeCallNode {
  const source = endpoint.source ?? endpoint.handler?.source;
  const handler = endpoint.handler;
  return {
    id: endpoint.id,
    label: `${endpoint.method || "ANY"} ${endpoint.path || endpoint.label}`,
    kind: "api",
    evidence: endpoint.evidence,
    source,
    children: handler ? [{
      id: handler.id,
      label: handler.label,
      kind: handler.kind,
      evidence: handler.evidence,
      relation: "handles",
      source: handler.source,
      children: [],
    }] : [],
  };
}

export function buildEndpointCallTree(
  endpoint: ApiEndpointInventoryItem,
  graph: GraphSnapshot,
): KnowledgeCallNode {
  const root = endpointRoot(endpoint, graph);
  if (!root) return fallbackTree(endpoint);
  const nodes = new Map(graph.nodes.map((node) => [node.id, node]));
  const outgoing = new Map<string, GraphEdge[]>();
  for (const edge of graph.edges) {
    if (!relationIsUseful(edge) || !nodes.has(edge.source) || !nodes.has(edge.target)) continue;
    outgoing.set(edge.source, [...(outgoing.get(edge.source) ?? []), edge]);
  }
  for (const edges of outgoing.values()) {
    edges.sort((left, right) => compareText(left.kind, right.kind)
      || compareText(nodes.get(left.target)?.label ?? left.target, nodes.get(right.target)?.label ?? right.target)
      || compareText(left.id, right.id));
  }

  let retained = 0;
  const visit = (
    node: GraphNode,
    relation: string | undefined,
    depth: number,
    ancestors: ReadonlySet<string>,
  ): KnowledgeCallNode => {
    retained += 1;
    const cycle = ancestors.has(node.id);
    const atLimit = depth >= MAX_CALL_DEPTH || retained >= MAX_CALL_NODES;
    if (cycle || atLimit) {
      return {
        id: node.id,
        label: node.label,
        kind: node.kind,
        evidence: node.evidence,
        relation,
        source: node.source,
        children: [],
        cycle,
        truncated: atLimit,
      };
    }
    const nextAncestors = new Set(ancestors);
    nextAncestors.add(node.id);
    const edges = outgoing.get(node.id) ?? [];
    const visible = edges.slice(0, MAX_CHILDREN);
    const children = visible.map((edge) => visit(nodes.get(edge.target)!, edge.kind, depth + 1, nextAncestors));
    if (edges.length > visible.length) {
      children.push({
        id: `${node.id}:more`,
        label: `${edges.length - visible.length} additional relationships omitted`,
        kind: "summary",
        evidence: "inferred",
        children: [],
        truncated: true,
      });
    }
    return {
      id: node.id,
      label: node.label,
      kind: node.kind,
      evidence: node.evidence,
      relation,
      source: node.source,
      children,
    };
  };

  const mapped = visit(root, undefined, 0, new Set());
  return {
    ...mapped,
    label: `${endpoint.method || "ANY"} ${endpoint.path || endpoint.label}`,
    kind: "api",
    source: endpoint.source ?? mapped.source,
  };
}

export function flattenKnowledgeTree(root: KnowledgeCallNode): KnowledgeCallNode[] {
  const result: KnowledgeCallNode[] = [];
  const walk = (node: KnowledgeCallNode) => {
    result.push(node);
    node.children.forEach(walk);
  };
  walk(root);
  return result;
}
