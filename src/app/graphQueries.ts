import { GraphQueryProjectionEnum } from "../generated/ipc/models/GraphQuery";
import { queryGraph } from "../lib/bridge";
import type { GraphQuery, GraphSnapshot } from "../types";
import { EMPTY_GRAPH } from "./model";

export type ExecutionFlowSnapshot = GraphSnapshot & {
  clientPageCount?: number;
  clientPagingLimited?: boolean;
};

export const MAX_EXECUTION_FLOW_NODES = 1_000;
export const MAX_EXECUTION_FLOW_EDGES = 2_000;
export const MAX_EXECUTION_FLOW_PAGES = 10;

export const SYSTEM_OVERVIEW_GRAPH_QUERY: GraphQuery = {
  projection: GraphQueryProjectionEnum.SystemOverview,
  depth: 2,
  limit: 60,
};

const DETAIL_GRAPH_QUERY: GraphQuery = {
  projection: GraphQueryProjectionEnum.Neighborhood,
  depth: 4,
  limit: 500,
};

export function executionFlowQueryFor(rootId: string, cursor?: string): GraphQuery {
  return {
    projection: GraphQueryProjectionEnum.ExecutionFlow,
    rootIds: [rootId],
    depth: 4,
    limit: 500,
    ...(cursor ? { cursor } : {}),
    includeTests: false,
  };
}

export async function queryExecutionFlow(rootId: string, cursor?: string): Promise<ExecutionFlowSnapshot> {
  const page = await queryGraph(executionFlowQueryFor(rootId, cursor));
  const nodes = page.nodes.slice(0, MAX_EXECUTION_FLOW_NODES);
  const nodeIds = new Set(nodes.map((node) => node.id));
  const connectedEdges = page.edges.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target));
  const edges = connectedEdges.slice(0, MAX_EXECUTION_FLOW_EDGES);
  const omittedNodes = Math.max(0, page.nodes.length - nodes.length);
  const omittedEdges = Math.max(0, page.edges.length - edges.length);
  return {
    ...page,
    nodes,
    edges,
    clientPageCount: 1,
    clientPagingLimited: omittedNodes > 0 || omittedEdges > 0,
  };
}

export function detailGraphQueryFor(overview: GraphSnapshot): GraphQuery | null {
  const rootIds = overview.nodes
    .map((node) => node.id)
    .sort()
    .slice(0, 50);
  return rootIds.length > 0 ? { ...DETAIL_GRAPH_QUERY, rootIds } : null;
}

function defaultFlowRootId(system: GraphSnapshot): string | undefined {
  const ranked = [...system.nodes].sort((left, right) => {
    const score = (node: typeof left) => Number(node.metadata.flowRoot === true) * 100
      + Number(node.kind === "endpoint") * 50
      + Number(["entry", "entrypoint", "main", "handler"].includes(node.kind)) * 25;
    return score(right) - score(left) || left.id.localeCompare(right.id);
  });
  return ranked[0]?.id;
}

export function mergeExecutionFlow(
  current: ExecutionFlowSnapshot,
  page: ExecutionFlowSnapshot,
): ExecutionFlowSnapshot {
  const nodes = new Map(current.nodes.map((node) => [node.id, node]));
  for (const node of page.nodes) {
    const previous = nodes.get(node.id);
    nodes.set(node.id, previous ? { ...previous, ...node, metadata: { ...previous.metadata, ...node.metadata } } : node);
  }
  const allNodes = [...nodes.values()];
  const boundedNodes = allNodes.slice(0, MAX_EXECUTION_FLOW_NODES);
  const nodeIds = new Set(boundedNodes.map((node) => node.id));
  const edges = new Map(current.edges.map((edge) => [edge.id, edge]));
  for (const edge of page.edges) edges.set(edge.id, edge);
  const allEdges = [...edges.values()];
  const boundedEdges = allEdges
    .filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target))
    .slice(0, MAX_EXECUTION_FLOW_EDGES);
  const pageCount = Math.min(MAX_EXECUTION_FLOW_PAGES, (current.clientPageCount ?? 1) + 1);
  const omittedNodes = allNodes.length > boundedNodes.length;
  const omittedEdges = allEdges.length > boundedEdges.length;
  const pageLimitReached = pageCount >= MAX_EXECUTION_FLOW_PAGES && Boolean(page.nextCursor);
  return {
    ...current,
    ...page,
    nodes: boundedNodes,
    edges: boundedEdges,
    clientPageCount: pageCount,
    clientPagingLimited: Boolean(
      current.clientPagingLimited
      || page.clientPagingLimited
      || omittedNodes
      || omittedEdges
      || pageLimitReached
    ),
  };
}

export async function loadGraphSnapshots(): Promise<[GraphSnapshot, GraphSnapshot, ExecutionFlowSnapshot]> {
  const system = await queryGraph(SYSTEM_OVERVIEW_GRAPH_QUERY);
  const detailQuery = detailGraphQueryFor(system);
  const rootId = defaultFlowRootId(system);
  const [detailResult, flowResult] = await Promise.allSettled([
    detailQuery ? queryGraph(detailQuery) : Promise.resolve(EMPTY_GRAPH),
    rootId ? queryExecutionFlow(rootId) : Promise.resolve(EMPTY_GRAPH),
  ]);
  const detail = detailResult.status === "fulfilled" ? detailResult.value : EMPTY_GRAPH;
  const flow = flowResult.status === "fulfilled" ? flowResult.value : EMPTY_GRAPH;
  return [system, detail, flow];
}
