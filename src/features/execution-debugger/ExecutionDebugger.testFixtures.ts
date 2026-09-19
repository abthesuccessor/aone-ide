import type { DebugSessionView, DebugWorkflowEventView } from "./contracts";

function capability(supported = true, cooperative = true) {
  return { supported, cooperative };
}

export function event(
  sequence: number,
  overrides: Partial<DebugWorkflowEventView> = {},
): DebugWorkflowEventView {
  return {
    id: `event-${sequence}`,
    workflowId: "workflow-active",
    sequence,
    controlEpoch: 2,
    stepId: `step-${sequence}`,
    kind: sequence === 1 ? "request" : "method",
    label: sequence === 1 ? "POST /api/recommendations" : `Resolve recommendation ${sequence}`,
    flowStage: sequence === 1 ? "trigger" : "service",
    source: { relativePath: "src/service.rs", line: 40 + sequence },
    safePointState: "paused",
    timestamp: `2026-08-17T02:00:${String(sequence % 60).padStart(2, "0")}Z`,
    ...overrides,
  };
}

export function session(
  events: DebugWorkflowEventView[],
  overrides: Partial<DebugSessionView> = {},
): DebugSessionView {
  return {
    debugSessionId: "debug-session",
    runId: "run-debug",
    status: "paused",
    eventCount: events.length,
    controlEpoch: 2,
    acknowledgedControlEpoch: 2,
    currentStepId: events.at(-1)?.stepId,
    activeWorkflowId: events.at(-1)?.workflowId,
    workflowCount: 1,
    startedAt: "2026-08-17T02:00:00Z",
    capabilities: {
      pause: capability(),
      resume: capability(),
      stepInto: capability(),
      stepOver: capability(),
      stop: capability(true, false),
    },
    limitation: "Other threads and requests may continue while this workflow is paused.",
    events,
    eventsTruncated: false,
    ...overrides,
  };
}
