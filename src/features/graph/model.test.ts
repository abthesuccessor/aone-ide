import { describe, expect, it } from "vitest";
import type { GraphNode, GraphSnapshot } from "../../types";
import { graphForLens, isSensitiveSystemNode, systemLayerForNode } from "./model";

function node(id: string, kind: string, label = id, metadata: GraphNode["metadata"] = {}, relativePath?: string): GraphNode {
  return {
    id, kind, label, evidence: "declared", metadata,
    ...(relativePath ? { source: { relativePath, startLine: 1, startColumn: 1, endLine: 2, endColumn: 1 } } : {}),
  };
}

describe("system graph classification safety", () => {
  it("trusts backend layers without guessing providers from labels", () => {
    expect(systemLayerForNode(node("explicit", "function", "Local helper", { systemLayer: "external" }))).toBe("external");
    expect(systemLayerForNode(node("openai", "function", "OpenAIClient"))).toBe("application");
    expect(systemLayerForNode(node("stripe", "service", "StripePaymentService"))).toBe("application");
    expect(systemLayerForNode(node("external", "external-api", "Billing"))).toBe("external");
  });

  it("recognizes safe config and main files but excludes env and credential nodes", () => {
    expect(systemLayerForNode(node("package", "file", "config/package.json"))).toBe("configuration");
    expect(systemLayerForNode(node("main", "file", "src/main.rs"))).toBe("entry");
    expect(isSensitiveSystemNode(node("env", "file", ".env.production"))).toBe(true);
    expect(isSensitiveSystemNode(node("secret", "credential", "API token"))).toBe(true);
  });

  it("suppresses raw, test, env, and inferred resolver leakage", () => {
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [
        node("app", "service"), node("target", "callTarget"), node("module", "module"),
        node("test", "function", "runs", {}, "src/unit_tests.rs"),
        node("live-test", "function", "runs live", {}, "src/store_live_tests.rs"),
        node("e2e", "function", "runs e2e", {}, "e2e/login.ts"), node("env", "file", ".env"),
      ],
      edges: [
        { id: "inferred", source: "app", target: "target", kind: "resolvesTo", evidence: "inferred", metadata: {} },
        { id: "resolved", source: "app", target: "module", kind: "calls", evidence: "resolved", metadata: {} },
      ],
    };
    const system = graphForLens(graph, "system");
    expect(system.nodes.map((candidate) => candidate.id)).toEqual(["app"]);
    expect(system.edges).toEqual([]);
  });

  it("admits sanitized observed boundaries but keeps ordinary runtime noise out of System", () => {
    const boundary = {
      ...node("runtime:http", "runtime-event", "POST /orders → 201", { eventKind: "http.completed" }),
      evidence: "observed" as const,
    };
    const stdout = {
      ...node("runtime:stdout", "runtime-event", "debug line", { eventKind: "process.stdout" }),
      evidence: "observed" as const,
    };
    const endpoint = node("endpoint", "endpoint", "POST /orders");
    const graph: GraphSnapshot = {
      nodes: [boundary, stdout, endpoint],
      edges: [{ id: "correlation", source: boundary.id, target: endpoint.id, kind: "correlatesTo", evidence: "inferred", metadata: {} }],
      truncated: false,
    };
    const system = graphForLens(graph, "system");
    expect(system.nodes.map((candidate) => candidate.id)).toEqual(["runtime:http", "endpoint"]);
    expect(system.edges).toHaveLength(1);
    expect(system.edges[0]?.evidence).toBe("inferred");
  });

  it("admits only accepted AONE_TRACE_V1 spans and keeps rejected diagnostics in the timeline", () => {
    const observed = (id: string, eventKind: string, metadata: GraphNode["metadata"] = {}) => ({
      ...node(id, "runtime-event", id, { traceProtocol: "AONE_TRACE_V1", eventKind, ...metadata }),
      evidence: "observed" as const,
    });
    const graph: GraphSnapshot = {
      nodes: [
        observed("accepted", "trace.database.start", { spanId: "db-span", phase: "start" }),
        observed("rejected", "trace.rejected"),
        observed("unsupported", "trace.internal.start", { spanId: "internal-span", phase: "start" }),
      ],
      edges: [],
      truncated: false,
    };

    expect(graphForLens(graph, "system").nodes.map((candidate) => candidate.id)).toEqual(["accepted"]);
  });
});
