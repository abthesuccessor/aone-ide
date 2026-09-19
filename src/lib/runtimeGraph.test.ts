import { describe, expect, it } from "vitest";
import type { GraphSnapshot, RuntimeEvent } from "../types";
import { withRuntimeProjection } from "./runtimeGraph";

describe("withRuntimeProjection", () => {
  it("keeps the event observed while labeling URL correlation as inferred", () => {
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [{
        id: "endpoint:orders",
        kind: "endpoint",
        label: "POST /api/orders",
        evidence: "declared",
        metadata: { method: "POST", route: "/api/orders" },
      }],
      edges: [],
    };
    const event: RuntimeEvent = {
      id: "event-1",
      kind: "http.response",
      timestamp: "2026-08-16T10:00:00Z",
      label: "HTTP 201 in 14 ms",
      evidence: "observed",
      metadata: { method: "POST", url: "http://127.0.0.1:4310/api/orders", status: 201 },
    };

    const projected = withRuntimeProjection(graph, [event]);

    expect(projected.nodes.find((node) => node.id === "runtime:event-1")?.evidence).toBe("observed");
    expect(projected.edges).toContainEqual(expect.objectContaining({
      source: "runtime:event-1",
      target: "endpoint:orders",
      kind: "correlatesTo",
      evidence: "inferred",
    }));
  });

  it("uses an observed edge when the backend supplies a source node", () => {
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [{ id: "handler", kind: "function", label: "handler", evidence: "declared", metadata: {} }],
      edges: [],
    };
    const event: RuntimeEvent = {
      id: "event-2",
      kind: "process.stdout",
      timestamp: "2026-08-16T10:00:00Z",
      label: "handler entered",
      evidence: "observed",
      sourceNodeId: "handler",
      metadata: {},
    };

    expect(withRuntimeProjection(graph, [event]).edges[0]).toEqual(expect.objectContaining({
      kind: "observedAt",
      evidence: "observed",
    }));
  });

  it("matches one route template through analyzer route metadata without claiming observation", () => {
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [{
        id: "endpoint:order",
        kind: "endpoint",
        label: "orderById",
        evidence: "declared",
        metadata: { httpMethod: "GET", routePath: "/api/orders/{orderId}" },
      }],
      edges: [],
    };
    const event = {
      id: "event-template",
      kind: "http.response",
      timestamp: "2026-08-16T10:00:00Z",
      label: "HTTP 200",
      evidence: "observed",
      metadata: { method: "GET", path: "/api/orders/42" },
    } satisfies RuntimeEvent;

    expect(withRuntimeProjection(graph, [event]).edges).toContainEqual(expect.objectContaining({
      target: "endpoint:order",
      kind: "correlatesTo",
      evidence: "inferred",
    }));
  });

  it("does not correlate an observed request when route templates are ambiguous", () => {
    const endpoint = (id: string, routePath: string) => ({
      id,
      kind: "endpoint",
      label: id,
      evidence: "declared" as const,
      metadata: { httpMethod: "GET", routePath },
    });
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [
        endpoint("endpoint:order", "/api/orders/{orderId}"),
        endpoint("endpoint:order-slug", "/api/orders/:slug"),
      ],
      edges: [],
    };
    const event = {
      id: "event-ambiguous",
      kind: "http.response",
      timestamp: "2026-08-16T10:00:00Z",
      label: "HTTP 200",
      evidence: "observed",
      metadata: { method: "GET", path: "/api/orders/42" },
    } satisfies RuntimeEvent;

    expect(withRuntimeProjection(graph, [event]).edges).toHaveLength(0);
  });

  it("keeps ordinary process output in the timeline but out of the graph", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const stdout = {
      id: "stdout-1",
      kind: "process.stdout",
      timestamp: "2026-08-16T10:00:00Z",
      label: "stdout",
      evidence: "observed",
      metadata: { text: "\u001b[32mready\u001b[0m" },
    } satisfies RuntimeEvent;
    const stderr = { ...stdout, id: "stderr-1", kind: "process.stderr" };

    expect(withRuntimeProjection(graph, [stdout, stderr])).toBe(graph);
    expect(withRuntimeProjection(graph, [stdout, stderr]).nodes).toHaveLength(0);
  });

  it("keeps rejected and unsupported AONE trace diagnostics in the timeline only", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const traceEvent = (id: string, kind: string, metadata: RuntimeEvent["metadata"]) => ({
      id,
      kind,
      timestamp: "2026-08-16T10:00:00Z",
      label: id,
      evidence: "observed" as const,
      metadata,
    }) satisfies RuntimeEvent;
    const rejected = traceEvent("rejected", "trace.rejected", { traceProtocol: "AONE_TRACE_V1" });
    const unsupported = traceEvent("unsupported", "trace.internal.start", {
      traceProtocol: "AONE_TRACE_V1", spanId: "internal", phase: "start",
    });

    expect(withRuntimeProjection(graph, [rejected, unsupported])).toBe(graph);
    expect(withRuntimeProjection(graph, [rejected, unsupported]).nodes).toHaveLength(0);
  });

  it("draws only unambiguous validated parent spans within one run and trace", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const span = (
      id: string,
      spanId: string,
      parentSpanId?: string,
      traceId = "trace-a",
    ) => ({
      id,
      runId: "run-a",
      traceId,
      kind: "runtime.span",
      timestamp: "2026-08-16T10:00:00Z",
      label: id,
      evidence: "observed" as const,
      metadata: { spanId, ...(parentSpanId ? { parentSpanId } : {}) },
    }) satisfies RuntimeEvent;

    const projected = withRuntimeProjection(graph, [span("parent", "span-1"), span("child", "span-2", "span-1")]);
    expect(projected.edges).toContainEqual(expect.objectContaining({
      source: "runtime:parent",
      target: "runtime:child",
      kind: "parentSpan",
      evidence: "observed",
    }));

    const ambiguous = withRuntimeProjection(graph, [
      span("parent-a", "span-1"),
      span("parent-b", "span-1"),
      span("child-ambiguous", "span-2", "span-1"),
    ]);
    expect(ambiguous.edges).toHaveLength(0);
    const otherTrace = withRuntimeProjection(graph, [
      span("parent-other", "span-1", undefined, "trace-b"),
      span("child-a", "span-2", "span-1", "trace-a"),
    ]);
    expect(otherTrace.edges).toHaveLength(0);
  });

  it("uses one visual span card when a validated span has start and end events", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const event = (id: string, spanId: string, phase: "start" | "end", parentSpanId?: string) => ({
      id,
      runId: "run-a",
      traceId: "trace-a",
      kind: `trace.function.${phase}`,
      timestamp: phase === "start" ? "2026-08-16T10:00:00Z" : "2026-08-16T10:00:01Z",
      label: spanId,
      evidence: "observed" as const,
      metadata: { traceProtocol: "AONE_TRACE_V1", spanId, phase, ...(parentSpanId ? { parentSpanId } : {}) },
    }) satisfies RuntimeEvent;
    const projected = withRuntimeProjection(graph, [
      event("parent-start", "parent", "start"),
      event("parent-end", "parent", "end"),
      event("child-start", "child", "start", "parent"),
      event("child-end", "child", "end", "parent"),
    ]);

    expect(projected.nodes.map((node) => node.id).sort()).toEqual(["runtime:child-start", "runtime:parent-start"]);
    expect(projected.edges).toContainEqual(expect.objectContaining({
      source: "runtime:parent-start",
      target: "runtime:child-start",
      evidence: "observed",
    }));
  });

  it.each([
    ["trace.http.server.start", "api"],
    ["trace.http.client.start", "api"],
    ["trace.event.event", "api"],
    ["trace.function.start", "service"],
    ["trace.agent.start", "service"],
    ["trace.job.start", "service"],
    ["trace.database.start", "data"],
    ["trace.external.start", "external"],
  ])("places an observed %s span in the %s flow lane", (kind, flowStage) => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const event = {
      id: `span-${kind}`,
      runId: "run-a",
      traceId: "trace-a",
      kind,
      timestamp: "2026-08-16T10:00:00Z",
      label: kind,
      evidence: "observed" as const,
      metadata: { traceProtocol: "AONE_TRACE_V1", spanId: kind, phase: "start" },
    } satisfies RuntimeEvent;

    expect(withRuntimeProjection(graph, [event]).nodes[0]).toEqual(expect.objectContaining({
      evidence: "observed",
      metadata: expect.objectContaining({ flowStage }),
    }));
  });

  it("keeps a process-reported source-range mapping inferred", () => {
    const graph: GraphSnapshot = {
      truncated: false,
      nodes: [{ id: "function:work", kind: "function", label: "work", evidence: "resolved", metadata: {} }],
      edges: [],
    };
    const event = {
      id: "span-mapped",
      runId: "run-a",
      traceId: "trace-a",
      kind: "trace.function.event",
      timestamp: "2026-08-16T10:00:00Z",
      label: "work",
      evidence: "observed",
      metadata: {
        traceProtocol: "AONE_TRACE_V1",
        spanId: "span-1",
        phase: "event",
        mappedNodeId: "function:work",
        mappingKind: "processReportedSourceRange",
        processReported: true,
      },
    } satisfies RuntimeEvent;

    expect(withRuntimeProjection(graph, [event]).edges).toContainEqual(expect.objectContaining({
      source: "runtime:span-mapped",
      target: "function:work",
      kind: "correlatesTo",
      evidence: "inferred",
      metadata: expect.objectContaining({ correlationBasis: "processReportedSourceRange" }),
    }));
  });

  it("accepts supported OTLP protocols and correlates parent spans across producer run ids", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const traceId = "5b8efff798038103d269b633813fc60c";
    const root = {
      id: "client-root",
      traceId,
      kind: "trace.http.client.event",
      timestamp: "2026-08-20T00:00:00Z",
      label: "Aone API client GET",
      evidence: "observed",
      metadata: {
        traceProtocol: "W3C_TRACE_CONTEXT",
        spanId: "1111111111111111",
        phase: "event",
        flowStage: "trigger",
      },
    } satisfies RuntimeEvent;
    const child = {
      ...root,
      id: "server-child",
      runId: "otlp:receiver",
      kind: "trace.http.server.event",
      label: "GET /api/shipments/{shipmentId}",
      metadata: {
        traceProtocol: "OTLP_HTTP_JSON",
        spanId: "2222222222222222",
        parentSpanId: "1111111111111111",
        phase: "event",
        flowStage: "api",
      },
    } satisfies RuntimeEvent;

    const projection = withRuntimeProjection(graph, [root, child]);
    expect(projection.nodes).toHaveLength(2);
    expect(projection.edges).toContainEqual(expect.objectContaining({
      source: "runtime:client-root",
      target: "runtime:server-child",
      kind: "parentSpan",
      evidence: "observed",
    }));
    expect(projection.nodes.find((node) => node.id === "runtime:server-child")?.metadata)
      .toEqual(expect.objectContaining({ flowStage: "api" }));
  });

  it("reuses the projection when appended logs are not graph relevant", () => {
    const graph: GraphSnapshot = { truncated: false, nodes: [], edges: [] };
    const lifecycle = {
      id: "started-1",
      kind: "process.started",
      timestamp: "2026-08-16T10:00:00Z",
      label: "started",
      evidence: "observed",
      metadata: {},
    } satisfies RuntimeEvent;
    const stdout = {
      ...lifecycle,
      id: "stdout-1",
      kind: "process.stdout",
      metadata: { text: "line" },
    } satisfies RuntimeEvent;

    const beforeLog = withRuntimeProjection(graph, [lifecycle]);
    const afterLog = withRuntimeProjection(graph, [lifecycle, stdout]);

    expect(afterLog).toBe(beforeLog);
  });
});
