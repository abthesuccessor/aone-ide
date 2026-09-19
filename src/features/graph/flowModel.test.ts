import { describe, expect, it } from "vitest";
import type { GraphNode, GraphSnapshot } from "../../types";
import {
  flowGroupForNode,
  flowStageForNode,
  latestObservedFlowNodeId,
  observedFlowNodeIds,
  searchFlowNodes,
  traceScopeNodeIds,
} from "./flowModel";

function node(id: string, kind: string, metadata: GraphNode["metadata"] = {}, path?: string): GraphNode {
  return {
    id,
    kind,
    label: id,
    evidence: "resolved",
    metadata,
    ...(path ? { source: { relativePath: path, startLine: 1, startColumn: 1, endLine: 2, endColumn: 1 } } : {}),
  };
}

describe("execution flow model", () => {
  it("honors backend flow stages before legacy kind and path heuristics", () => {
    expect(flowStageForNode(node("repository helper", "function", { flowStage: "data" }, "frontend/store.ts"))).toBe("data");
    expect(flowStageForNode(node("agent", "function", { flowStage: "agent" }))).toBe("service");
    expect(flowStageForNode(node("event", "function", { flowStage: "event" }))).toBe("api");
    expect(flowStageForNode(node("view", "component", {}, "src/pages/Checkout.tsx"))).toBe("trigger");
  });

  it("uses explicit groups and derives stable source groups as a fallback", () => {
    expect(flowGroupForNode(node("one", "function", { flowGroup: "Order Service" }))).toBe("Order Service");
    expect(flowGroupForNode(node("two", "function", {}, "tyson-backend/src/orders/create.rs"))).toBe("Tyson backend / orders");
  });

  it("walks upstream triggers and downstream nested calls from a selected API", () => {
    const graph: GraphSnapshot = {
      nodes: ["ui", "api", "handler", "service", "repo", "other"].map((id) => node(id, id === "api" ? "endpoint" : "function")),
      edges: [
        { id: "1", source: "ui", target: "api", kind: "requests", evidence: "resolved", metadata: {} },
        { id: "2", source: "api", target: "handler", kind: "handledBy", evidence: "resolved", metadata: {} },
        { id: "3", source: "handler", target: "service", kind: "calls", evidence: "resolved", metadata: {} },
        { id: "4", source: "service", target: "repo", kind: "writes", evidence: "resolved", metadata: {} },
      ],
      truncated: false,
    };
    expect([...traceScopeNodeIds(graph, "api")].sort()).toEqual(["api", "handler", "repo", "service", "ui"]);
  });

  it("walks an upstream branch even when a cycle already reached its convergence node downstream", () => {
    const graph: GraphSnapshot = {
      nodes: ["root", "a", "b"].map((id) => node(id, "function")),
      edges: [
        { id: "down", source: "root", target: "a", kind: "calls", evidence: "resolved", metadata: {} },
        { id: "cycle", source: "a", target: "root", kind: "calls", evidence: "resolved", metadata: {} },
        { id: "upstream", source: "b", target: "a", kind: "calls", evidence: "resolved", metadata: {} },
      ],
      truncated: false,
    };

    expect([...traceScopeNodeIds(graph, "root")].sort()).toEqual(["a", "b", "root"]);
  });

  it("attaches observed mapped spans and their parent chain to a selected static API corridor", () => {
    const observedRuntime = (id: string) => ({
      ...node(id, "runtime-event", { traceId: "trace-a" }), evidence: "observed" as const,
    });
    const graph: GraphSnapshot = {
      nodes: [
        node("api", "endpoint"), node("handler", "handler"), node("service", "service"),
        node("other", "service"), observedRuntime("r1"), observedRuntime("r2"), observedRuntime("unrelated"),
      ],
      edges: [
        { id: "api-handler", source: "api", target: "handler", kind: "handledBy", evidence: "resolved", metadata: {} },
        { id: "handler-service", source: "handler", target: "service", kind: "calls", evidence: "resolved", metadata: {} },
        { id: "r1-handler", source: "r1", target: "handler", kind: "correlatesTo", evidence: "inferred", metadata: {} },
        { id: "r2-service", source: "r2", target: "service", kind: "correlatesTo", evidence: "inferred", metadata: {} },
        { id: "r1-r2", source: "r1", target: "r2", kind: "parentSpan", evidence: "observed", metadata: {} },
        { id: "unrelated-other", source: "unrelated", target: "other", kind: "correlatesTo", evidence: "inferred", metadata: {} },
      ],
      truncated: false,
    };

    expect([...traceScopeNodeIds(graph, "api")].sort()).toEqual(["api", "handler", "r1", "r2", "service"]);
    expect([...observedFlowNodeIds(graph)].sort()).toEqual(["r1", "r2", "unrelated"]);
    expect(graph.nodes.find((candidate) => candidate.id === "service")?.evidence).toBe("resolved");
  });

  it("does not promote static internals from an inferred runtime correlation", () => {
    const runtime = { ...node("runtime", "runtime-event"), evidence: "observed" as const };
    const endpoint = node("endpoint", "endpoint");
    const internal = node("internal", "function");
    const graph: GraphSnapshot = {
      nodes: [runtime, endpoint, internal],
      edges: [
        { id: "correlation", source: "runtime", target: "endpoint", kind: "correlatesTo", evidence: "inferred", metadata: {} },
        { id: "call", source: "endpoint", target: "internal", kind: "calls", evidence: "resolved", metadata: {} },
      ],
      truncated: false,
    };
    expect([...observedFlowNodeIds(graph)]).toEqual(["runtime"]);
    graph.edges[0] = { ...graph.edges[0]!, evidence: "observed" };
    expect([...observedFlowNodeIds(graph)]).toEqual(["runtime"]);
  });

  it("selects the newest timestamped observed card as the live flow position", () => {
    const graph: GraphSnapshot = {
      nodes: [
        { ...node("first", "runtime-event", { timestamp: "2026-08-22T01:00:00Z" }), evidence: "observed" },
        { ...node("latest", "runtime-event", { timestamp: "2026-08-22T01:00:02Z" }), evidence: "observed" },
        { ...node("static", "function", { timestamp: "2026-08-22T01:00:03Z" }), evidence: "resolved" },
      ],
      edges: [],
      truncated: false,
    };

    expect(latestObservedFlowNodeId(graph)).toBe("latest");
  });

  it("searches labels, kinds, groups, and exact source paths deterministically", () => {
    const graph: GraphSnapshot = {
      nodes: [
        node("alpha", "trait", { flowGroup: "Payments" }, "src/services/payment.rs"),
        node("beta", "repository", { flowGroup: "Orders" }, "src/data/order.rs"),
      ],
      edges: [],
      truncated: false,
    };
    expect(searchFlowNodes(graph, "payment.rs").map((candidate) => candidate.id)).toEqual(["alpha"]);
    expect(searchFlowNodes(graph, "Orders").map((candidate) => candidate.id)).toEqual(["beta"]);
  });
});
