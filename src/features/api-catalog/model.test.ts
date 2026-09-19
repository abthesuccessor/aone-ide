import { describe, expect, it } from "vitest";
import {
  buildApiSourceGroups,
  filterApiEndpoints,
  type ApiEndpointInventoryItem,
} from "./model";

const wireFixture: ApiEndpointInventoryItem = {
  id: "openapi:GET:/api/v1/graph/status",
  label: "Graph status",
  method: "GET",
  path: "/api/v1/graph/status",
  role: "producer",
  protocol: "http",
  sourceFile: "openapi/openapi.json",
  language: "json",
  framework: "OpenAPI",
  operationId: "graphStatus",
  evidence: "declared",
  grouping: {
    key: "TYSON API / Graph",
    segments: ["TYSON API", "Graph"],
    basis: "openApiTag",
  },
  handler: {
    id: "handler:graph-status",
    kind: "function",
    label: "getGraphStatus",
    evidence: "resolved",
  },
  clientCoverage: {
    count: 1,
    firstSource: {
      relativePath: "tyson-frontend/src/api/graph.ts",
      startLine: 44,
      startColumn: 1,
      endLine: 48,
      endColumn: 1,
    },
  },
  occurrenceCount: 3,
};

describe("API inventory contract model", () => {
  it("accepts the frozen camelCase wire shape with an optional server range", () => {
    expect(wireFixture.source).toBeUndefined();
    const groups = buildApiSourceGroups([wireFixture]);
    expect(groups[0]?.label).toBe("TYSON API / Graph");
    expect(groups[0]?.routes[0]?.label).toBe("/api/v1/graph");
  });

  it("filters unique operations by server and client coverage independently", () => {
    const serverOnly = {
      ...wireFixture,
      id: "openapi:POST:/api/v1/demands",
      method: "POST",
      path: "/api/v1/demands",
      clientCoverage: { count: 0 },
    } satisfies ApiEndpointInventoryItem;
    const clientOnly = {
      ...wireFixture,
      id: "client:stripe",
      role: "consumer" as const,
      path: "https://api.stripe.com/v1/customers",
    } satisfies ApiEndpointInventoryItem;
    const endpoints = [wireFixture, serverOnly, clientOnly];
    expect(filterApiEndpoints(endpoints, { query: "", coverage: "server", method: "ALL" })).toHaveLength(2);
    expect(filterApiEndpoints(endpoints, { query: "", coverage: "client-covered", method: "ALL" })).toEqual([wireFixture]);
    expect(filterApiEndpoints(endpoints, { query: "", coverage: "client-only", method: "ALL" })).toEqual([clientOnly]);
    expect(filterApiEndpoints(endpoints, { query: "graph/status", coverage: "all", method: "GET" })).toEqual([wireFixture]);
  });

  it("groups sanitized external client targets by host and route prefix", () => {
    const external: ApiEndpointInventoryItem = {
      ...wireFixture,
      id: "client:stripe",
      role: "consumer",
      path: "https://api.stripe.com/v1/checkout/sessions",
      grouping: { key: "src/api/clients", segments: ["src", "api", "clients"], basis: "sourcePath" },
    };
    expect(buildApiSourceGroups([external])[0]?.routes[0]?.label)
      .toBe("https://api.stripe.com/v1/checkout");
  });
});
