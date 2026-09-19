import { demoGraph, demoSources } from "../data/demo";
import type {
  SearchMatch,
  SearchMatchKind,
  SearchWorkspaceRequest,
  SearchWorkspaceResult,
} from "../features/search/model";

function desktopRuntime() {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

function graphKind(kind: string): SearchMatchKind {
  if (kind === "endpoint") return "endpoint";
  if (kind.includes("event") || kind.includes("listener")) return "event";
  if (kind === "heading") return "heading";
  if (kind === "sentence") return "sentence";
  return "symbol";
}

function boundedDemoMatches(request: SearchWorkspaceRequest): SearchWorkspaceResult {
  const query = request.query.trim();
  const needle = query.toLocaleLowerCase();
  const matches: SearchMatch[] = [];
  const seen = new Set<string>();
  const add = (match: SearchMatch) => {
    if (seen.has(match.key)) return;
    seen.add(match.key);
    matches.push(match);
  };

  for (const [relativePath, source] of Object.entries(demoSources)) {
    const pathOffset = relativePath.toLocaleLowerCase().indexOf(needle);
    if (pathOffset >= 0) {
      add({
        key: `path:${relativePath}`,
        relativePath,
        startLine: 1,
        startColumn: 1,
        endLine: 1,
        endColumn: 1,
        preview: relativePath,
        matchText: relativePath.slice(pathOffset, pathOffset + query.length),
        kind: "path",
      });
    }
    source.content.split(/\r?\n/).forEach((line, lineIndex) => {
      let offset = line.toLocaleLowerCase().indexOf(needle);
      while (offset >= 0) {
        const matchText = line.slice(offset, offset + query.length);
        add({
          key: `content:${relativePath}:${lineIndex + 1}:${offset + 1}`,
          relativePath,
          startLine: lineIndex + 1,
          startColumn: offset + 1,
          endLine: lineIndex + 1,
          endColumn: offset + query.length + 1,
          preview: line.trim() || line,
          matchText,
          kind: "content",
        });
        offset = line.toLocaleLowerCase().indexOf(needle, offset + Math.max(query.length, 1));
      }
    });
  }

  for (const node of demoGraph.nodes) {
    if (!node.source) continue;
    const offset = node.label.toLocaleLowerCase().indexOf(needle);
    if (offset < 0) continue;
    add({
      key: `node:${node.id}`,
      ...node.source,
      preview: node.label,
      matchText: node.label.slice(offset, offset + query.length),
      kind: graphKind(node.kind),
    });
  }

  const limit = Math.max(1, request.limit);
  return {
    query,
    matches: matches.slice(0, limit),
    totalMatches: matches.length,
    truncated: matches.length > limit,
    indexedFileCount: Object.keys(demoSources).length,
  };
}

export async function searchWorkspace(
  request: SearchWorkspaceRequest,
): Promise<SearchWorkspaceResult> {
  if (desktopRuntime()) return invokeDesktop("search_workspace", { request });
  await new Promise((resolve) => window.setTimeout(resolve, 75));
  return boundedDemoMatches(request);
}
