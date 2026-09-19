import { describe, expect, it } from "vitest";
import type { ApiEndpointInventoryItem } from "../api-catalog/model";
import type { GraphSnapshot } from "../../types";
import { buildEndpointCallTree, flattenKnowledgeTree } from "./model";

const endpoint: ApiEndpointInventoryItem = {
  id: "api:orders",
  label: "createOrder",
  method: "POST",
  path: "/orders",
  sourceFile: "src/routes.ts",
  source: { relativePath: "src/routes.ts", startLine: 10, endLine: 12, startColumn: 1, endColumn: 1 },
  evidence: "declared",
  role: "producer",
  protocol: "http",
  grouping: { key: "orders", segments: ["Orders"], basis: "sourcePath" },
  handler: { id: "handler", label: "createOrder", kind: "handler", evidence: "resolved" },
  clientCoverage: { count: 1 },
  occurrenceCount: 1,
};

const graph: GraphSnapshot = {
  nodes: [
    { id: "api:orders", kind: "endpoint", label: "POST /orders", evidence: "declared", metadata: {} },
    { id: "handler", kind: "handler", label: "createOrder", evidence: "resolved", metadata: {} },
    { id: "service", kind: "service", label: "OrderService", evidence: "resolved", metadata: {} },
    { id: "repo", kind: "repository", label: "OrderRepository", evidence: "resolved", metadata: {} },
  ],
  edges: [
    { id: "a", source: "api:orders", target: "handler", kind: "handles", evidence: "resolved", confidence: 1, metadata: {} },
    { id: "b", source: "handler", target: "service", kind: "calls", evidence: "resolved", confidence: 1, metadata: {} },
    { id: "c", source: "service", target: "repo", kind: "calls", evidence: "resolved", confidence: 1, metadata: {} },
    { id: "d", source: "repo", target: "service", kind: "calls", evidence: "inferred", confidence: 0.5, metadata: {} },
  ],
  truncated: false,
};

describe("code knowledge call tree", () => {
  it("renders a bounded nested API to repository path and marks cycles", () => {
    const tree = buildEndpointCallTree(endpoint, graph);
    expect(tree.label).toBe("POST /orders");
    expect(tree.children[0]?.label).toBe("createOrder");
    expect(tree.children[0]?.children[0]?.label).toBe("OrderService");
    const flattened = flattenKnowledgeTree(tree);
    expect(flattened.map((node) => node.label)).toContain("OrderRepository");
    expect(flattened.some((node) => node.cycle)).toBe(true);
  });

  it("falls back to declared endpoint and handler evidence before a trace is loaded", () => {
    const tree = buildEndpointCallTree(endpoint, { nodes: [], edges: [], truncated: false });
    expect(tree.children).toHaveLength(1);
    expect(tree.children[0]).toMatchObject({ label: "createOrder", relation: "handles" });
  });
});
