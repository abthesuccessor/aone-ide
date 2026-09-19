import { describe, expect, it } from "vitest";
import {
  GetDebugSession200ResponseEventsInnerDataPreviewKindEnum,
  GetDebugSession200ResponseEventsInnerFlowStageEnum,
  GetDebugSession200ResponseEventsInnerKindEnum,
  GetDebugSession200ResponseEventsInnerSafePointStateEnum,
  GetDebugSession200ResponseLastWorkflowStateEnum,
  GetDebugSession200ResponseStatusEnum,
  type GetDebugSession200Response,
} from "../../generated/ipc/models";
import { adaptDebugSession } from "./adapter";

describe("debugger generated DTO adapter", () => {
  it("maps the frozen response lane, nested capabilities, and shape-only fields", () => {
    const generated: GetDebugSession200Response = {
      debugSessionId: "debug-1",
      runId: "run-1",
      status: GetDebugSession200ResponseStatusEnum.Paused,
      eventCount: 8,
      controlEpoch: 3,
      acknowledgedControlEpoch: 3,
      currentStepId: "respond",
      activeWorkflowId: "workflow-1",
      workflowCount: 2,
      lastWorkflowState: GetDebugSession200ResponseLastWorkflowStateEnum.WorkflowCompleted,
      startedAt: "2026-08-17T02:00:00Z",
      capabilities: {
        pause: { supported: false, cooperative: true },
        resume: { supported: true, cooperative: true },
        stepInto: { supported: true, cooperative: true, limitation: "Instrumentation hint only" },
        stepOver: { supported: true, cooperative: true },
        stop: { supported: true, cooperative: false, limitation: "Native process-group authority" },
      },
      limitation: "Process-reported identifiers are not independently proven value-free.",
      events: [{
        id: "event-8",
        workflowId: "workflow-1",
        sequence: 8,
        controlEpoch: 3,
        stepId: "respond",
        kind: GetDebugSession200ResponseEventsInnerKindEnum.Response,
        label: "Return response shape",
        flowStage: GetDebugSession200ResponseEventsInnerFlowStageEnum.Response,
        source: { relativePath: "src/api.rs", line: 88, lineEnd: 91 },
        safePointState: GetDebugSession200ResponseEventsInnerSafePointStateEnum.Paused,
        operation: "serializeResponse",
        resource: "recommendation response",
        dataPreview: {
          kind: GetDebugSession200ResponseEventsInnerDataPreviewKindEnum.Object,
          typeName: "RecommendationResponse",
          fields: [{ name: "recommendation_id", valueType: "uuid", nullable: false }],
          truncated: false,
        },
        timestamp: "2026-08-17T02:00:08Z",
      }],
      eventsTruncated: true,
      eventsStartSequence: 8,
    };

    expect(adaptDebugSession(generated)).toMatchObject({
      status: "paused",
      workflowCount: 2,
      lastWorkflowState: "workflowCompleted",
      eventsTruncated: true,
      capabilities: { stop: { supported: true, cooperative: false } },
      events: [{
        kind: "response",
        flowStage: "response",
        operation: "serializeResponse",
        resource: "recommendation response",
        dataPreview: {
          kind: "object",
          fields: [{ name: "recommendation_id", valueType: "uuid", nullable: false }],
        },
      }],
    });
  });
});
