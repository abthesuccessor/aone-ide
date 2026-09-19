import { describe, expect, it } from "vitest";
import type { GraphNode, GraphSnapshot } from "../../types";
import {
  createExecutionFlowLayout,
  executionFlowEdgePath,
  MAX_RENDERED_FLOW_EDGES,
  nextFlowNodeByGeometry,
} from "./flowLayout";

function node(id: string, stage: string, group = `${stage} group`): GraphNode {
  return { id, kind: "function", label: id, evidence: "resolved", metadata: { flowStage: stage, flowGroup: group } };
}

function layout(graph: GraphSnapshot, rootNodeId: string | null = null, expandedGroups = new Set<string>()) {
  return createExecutionFlowLayout(graph, { rootNodeId, expandedGroups, mode: "trace" });
}

function geometry(result: ReturnType<typeof layout>) {
  return result.nodes.map(({ node: candidate, stage, groupKey, x, y }) => ({ id: candidate.id, stage, groupKey, x, y }))
    .sort((left, right) => left.id.localeCompare(right.id));
}

describe("static execution flow layout", () => {
  it("is deterministic for shuffled cyclic input and emits orthogonal paths", () => {
    const nodes = [
      node("ui", "frontend"), node("api", "api", "Api"), node("api-helper", "api", "API"),
      node("handler", "backend"), node("service", "service"),
    ];
    const edges = [
      { id: "1", source: "ui", target: "api", kind: "requests", evidence: "resolved" as const, metadata: {} },
      { id: "2", source: "api", target: "handler", kind: "handledBy", evidence: "resolved" as const, metadata: {} },
      { id: "3", source: "handler", target: "service", kind: "calls", evidence: "resolved" as const, metadata: {} },
      { id: "4", source: "service", target: "handler", kind: "returns", evidence: "resolved" as const, metadata: {} },
    ];
    const first = layout({ nodes, edges, truncated: false });
    const shuffled = layout({ nodes: [...nodes].reverse(), edges: [...edges].reverse(), truncated: false });
    expect(geometry(first)).toEqual(geometry(shuffled));
    expect(first.groups.map((group) => group.label)).toEqual(shuffled.groups.map((group) => group.label));
    expect(first.groups.find((group) => group.key === "api:api")?.label).toBe("API");
    expect(executionFlowEdgePath(first.nodes[0]!, first.nodes[1]!)).toMatch(/^M .* V .* H .* V /);
  });

  it("places stages top to bottom and cards within each stage left to right", () => {
    const graph: GraphSnapshot = {
      nodes: [
        node("trigger-a", "frontend", "Trigger"),
        node("trigger-b", "frontend", "Trigger"),
        node("api", "api"),
        node("handler", "backend"),
        node("service", "service"),
        node("repository", "data"),
        node("vendor", "external"),
      ],
      edges: [],
      truncated: false,
    };
    const result = layout(graph);
    const byId = new Map(result.nodes.map((candidate) => [candidate.node.id, candidate]));

    expect(byId.get("trigger-a")!.y).toBe(byId.get("trigger-b")!.y);
    expect(byId.get("trigger-a")!.x).toBeLessThan(byId.get("trigger-b")!.x);
    expect(byId.get("trigger-a")!.y).toBeLessThan(byId.get("api")!.y);
    expect(byId.get("api")!.y).toBeLessThan(byId.get("handler")!.y);
    expect(byId.get("handler")!.y).toBeLessThan(byId.get("service")!.y);
    expect(byId.get("service")!.y).toBeLessThan(byId.get("repository")!.y);
    expect(byId.get("repository")!.y).toBeLessThan(byId.get("vendor")!.y);
    expect(result.stages.map((stage) => stage.id)).toEqual([
      "trigger", "api", "backend", "service", "data", "external",
    ]);
    expect(result.stages.every((stage, index, stages) => index === 0 || stage.y > stages[index - 1]!.y)).toBe(true);
  });

  it("uses left and right within a stage, and up and down between stages", () => {
    const result = layout({
      nodes: [
        node("trigger-a", "frontend", "Trigger"),
        node("trigger-b", "frontend", "Trigger"),
        node("api", "api"),
        node("handler", "backend"),
      ],
      edges: [],
      truncated: false,
    });

    expect(nextFlowNodeByGeometry(result.nodes, "trigger-a", "ArrowRight")?.node.id).toBe("trigger-b");
    expect(nextFlowNodeByGeometry(result.nodes, "trigger-b", "ArrowLeft")?.node.id).toBe("trigger-a");
    expect(nextFlowNodeByGeometry(result.nodes, "trigger-a", "ArrowDown")?.node.id).toBe("api");
    expect(nextFlowNodeByGeometry(result.nodes, "api", "ArrowUp")?.node.id).toBe("trigger-a");
    expect(nextFlowNodeByGeometry(result.nodes, "api", "ArrowDown")?.node.id).toBe("handler");
    expect(nextFlowNodeByGeometry(result.nodes, "handler", "Home")?.node.id).toBe("trigger-a");
    expect(nextFlowNodeByGeometry(result.nodes, "trigger-a", "End")?.node.id).toBe("handler");
  });

  it("bounds a 500-node group to three cards and expands without a force simulation", () => {
    const graph: GraphSnapshot = {
      nodes: Array.from({ length: 500 }, (_, index) => node(`service-${index}`, "service", "Services")),
      edges: [],
      truncated: false,
    };
    const collapsed = layout(graph);
    expect(collapsed.nodes).toHaveLength(3);
    expect(collapsed.hiddenNodeCount).toBe(497);
    expect(collapsed.groups[0]).toMatchObject({ truthCount: 500, hiddenCount: 497, expanded: false });
    const expanded = layout(graph, null, new Set([collapsed.groups[0]!.key]));
    expect(expanded.nodes).toHaveLength(18);
    expect(expanded.hiddenNodeCount).toBe(482);
  });

  it("forces a searched or selected hidden card and its source group into the trace", () => {
    const nodes = Array.from({ length: 10 }, (_, index) => node(`ordinary-${index}`, "service", `Group ${index}`));
    nodes.push(node("selected", "service", "Z hidden group"));
    const graph: GraphSnapshot = { nodes, edges: [], truncated: false };
    const ordinary = layout(graph);
    expect(ordinary.nodes.map((candidate) => candidate.node.id)).not.toContain("selected");
    const focused = layout(graph, "selected");
    expect(focused.nodes.map((candidate) => candidate.node.id)).toContain("selected");
    expect(focused.groups.map((group) => group.label)).toContain("Z hidden group");
  });

  it("keeps all stage cards non-overlapping", () => {
    const graph: GraphSnapshot = {
      nodes: [
        node("ui", "frontend"), node("api", "api"), node("handler", "backend"),
        node("service", "service"), node("repo", "data"), node("cloud", "external"),
      ],
      edges: [],
      truncated: false,
    };
    const result = layout(graph);
    for (let index = 0; index < result.nodes.length; index += 1) {
      for (let other = index + 1; other < result.nodes.length; other += 1) {
        const left = result.nodes[index]!;
        const right = result.nodes[other]!;
        const overlap = left.x < right.x + right.width && left.x + left.width > right.x
          && left.y < right.y + right.height && left.y + left.height > right.y;
        expect(overlap).toBe(false);
      }
    }
  });

  it("deterministically caps dense SVG relationships and prioritizes selected and observed edges", () => {
    const stages = ["frontend", "api", "backend", "service", "data", "external"];
    const nodes = stages.flatMap((stage) => Array.from(
      { length: 18 },
      (_, index) => node(`${stage}-${index}`, stage, `${stage} group`),
    ));
    const edges = nodes.flatMap((source) => nodes
      .filter((target) => target.id !== source.id)
      .map((target) => ({
        id: `${source.id}->${target.id}`,
        source: source.id,
        target: target.id,
        kind: "calls",
        evidence: source.id === "frontend-0" ? "observed" as const : "inferred" as const,
        metadata: {},
      })));
    const expanded = new Set(stages.map((stage) => `${stage === "frontend" ? "trigger" : stage}:${stage} group`));
    const result = layout({ nodes, edges, truncated: false }, "frontend-0", expanded);
    const reversed = layout({
      nodes: [...nodes].reverse(),
      edges: [...edges].reverse(),
      truncated: false,
    }, "frontend-0", expanded);

    expect(result.edges).toHaveLength(MAX_RENDERED_FLOW_EDGES);
    expect(result.omittedVisibleEdgeCount).toBe(edges.length - MAX_RENDERED_FLOW_EDGES);
    expect(result.edges[0]).toMatchObject({ source: "frontend-0", evidence: "observed" });
    expect(reversed.edges.map((edge) => edge.id)).toEqual(result.edges.map((edge) => edge.id));
    expect(reversed.omittedVisibleEdgeCount).toBe(result.omittedVisibleEdgeCount);
    expect(reversed.edges[0]).toMatchObject({ source: "frontend-0", evidence: "observed" });
  });
});
