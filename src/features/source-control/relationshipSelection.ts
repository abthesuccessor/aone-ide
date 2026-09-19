import type { GraphNode } from "../../types";

const MAX_CHANGED_LINES = 512;

function normalizedPath(path: string): string {
  return path.replaceAll("\\", "/").replace(/^\.\//, "");
}

export function changedNewLinesFromPatch(patch: string): readonly number[] {
  const changed = new Set<number>();
  let newLine: number | null = null;

  for (const line of patch.split("\n")) {
    const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (hunk) {
      newLine = Number(hunk[1]);
      continue;
    }
    if (newLine === null || line.startsWith("\\")) continue;
    if (line.startsWith("+")) {
      if (!line.startsWith("+++")) changed.add(newLine);
      newLine += 1;
    } else if (!line.startsWith("-")) {
      newLine += 1;
    }
    if (changed.size >= MAX_CHANGED_LINES) break;
  }

  return [...changed].sort((left, right) => left - right);
}

function rangeEnd(node: GraphNode): number {
  const source = node.source;
  if (!source) return Number.MAX_SAFE_INTEGER;
  return Math.max(source.startLine, source.endLine - 1);
}

function rangeSpan(node: GraphNode): number {
  if (!node.source) return Number.MAX_SAFE_INTEGER;
  return rangeEnd(node) - node.source.startLine;
}

function containsChangedLine(node: GraphNode, changedLines: readonly number[]): boolean {
  if (!node.source) return false;
  const end = rangeEnd(node);
  return changedLines.some((line) => line >= node.source!.startLine && line <= end);
}

function compareNodes(left: GraphNode, right: GraphNode): number {
  const spanDifference = rangeSpan(left) - rangeSpan(right);
  if (spanDifference !== 0) return spanDifference;
  const lineDifference = (left.source?.startLine ?? Number.MAX_SAFE_INTEGER)
    - (right.source?.startLine ?? Number.MAX_SAFE_INTEGER);
  if (lineDifference !== 0) return lineDifference;
  return left.id.localeCompare(right.id);
}

function compareFallbackNodes(left: GraphNode, right: GraphNode): number {
  const lineDifference = (left.source?.startLine ?? Number.MAX_SAFE_INTEGER)
    - (right.source?.startLine ?? Number.MAX_SAFE_INTEGER);
  if (lineDifference !== 0) return lineDifference;
  return compareNodes(left, right);
}

export function relationshipNodeForGitPath(
  nodes: readonly GraphNode[],
  relativePath: string,
  patch = "",
): GraphNode | null {
  const targetPath = normalizedPath(relativePath);
  const candidates = nodes.filter(
    (node) => node.source && normalizedPath(node.source.relativePath) === targetPath,
  );
  if (candidates.length === 0) return null;

  const changedLines = changedNewLinesFromPatch(patch);
  const intersecting = changedLines.length > 0
    ? candidates.filter(
      (node) => node.metadata.gitChangeMarker !== true && containsChangedLine(node, changedLines),
    )
    : [];
  const marker = candidates.find((node) => node.metadata.gitChangeMarker === true);
  if (intersecting.length === 0 && marker) return marker;
  const comparable = intersecting.length > 0 ? intersecting : candidates;
  return [...comparable].sort(intersecting.length > 0 ? compareNodes : compareFallbackNodes)[0] ?? null;
}
