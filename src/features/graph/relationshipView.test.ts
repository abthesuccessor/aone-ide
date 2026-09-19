import { describe, expect, it } from "vitest";
import type { GraphNode, GraphSnapshot } from "../../types";
import { relationshipGraphForView } from "./relationshipView";

function node(id: string, stage: string, relativePath: string): GraphNode {
  return {
    id,
    kind: id,
    label: id,
    evidence: "resolved",
    metadata: { flowStage: stage },
    source: {
      relativePath,
      startLine: 1,
      startColumn: 1,
      endLine: 2,
      endColumn: 1,
    },
  };
}

const workspace: GraphSnapshot = {
  nodes: [
    node("api", "api", "src/api.ts"),
    node("handler", "backend", "src/api.ts"),
    node("service", "service", "src/service.ts"),
    node("repo", "data", "src/repo.ts"),
    node("external", "external", "src/client.ts"),
  ],
  edges: [
    { id: "1", source: "api", target: "handler", kind: "handledBy", evidence: "resolved", metadata: {} },
    { id: "2", source: "handler", target: "service", kind: "calls", evidence: "resolved", metadata: {} },
    { id: "3", source: "service", target: "repo", kind: "calls", evidence: "resolved", metadata: {} },
    { id: "4", source: "service", target: "external", kind: "requests", evidence: "resolved", metadata: {} },
  ],
  truncated: false,
};

describe("relationship category projection", () => {
  it("keeps a directed API path through handler, service, repository, and external calls", () => {
    const result = relationshipGraphForView(workspace, workspace, "api", new Set());
    expect(result.nodes.map((item) => item.id).sort()).toEqual([
      "api", "external", "handler", "repo", "service",
    ]);
    expect(result.edges).toHaveLength(4);
  });

  it("shows a changed node with its complete upstream API ancestry", () => {
    const result = relationshipGraphForView(workspace, workspace, "changes", new Set(["src/service.ts"]));
    expect(result.nodes.map((item) => item.id).sort()).toEqual([
      "api",
      "git-change:src/service.ts",
      "handler",
      "service",
    ]);
    expect(result.nodes).toEqual(expect.arrayContaining([
      expect.objectContaining({
        id: "git-change:src/service.ts",
        label: "Modified service.ts",
        metadata: expect.objectContaining({ gitChangeMarker: true, gitChangeKind: "modify" }),
      }),
      expect.objectContaining({
        id: "service",
        metadata: expect.objectContaining({ gitChanged: true }),
      }),
    ]));
    expect(result.edges).toContainEqual(expect.objectContaining({
      source: "service",
      target: "git-change:src/service.ts",
      kind: "changedIn",
    }));
    expect(result.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: "api", target: "handler" }),
      expect.objectContaining({ source: "handler", target: "service" }),
    ]));
    expect(result.nodes.some((item) => item.id === "repo" || item.id === "external")).toBe(false);
  });

  it("creates visible change nodes for files missing from the bounded workspace graph", () => {
    const result = relationshipGraphForView(
      workspace,
      workspace,
      "changes",
      new Set(["README.md", "proto/new.proto"]),
      [
        { relativePath: "README.md", indexStatus: " ", workingTreeStatus: "M", staged: false, conflicted: false },
        { relativePath: "proto/new.proto", indexStatus: "?", workingTreeStatus: "?", staged: false, conflicted: false },
      ],
    );

    expect(result.nodes).toEqual(expect.arrayContaining([
      expect.objectContaining({
        id: "git-change:README.md",
        label: "Modified README.md",
        metadata: expect.objectContaining({ gitChangeKind: "modify" }),
      }),
      expect.objectContaining({
        id: "git-change:proto/new.proto",
        label: "Inserted new.proto",
        metadata: expect.objectContaining({ gitChangeKind: "insert" }),
      }),
    ]));
  });

  it("uses the diff-matched symbol as the ancestry endpoint within a changed file", () => {
    const result = relationshipGraphForView(
      workspace,
      workspace,
      "changes",
      new Set(["src/api.ts"]),
      [],
      "handler",
    );

    expect(result.nodes.map((item) => item.id).sort()).toEqual([
      "api",
      "git-change:src/api.ts",
      "handler",
    ]);
    expect(result.nodes.find((item) => item.id === "handler")?.metadata.gitChanged).toBe(true);
    expect(result.nodes.find((item) => item.id === "api")?.metadata.gitChanged).not.toBe(true);
    expect(result.nodes.some((item) => item.id === "service")).toBe(false);
  });

  it("uses the selected execution-flow graph for the deep code-path view", () => {
    const codePath = { ...workspace, nodes: [workspace.nodes[2]!], edges: [] };
    expect(relationshipGraphForView(workspace, codePath, "codepath", new Set())).toBe(codePath);
  });
});
