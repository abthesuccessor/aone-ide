import type {
  ControlDebugSession200ResponseSnapshot,
  GetDebugSession200Response,
  GetDebugSession200ResponseCapabilitiesPause,
  GetDebugSession200ResponseCurrentSource,
  GetDebugSession200ResponseEventsInner,
  GetDebugSession200ResponseEventsInnerDataPreview,
} from "../../generated/ipc/models";
import type {
  DebugActionCapabilityView,
  DebugDataPreviewView,
  DebugSafePointState,
  DebugSessionView,
  DebugWorkflowEventView,
  DebuggerControlAction,
  DebuggerStatus,
} from "./contracts";

type GeneratedSnapshot = GetDebugSession200Response | ControlDebugSession200ResponseSnapshot;
type GeneratedState = GetDebugSession200ResponseEventsInner["safePointState"]
  | NonNullable<GeneratedSnapshot["lastWorkflowState"]>;

function unreachable(value: never): never {
  throw new Error(`Unsupported generated debugger enum: ${String(value)}`);
}

function statusView(value: GeneratedSnapshot["status"]): DebuggerStatus {
  switch (value) {
    case "starting": return "starting";
    case "running": return "running";
    case "paused": return "paused";
    case "completed": return "completed";
    case "failed": return "failed";
    case "stopped": return "stopped";
    default: return unreachable(value);
  }
}

function actionView(value: NonNullable<GeneratedSnapshot["pendingAction"]>): DebuggerControlAction {
  switch (value) {
    case "pause": return "pause";
    case "resume": return "resume";
    case "stepInto": return "stepInto";
    case "stepOver": return "stepOver";
    case "stop": return "stop";
    default: return unreachable(value);
  }
}

function stateView(value: GeneratedState): DebugSafePointState {
  switch (value) {
    case "running": return "running";
    case "paused": return "paused";
    case "workflowCompleted": return "workflowCompleted";
    case "workflowFailed": return "workflowFailed";
    default: return unreachable(value);
  }
}

function sourceView(value: GetDebugSession200ResponseCurrentSource) {
  return {
    relativePath: value.relativePath,
    line: value.line,
    ...(value.lineEnd === undefined ? {} : { lineEnd: value.lineEnd }),
  };
}

function capabilityView(value: GetDebugSession200ResponseCapabilitiesPause): DebugActionCapabilityView {
  return {
    supported: value.supported,
    cooperative: value.cooperative,
    ...(value.limitation === undefined ? {} : { limitation: value.limitation }),
  };
}

function previewView(value: GetDebugSession200ResponseEventsInnerDataPreview): DebugDataPreviewView {
  switch (value.kind) {
    case "none":
    case "scalar":
    case "object":
    case "array":
    case "rowSet":
      break;
    default:
      return unreachable(value.kind);
  }
  return {
    kind: value.kind,
    ...(value.typeName === undefined ? {} : { typeName: value.typeName }),
    fields: value.fields.map((field) => ({
      name: field.name,
      valueType: field.valueType,
      nullable: field.nullable,
    })),
    ...(value.itemCount === undefined ? {} : { itemCount: value.itemCount }),
    ...(value.rowCount === undefined ? {} : { rowCount: value.rowCount }),
    ...(value.nullCount === undefined ? {} : { nullCount: value.nullCount }),
    truncated: value.truncated,
  };
}

function eventView(value: GetDebugSession200ResponseEventsInner): DebugWorkflowEventView {
  switch (value.kind) {
    case "request":
    case "method":
    case "line":
    case "branch":
    case "database":
    case "agent":
    case "external":
    case "response":
      break;
    default:
      return unreachable(value.kind);
  }
  switch (value.flowStage) {
    case "trigger":
    case "api":
    case "backend":
    case "service":
    case "data":
    case "external":
    case "response":
      break;
    default:
      return unreachable(value.flowStage);
  }
  return {
    id: value.id,
    workflowId: value.workflowId,
    sequence: value.sequence,
    controlEpoch: value.controlEpoch,
    stepId: value.stepId,
    ...(value.parentStepId === undefined ? {} : { parentStepId: value.parentStepId }),
    kind: value.kind,
    label: value.label,
    flowStage: value.flowStage,
    source: sourceView(value.source),
    safePointState: stateView(value.safePointState),
    ...(value.branchOutcome === undefined ? {} : { branchOutcome: value.branchOutcome }),
    ...(value.operation === undefined ? {} : { operation: value.operation }),
    ...(value.resource === undefined ? {} : { resource: value.resource }),
    ...(value.dataPreview === undefined ? {} : { dataPreview: previewView(value.dataPreview) }),
    timestamp: value.timestamp,
  };
}

export function adaptDebugSession(value: GeneratedSnapshot): DebugSessionView {
  return {
    debugSessionId: value.debugSessionId,
    runId: value.runId,
    status: statusView(value.status),
    eventCount: value.eventCount,
    controlEpoch: value.controlEpoch,
    acknowledgedControlEpoch: value.acknowledgedControlEpoch,
    ...(value.currentStepId === undefined ? {} : { currentStepId: value.currentStepId }),
    ...(value.currentSource === undefined ? {} : { currentSource: sourceView(value.currentSource) }),
    ...(value.pendingAction === undefined ? {} : { pendingAction: actionView(value.pendingAction) }),
    ...(value.pendingControlEpoch === undefined ? {} : { pendingControlEpoch: value.pendingControlEpoch }),
    ...(value.activeWorkflowId === undefined ? {} : { activeWorkflowId: value.activeWorkflowId }),
    workflowCount: value.workflowCount,
    ...(value.lastWorkflowState === undefined ? {} : { lastWorkflowState: stateView(value.lastWorkflowState) }),
    startedAt: value.startedAt,
    ...(value.endedAt === undefined ? {} : { endedAt: value.endedAt }),
    ...(value.error === undefined ? {} : { error: value.error }),
    capabilities: {
      pause: capabilityView(value.capabilities.pause),
      resume: capabilityView(value.capabilities.resume),
      stepInto: capabilityView(value.capabilities.stepInto),
      stepOver: capabilityView(value.capabilities.stepOver),
      stop: capabilityView(value.capabilities.stop),
    },
    limitation: value.limitation,
    events: value.events.map(eventView),
    eventsTruncated: value.eventsTruncated,
    ...(value.eventsStartSequence === undefined ? {} : { eventsStartSequence: value.eventsStartSequence }),
  };
}
