import type {
  WorkspaceSearchMatch,
  WorkspaceSearchMatchKind,
  WorkspaceSearchRequest,
  WorkspaceSearchResult,
} from "../../types";

export type SearchMatchKind = WorkspaceSearchMatchKind;
export type SearchWorkspaceRequest = WorkspaceSearchRequest;
export type SearchMatch = WorkspaceSearchMatch;
export type SearchWorkspaceResult = WorkspaceSearchResult;

export interface SearchResultGroup {
  relativePath: string;
  fileName: string;
  directory: string;
  matches: SearchMatch[];
}

function pathParts(relativePath: string) {
  const separator = relativePath.lastIndexOf("/");
  return separator < 0
    ? { fileName: relativePath, directory: "" }
    : {
        fileName: relativePath.slice(separator + 1),
        directory: relativePath.slice(0, separator),
      };
}

export function groupSearchMatches(matches: SearchMatch[]): SearchResultGroup[] {
  const groups = new Map<string, SearchMatch[]>();
  for (const match of matches) {
    const current = groups.get(match.relativePath);
    if (current) current.push(match);
    else groups.set(match.relativePath, [match]);
  }
  return Array.from(groups, ([relativePath, groupedMatches]) => ({
    relativePath,
    ...pathParts(relativePath),
    matches: groupedMatches,
  }));
}
