import type { GraphEdge, GraphNode, GraphSnapshot } from "../../types";
import {
  FLOW_STAGES,
  flowGroupForNode,
  flowStageForNode,
  observedFlowNodeIds,
  traceScopeNodeIds,
  type FlowStageId,
} from "./flowModel";

export const FLOW_VIEW_WIDTH = 900;
export const FLOW_VIEW_HEIGHT = 520;
export const FLOW_CARD_WIDTH = 142;
export const FLOW_CARD_HEIGHT = 34;
export const FLOW_LEFT = 18;
const TOP = 58;
const STAGE_HEADER = 22;
const STAGE_GAP = 8;
const GROUP_HEADER = 22;
const GROUP_PADDING = 7;
const GROUP_GAP = 10;
const CARD_GAP = 8;
const COLLAPSED_NODES = 3;
const EXPANDED_NODES = 18;
const MAX_GROUPS_PER_STAGE = 7;
export const MAX_RENDERED_FLOW_EDGES = 600;

export type FlowLayoutMode = "trace" | "overview";

export interface PositionedFlowNode {
  node: GraphNode;
  stage: FlowStageId;
  groupKey: string;
  x: number;
  y: number;
  width: number;
  height: number;
  observed: boolean;
}

export interface PositionedFlowGroup {
  key: string;
  label: string;
  stage: FlowStageId;
  x: number;
  y: number;
  width: number;
  height: number;
  truthCount: number;
  hiddenCount: number;
  observedCount: number;
  expanded: boolean;
}

export interface PositionedFlowStage {
  id: FlowStageId;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ExecutionFlowLayout {
  nodes: PositionedFlowNode[];
  groups: PositionedFlowGroup[];
  stages: PositionedFlowStage[];
  edges: GraphEdge[];
  width: number;
  height: number;
  rootNodeId: string | null;
  scopeNodeCount: number;
  scopeEdgeCount: number;
  hiddenNodeCount: number;
  hiddenGroupCount: number;
  omittedVisibleEdgeCount: number;
  truthNodeCount: number;
  truthEdgeCount: number;
  stageTruthCounts: Record<FlowStageId, number>;
  stageVisibleCounts: Record<FlowStageId, number>;
}

interface GroupBucket {
  key: string;
  label: string;
  stage: FlowStageId;
  nodes: GraphNode[];
  observedCount: number;
  score: number;
}

export interface ExecutionFlowLayoutOptions {
  rootNodeId: string | null;
  expandedGroups: ReadonlySet<string>;
  mode: FlowLayoutMode;
}

function emptyCounts(): Record<FlowStageId, number> {
  return { trigger: 0, api: 0, backend: 0, service: 0, data: 0, external: 0 };
}

function compareText(left: string, right: string): number {
  return left === right ? 0 : left < right ? -1 : 1;
}

function incidentCounts(edges: readonly GraphEdge[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const edge of edges) {
    counts.set(edge.source, (counts.get(edge.source) ?? 0) + 1);
    counts.set(edge.target, (counts.get(edge.target) ?? 0) + 1);
  }
  return counts;
}

function groupBuckets(
  nodes: readonly GraphNode[],
  edges: readonly GraphEdge[],
  rootNodeId: string | null,
): GroupBucket[] {
  const observedIds = observedFlowNodeIds({ nodes: [...nodes], edges: [...edges], truncated: false });
  const degree = incidentCounts(edges);
  const buckets = new Map<string, GroupBucket>();
  for (const node of nodes) {
    const stage = flowStageForNode(node);
    const label = flowGroupForNode(node);
    const key = `${stage}:${label.toLowerCase()}`;
    const bucket = buckets.get(key) ?? { key, label, stage, nodes: [], observedCount: 0, score: 0 };
    if (compareText(label, bucket.label) < 0) bucket.label = label;
    bucket.nodes.push(node);
    if (observedIds.has(node.id)) bucket.observedCount += 1;
    bucket.score += degree.get(node.id) ?? 0;
    if (node.id === rootNodeId) bucket.score += 10_000;
    buckets.set(key, bucket);
  }
  for (const bucket of buckets.values()) {
    bucket.nodes.sort((left, right) => Number(right.id === rootNodeId) - Number(left.id === rootNodeId)
      || Number(observedIds.has(right.id)) - Number(observedIds.has(left.id))
      || (degree.get(right.id) ?? 0) - (degree.get(left.id) ?? 0)
      || compareText(left.label.toLowerCase(), right.label.toLowerCase())
      || compareText(left.id, right.id));
  }
  return [...buckets.values()];
}

function visibleBucketsForStage(
  buckets: readonly GroupBucket[],
  stage: FlowStageId,
  rootNodeId: string | null,
): GroupBucket[] {
  const stageBuckets = buckets
    .filter((bucket) => bucket.stage === stage)
    .sort((left, right) => right.score - left.score
      || right.observedCount - left.observedCount
      || compareText(left.label.toLowerCase(), right.label.toLowerCase())
      || compareText(left.key, right.key));
  const visible = stageBuckets.slice(0, MAX_GROUPS_PER_STAGE);
  const rootBucket = stageBuckets.find((bucket) => bucket.nodes.some((node) => node.id === rootNodeId));
  if (rootBucket && !visible.includes(rootBucket)) visible[visible.length - 1] = rootBucket;
  return visible;
}

function visibleNodesForBucket(
  bucket: GroupBucket,
  expanded: boolean,
  rootNodeId: string | null,
): GraphNode[] {
  const limit = expanded ? EXPANDED_NODES : COLLAPSED_NODES;
  const visible = bucket.nodes.slice(0, limit);
  const root = bucket.nodes.find((node) => node.id === rootNodeId);
  if (root && !visible.includes(root)) visible[Math.max(0, visible.length - 1)] = root;
  return visible;
}

function boundedVisibleEdges(
  edges: readonly GraphEdge[],
  visibleIds: ReadonlySet<string>,
  rootNodeId: string | null,
): { edges: GraphEdge[]; omitted: number } {
  const connected = edges.filter((edge) => visibleIds.has(edge.source) && visibleIds.has(edge.target));
  const score = (edge: GraphEdge) => (
    Number(Boolean(rootNodeId && (edge.source === rootNodeId || edge.target === rootNodeId))) * 10
    + Number(edge.evidence === "observed") * 6
    + Number(edge.evidence === "resolved") * 3
    + Number(edge.evidence === "declared") * 2
  );
  const ranked = [...connected].sort((left, right) => score(right) - score(left)
    || compareText(left.source, right.source)
    || compareText(left.target, right.target)
    || compareText(left.kind, right.kind)
    || compareText(left.id, right.id));
  return {
    edges: ranked.slice(0, MAX_RENDERED_FLOW_EDGES),
    omitted: Math.max(0, ranked.length - MAX_RENDERED_FLOW_EDGES),
  };
}

export function createExecutionFlowLayout(
  graph: GraphSnapshot,
  options: ExecutionFlowLayoutOptions,
): ExecutionFlowLayout {
  const scopeIds = options.mode === "trace"
    ? traceScopeNodeIds(graph, options.rootNodeId)
    : new Set(graph.nodes.map((node) => node.id));
  const scopeNodes = graph.nodes.filter((node) => scopeIds.has(node.id));
  const scopeEdges = graph.edges.filter((edge) => scopeIds.has(edge.source) && scopeIds.has(edge.target));
  const observedIds = observedFlowNodeIds(graph);
  const buckets = groupBuckets(scopeNodes, scopeEdges, options.rootNodeId);
  const stageTruthCounts = emptyCounts();
  const stageVisibleCounts = emptyCounts();
  for (const node of scopeNodes) stageTruthCounts[flowStageForNode(node)] += 1;

  const groups: PositionedFlowGroup[] = [];
  const nodes: PositionedFlowNode[] = [];
  const stageRows: Array<Omit<PositionedFlowStage, "width">> = [];
  let maxRight = FLOW_VIEW_WIDTH - FLOW_LEFT;
  let stageY = TOP;
  let hiddenGroupCount = 0;
  FLOW_STAGES.forEach((stage) => {
    const stageBuckets = buckets.filter((bucket) => bucket.stage === stage.id);
    const visibleBuckets = visibleBucketsForStage(buckets, stage.id, options.rootNodeId);
    if (stageBuckets.length === 0) return;
    hiddenGroupCount += Math.max(0, stageBuckets.length - visibleBuckets.length);
    let groupX = FLOW_LEFT + GROUP_PADDING;
    let tallestGroup = 0;
    for (const bucket of visibleBuckets) {
      const expanded = options.expandedGroups.has(bucket.key);
      const visibleNodes = visibleNodesForBucket(bucket, expanded, options.rootNodeId);
      const hiddenCount = bucket.nodes.length - visibleNodes.length;
      const footerHeight = hiddenCount > 0 || expanded ? 20 : 5;
      const width = GROUP_PADDING * 2 + visibleNodes.length * FLOW_CARD_WIDTH
        + Math.max(0, visibleNodes.length - 1) * CARD_GAP;
      const height = GROUP_HEADER + GROUP_PADDING + FLOW_CARD_HEIGHT + footerHeight;
      const groupY = stageY + STAGE_HEADER;
      groups.push({
        key: bucket.key,
        label: bucket.label,
        stage: bucket.stage,
        x: groupX,
        y: groupY,
        width,
        height,
        truthCount: bucket.nodes.length,
        hiddenCount,
        observedCount: bucket.observedCount,
        expanded,
      });
      visibleNodes.forEach((node, index) => nodes.push({
        node,
        stage: bucket.stage,
        groupKey: bucket.key,
        x: groupX + GROUP_PADDING + index * (FLOW_CARD_WIDTH + CARD_GAP),
        y: groupY + GROUP_HEADER + GROUP_PADDING,
        width: FLOW_CARD_WIDTH,
        height: FLOW_CARD_HEIGHT,
        observed: observedIds.has(node.id),
      }));
      stageVisibleCounts[stage.id] += visibleNodes.length;
      groupX += width + GROUP_GAP;
      tallestGroup = Math.max(tallestGroup, height);
    }
    const stageHeight = STAGE_HEADER + Math.max(tallestGroup, 30) + GROUP_PADDING;
    stageRows.push({ id: stage.id, x: FLOW_LEFT, y: stageY, height: stageHeight });
    maxRight = Math.max(maxRight, groupX - GROUP_GAP);
    stageY += stageHeight + STAGE_GAP;
  });
  const width = Math.max(FLOW_VIEW_WIDTH, maxRight + FLOW_LEFT);
  const stages = stageRows.map((stage) => ({ ...stage, width: width - FLOW_LEFT * 2 }));
  const visibleIds = new Set(nodes.map((node) => node.node.id));
  const visibleEdges = boundedVisibleEdges(scopeEdges, visibleIds, options.rootNodeId);
  return {
    nodes,
    groups,
    stages,
    edges: visibleEdges.edges,
    width,
    height: Math.max(FLOW_VIEW_HEIGHT, stageY - STAGE_GAP + FLOW_LEFT),
    rootNodeId: options.rootNodeId && scopeIds.has(options.rootNodeId) ? options.rootNodeId : null,
    scopeNodeCount: scopeNodes.length,
    scopeEdgeCount: scopeEdges.length,
    hiddenNodeCount: scopeNodes.length - nodes.length,
    hiddenGroupCount,
    omittedVisibleEdgeCount: visibleEdges.omitted,
    truthNodeCount: graph.nodes.length,
    truthEdgeCount: graph.edges.length,
    stageTruthCounts,
    stageVisibleCounts,
  };
}

export function executionFlowEdgePath(source: PositionedFlowNode, target: PositionedFlowNode): string {
  if (source.node.id === target.node.id) {
    const centerX = source.x + source.width / 2;
    const top = source.y;
    const bottom = source.y + source.height;
    const right = source.x + source.width + 28;
    return `M ${centerX} ${bottom} V ${bottom + 24} H ${right} V ${top - 18} H ${centerX} V ${top}`;
  }
  const startX = source.x + source.width / 2;
  const endX = target.x + target.width / 2;
  if (source.stage === target.stage) {
    const startY = source.y + source.height;
    const endY = target.y + target.height;
    const laneY = Math.max(startY, endY) + 24;
    return `M ${startX} ${startY} V ${laneY} H ${endX} V ${endY}`;
  }
  const forward = target.y > source.y;
  const startY = forward ? source.y + source.height : source.y;
  const endY = forward ? target.y : target.y + target.height;
  const middleY = startY + (endY - startY) / 2;
  return `M ${startX} ${startY} V ${middleY} H ${endX} V ${endY}`;
}

export function nextFlowNodeByGeometry(
  nodes: readonly PositionedFlowNode[],
  currentId: string,
  key: string,
): PositionedFlowNode | undefined {
  const current = nodes.find((candidate) => candidate.node.id === currentId);
  if (!current) return nodes[0];
  const ordered = [...nodes].sort((left, right) => left.y - right.y || left.x - right.x || compareText(left.node.id, right.node.id));
  if (key === "Home") return ordered[0];
  if (key === "End") return ordered[ordered.length - 1];
  const currentX = current.x + current.width / 2;
  const currentY = current.y + current.height / 2;
  const candidates = nodes.filter((candidate) => {
    if (candidate.node.id === currentId) return false;
    const x = candidate.x + candidate.width / 2;
    const y = candidate.y + candidate.height / 2;
    if (key === "ArrowLeft") return candidate.stage === current.stage && x < currentX;
    if (key === "ArrowRight") return candidate.stage === current.stage && x > currentX;
    if (key === "ArrowUp") return candidate.stage !== current.stage && y < currentY;
    return candidate.stage !== current.stage && y > currentY;
  });
  return candidates.sort((left, right) => {
    const vertical = key === "ArrowUp" || key === "ArrowDown";
    const leftPrimary = Math.abs((vertical ? left.y : left.x) - (vertical ? current.y : current.x));
    const rightPrimary = Math.abs((vertical ? right.y : right.x) - (vertical ? current.y : current.x));
    const leftSecondary = Math.abs((vertical ? left.x : left.y) - (vertical ? current.x : current.y));
    const rightSecondary = Math.abs((vertical ? right.x : right.y) - (vertical ? current.x : current.y));
    return leftPrimary - rightPrimary
      || leftSecondary - rightSecondary
      || compareText(left.node.id, right.node.id);
  })[0] ?? current;
}
