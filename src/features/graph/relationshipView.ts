import type { GraphEdge, GraphNode, GraphSnapshot } from "../../types";
import type { GitFileStatus } from "../source-control/model";
import { flowStageForNode } from "./flowModel";

export type RelationshipView = "workspace" | "api" | "logic" | "data" | "codepath" | "changes";

export const RELATIONSHIP_VIEWS: readonly { id: RelationshipView; label: string }[] = [
  { id: "workspace", label: "All" },
  { id: "api", label: "API" },
  { id: "logic", label: "Logic" },
  { id: "data", label: "Data" },
  { id: "codepath", label: "Code path" },
  { id: "changes", label: "Changes" },
];

function graphFromNodeIds(graph: GraphSnapshot, ids: Set<string>): GraphSnapshot {
  return {
    ...graph,
    nodes: graph.nodes.filter((node) => ids.has(node.id)),
    edges: graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target)),
  };
}

function sourcePath(node: GraphNode): string | undefined {
  return node.source?.relativePath;
}

type GitChangeKind = "insert" | "delete" | "conflict" | "modify";

function gitChangeKind(file: GitFileStatus): GitChangeKind {
  if (file.conflicted) return "conflict";
  if (file.indexStatus === "?" || file.workingTreeStatus === "?" || file.indexStatus === "A") {
    return "insert";
  }
  if (file.indexStatus === "D" || file.workingTreeStatus === "D") return "delete";
  return "modify";
}

function gitChangeLabel(kind: GitChangeKind): string {
  if (kind === "insert") return "Inserted";
  if (kind === "delete") return "Deleted";
  if (kind === "conflict") return "Conflict";
  return "Modified";
}

function changedGraph(
  workspaceGraph: GraphSnapshot,
  changedPaths: ReadonlySet<string>,
  changedFiles: readonly GitFileStatus[],
  focusedNodeId?: string | null,
): GraphSnapshot {
  const statusByPath = new Map(changedFiles.map((file) => [file.relativePath, file]));
  const pathSourceNodes = workspaceGraph.nodes.filter((node) => changedPaths.has(sourcePath(node) ?? ""));
  const focusedNode = focusedNodeId
    ? pathSourceNodes.find((node) => node.id === focusedNodeId)
    : undefined;
  const changedSourceNodes = focusedNode ? [focusedNode] : pathSourceNodes;
  const changedSourceIds = new Set(changedSourceNodes.map((node) => node.id));
  const ancestryIds = upstreamNodeIds(
    workspaceGraph,
    new Set(changedSourceNodes.map((node) => node.id)),
  );
  const sourceNodes = workspaceGraph.nodes.filter((node) => ancestryIds.has(node.id));
  const sourceIds = new Set(sourceNodes.map((node) => node.id));
  const sourceEdges = workspaceGraph.edges.filter(
    (edge) => sourceIds.has(edge.source) && sourceIds.has(edge.target),
  );
  const markerNodes: GraphNode[] = [];
  const markerEdges: GraphEdge[] = [];

  for (const relativePath of [...changedPaths].sort()) {
    const file = statusByPath.get(relativePath);
    const kind = file ? gitChangeKind(file) : "modify";
    const markerId = `git-change:${relativePath}`;
    markerNodes.push({
      id: markerId,
      kind: `git-${kind}`,
      label: `${gitChangeLabel(kind)} ${relativePath.split("/").at(-1) ?? relativePath}`,
      source: { relativePath, startLine: 1, startColumn: 1, endLine: 2, endColumn: 1 },
      evidence: "resolved",
      metadata: {
        flowStage: "trigger",
        gitChanged: true,
        gitChangeMarker: true,
        gitChangeKind: kind,
      },
    });
    for (const node of changedSourceNodes) {
      if (sourcePath(node) !== relativePath) continue;
      markerEdges.push({
        id: `${markerId}:${node.id}`,
        source: node.id,
        target: markerId,
        kind: "changedIn",
        evidence: "resolved" as const,
        metadata: { gitChanged: true, gitChangeKind: kind },
      });
    }
  }

  return {
    ...workspaceGraph,
    nodes: [
      ...markerNodes,
      ...sourceNodes.map((node) => {
        if (!changedSourceIds.has(node.id)) return node;
        const relativePath = sourcePath(node) ?? "";
        const file = statusByPath.get(relativePath);
        const kind = file ? gitChangeKind(file) : "modify";
        return { ...node, metadata: { ...node.metadata, gitChanged: true, gitChangeKind: kind } };
      }),
    ],
    edges: [...sourceEdges, ...markerEdges],
  };
}

function upstreamNodeIds(graph: GraphSnapshot, roots: Set<string>): Set<string> {
  const result = new Set(roots);
  let frontier = new Set(roots);
  while (frontier.size > 0) {
    const next = new Set<string>();
    for (const edge of graph.edges) {
      if (frontier.has(edge.target) && !result.has(edge.source)) next.add(edge.source);
    }
    for (const id of next) result.add(id);
    frontier = next;
  }
  return result;
}

function directedReachable(graph: GraphSnapshot, roots: Set<string>, depth: number): Set<string> {
  const result = new Set(roots);
  let frontier = new Set(roots);
  for (let level = 0; level < depth && frontier.size > 0; level += 1) {
    const next = new Set<string>();
    for (const edge of graph.edges) {
      if (frontier.has(edge.source) && !result.has(edge.target)) next.add(edge.target);
    }
    for (const id of next) result.add(id);
    frontier = next;
  }
  return result;
}

function withNeighbors(graph: GraphSnapshot, roots: Set<string>): Set<string> {
  const result = new Set(roots);
  for (const edge of graph.edges) {
    if (roots.has(edge.source)) result.add(edge.target);
    if (roots.has(edge.target)) result.add(edge.source);
  }
  return result;
}

export function relationshipGraphForView(
  workspaceGraph: GraphSnapshot,
  codePathGraph: GraphSnapshot,
  view: RelationshipView,
  changedPaths: ReadonlySet<string>,
  changedFiles: readonly GitFileStatus[] = [],
  focusedChangedNodeId?: string | null,
): GraphSnapshot {
  if (view === "workspace") return workspaceGraph;
  if (view === "codepath") return codePathGraph;
  if (view === "changes") {
    return changedGraph(workspaceGraph, changedPaths, changedFiles, focusedChangedNodeId);
  }

  const matching = new Set(workspaceGraph.nodes.filter((node) => {
    const stage = flowStageForNode(node);
    if (view === "api") return stage === "trigger" || stage === "api";
    if (view === "logic") return stage === "backend" || stage === "service";
    return stage === "data";
  }).map((node) => node.id));
  const ids = view === "api"
    ? directedReachable(workspaceGraph, matching, 4)
    : withNeighbors(workspaceGraph, matching);
  return graphFromNodeIds(workspaceGraph, ids);
}
