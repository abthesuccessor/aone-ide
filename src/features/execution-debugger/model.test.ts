import { describe, expect, it } from "vitest";
import type { RuntimeEvent } from "../../types";
import type { DebugSessionView } from "./contracts";
import {
  buildDebugExecutionTrace,
  criticalExecutionSteps,
  debuggerCapabilities,
  debugExecutionStepId,
  debugWorkflowSummaries,
} from "./model";

function runtimeEvent(overrides: Partial<RuntimeEvent> = {}): RuntimeEvent {
  return {
    id: "event-1",
    runId: "run-1",
    kind: "debug.database",
    timestamp: "2026-08-17T02:00:00Z",
    label: "Load recommendation rows",
    evidence: "observed",
    metadata: {
      workflowId: "workflow-1",
      sequence: 4,
      stepId: "load-rows",
      flowStage: "data",
      safePointState: "paused",
      sourceRelativePath: "src/repository.rs",
      sourceLine: 82,
      operation: "selectRecommendations",
      resource: "recommendations",
      dataPreview: {
        kind: "rowSet",
        fields: [{ name: "id", valueType: "uuid", nullable: false }],
        rowCount: 3,
        truncated: false,
        forbiddenValue: "secret",
      },
    },
    ...overrides,
  };
}

function capability(supported = true, cooperative = true) {
  return { supported, cooperative };
}

function session(events: DebugSessionView["events"] = []): DebugSessionView {
  return {
    debugSessionId: "debug-1",
    runId: "run-1",
    status: "paused",
    eventCount: events.length,
    controlEpoch: 2,
    acknowledgedControlEpoch: 2,
    currentStepId: events.at(-1)?.stepId,
    activeWorkflowId: events.at(-1)?.workflowId,
    workflowCount: 1,
    startedAt: "2026-08-17T02:00:00Z",
    capabilities: {
      pause: capability(), resume: capability(), stepInto: capability(),
      stepOver: capability(), stop: capability(true, false),
    },
    limitation: "Cooperative safe points only.",
    events,
    eventsTruncated: false,
  };
}

describe("execution debugger model", () => {
  it("keeps process-reported semantics and ignores unsupported arbitrary preview fields", () => {
    const trace = buildDebugExecutionTrace([runtimeEvent()], "run-1");
    expect(trace.mode).toBe("debug");
    expect(trace.steps[0]).toMatchObject({
      id: debugExecutionStepId("workflow-1", "load-rows"),
      reportedStepId: "load-rows",
      workflowId: "workflow-1",
      state: "paused",
      operation: "selectRecommendations",
      resource: "recommendations",
      source: { relativePath: "src/repository.rs", startLine: 82 },
      preview: {
        kind: "rowSet",
        fields: [{ name: "id", valueType: "uuid", nullable: false }],
        rowCount: 3,
      },
    });
    expect(JSON.stringify(trace.steps[0]?.preview)).not.toContain("secret");
  });

  it("uses workflow-qualified card identities for repeated checkpoint names", () => {
    const trace = buildDebugExecutionTrace([
      runtimeEvent(),
      runtimeEvent({
        id: "event-2",
        metadata: { ...runtimeEvent().metadata, workflowId: "workflow-2", sequence: 5 },
      }),
    ], "run-1");
    expect(new Set(trace.steps.map((step) => step.id)).size).toBe(2);
    expect(debugWorkflowSummaries(trace.steps).map((workflow) => workflow.id)).toEqual([
      "workflow-2",
      "workflow-1",
    ]);
  });

  it("does not keep every historical paused line in the key-checkpoint view", () => {
    const steps = buildDebugExecutionTrace([
      runtimeEvent({ kind: "debug.line", metadata: { ...runtimeEvent().metadata, stepId: "line-1", sequence: 1 } }),
      runtimeEvent({ id: "event-2", kind: "debug.line", metadata: { ...runtimeEvent().metadata, stepId: "line-2", sequence: 2 } }),
      runtimeEvent({ id: "event-3", kind: "debug.branch", metadata: { ...runtimeEvent().metadata, stepId: "branch", sequence: 3 } }),
    ], "run-1").steps;
    expect(criticalExecutionSteps(steps, debugExecutionStepId("workflow-1", "line-2")).map((step) => step.reportedStepId)).toEqual([
      "line-2",
      "branch",
    ]);
  });

  it("keeps authoritative process stop available when it is supported but non-cooperative", () => {
    const view = session();
    view.status = "running";
    expect(debuggerCapabilities(view)).toMatchObject({ pause: true, stop: true, stepInto: false });
  });

  it("treats uncorrelated HTTP boundaries as separate workflows", () => {
    const trace = buildDebugExecutionTrace([
      runtimeEvent({ id: "http-1", runId: undefined, kind: "http.response", metadata: {} }),
      runtimeEvent({ id: "http-2", runId: undefined, kind: "http.response", metadata: {} }),
    ]);
    expect(trace.mode).toBe("boundary-only");
    expect(trace.steps.map((step) => step.workflowId)).toEqual(["boundary:http-1", "boundary:http-2"]);
  });

  it("uses the canonical session event state and seven-lane response mapping", () => {
    const view = session([{
      id: "response-event",
      workflowId: "workflow-1",
      sequence: 9,
      controlEpoch: 2,
      stepId: "response",
      kind: "response",
      label: "Return 200",
      flowStage: "response",
      source: { relativePath: "src/api.rs", line: 91 },
      safePointState: "workflowCompleted",
      timestamp: "2026-08-17T02:00:03Z",
    }]);
    const trace = buildDebugExecutionTrace([], "run-1", view);
    expect(trace.steps[0]).toMatchObject({ stage: "response", state: "workflowCompleted" });
  });

  it("combines an injected W3C client span with OTLP child spans in one trace workflow", () => {
    const traceId = "5b8efff798038103d269b633813fc60c";
    const observed = (
      id: string,
      spanId: string,
      protocol: "W3C_TRACE_CONTEXT" | "OTLP_HTTP_JSON",
      parentSpanId?: string,
    ) => runtimeEvent({
      id,
      runId: protocol === "OTLP_HTTP_JSON" ? "otlp:receiver" : undefined,
      traceId,
      kind: protocol === "OTLP_HTTP_JSON" ? "trace.http.server.event" : "trace.http.client.event",
      label: id,
      metadata: {
        traceProtocol: protocol,
        spanId,
        phase: "event",
        ...(parentSpanId ? { parentSpanId } : {}),
      },
    });
    const result = buildDebugExecutionTrace([
      observed("Aone API client", "1111111111111111", "W3C_TRACE_CONTEXT"),
      observed("GET /shipments", "2222222222222222", "OTLP_HTTP_JSON", "1111111111111111"),
    ]);

    expect(result.mode).toBe("trace-only");
    expect(result.steps).toHaveLength(2);
    expect(new Set(result.steps.map((step) => step.workflowId))).toEqual(new Set([`trace:${traceId}`]));
    expect(result.steps.map((step) => step.protocol)).toEqual(["W3C_TRACE_CONTEXT", "OTLP_HTTP_JSON"]);
    expect(result.steps[1]?.parentStepId).toBe(`${result.steps[0]?.workflowId}:1111111111111111`);
  });
});
