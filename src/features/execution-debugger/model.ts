import type { RuntimeEvent, SourceLocation } from "../../types";
import type {
  DataShapePreview,
  DebugActionCapabilityView,
  DebugDataFieldView,
  DebugExecutionStep,
  DebugExecutionTrace,
  DebugSessionView,
  DebugStage,
  DebugStepKind,
  DebuggerControlAction,
  DebugWorkflowSummary,
} from "./contracts";

export type * from "./contracts";

const DEBUG_KINDS = new Map<string, DebugStepKind>([
  ["debug.request", "request"],
  ["debug.method", "method"],
  ["debug.line", "line"],
  ["debug.branch", "branch"],
  ["debug.database", "database"],
  ["debug.agent", "agent"],
  ["debug.external", "external"],
  ["debug.response", "response"],
]);

const ALLOWED_STATES = new Set([
  "running",
  "paused",
  "workflowCompleted",
  "workflowFailed",
]);

const ALLOWED_BRANCH_OUTCOMES = new Set([
  "then",
  "else",
  "case",
  "loop",
  "shortCircuit",
  "unknown",
]);

const STAGE_BY_KIND: Record<DebugStepKind, DebugStage> = {
  request: "trigger",
  method: "service",
  line: "service",
  branch: "service",
  database: "data",
  agent: "external",
  external: "external",
  response: "response",
  trace: "service",
  boundary: "trigger",
};

export function debugExecutionStepId(workflowId: string, stepId: string): string {
  return `debug:${workflowId.length}:${workflowId}:${stepId}`;
}

function metadataString(event: RuntimeEvent, key: string): string | undefined {
  const value = event.metadata[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function metadataNumber(event: RuntimeEvent, key: string): number | undefined {
  const value = event.metadata[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function boundedCount(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value >= 0
    ? Math.min(value, Number.MAX_SAFE_INTEGER)
    : undefined;
}

function boundedFields(value: unknown): DebugDataFieldView[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((entry): entry is Record<string, unknown> => Boolean(entry) && typeof entry === "object" && !Array.isArray(entry))
    .map((entry) => ({
      name: typeof entry.name === "string"
        ? entry.name.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 80)
        : "field",
      valueType: typeof entry.valueType === "string"
        ? entry.valueType.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 80)
        : "unknown",
      nullable: entry.nullable === true,
    }))
    .filter((field) => field.name.length > 0)
    .slice(0, 24);
}

export function parseDataShapePreview(value: unknown): DataShapePreview | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) return undefined;
  const candidate = value as Record<string, unknown>;
  const kind = typeof candidate.kind === "string"
    ? candidate.kind.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 48)
    : "shape";
  const typeName = typeof candidate.typeName === "string"
    ? candidate.typeName.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 96)
    : undefined;
  return {
    kind,
    ...(typeName ? { typeName } : {}),
    fields: boundedFields(candidate.fields),
    ...(boundedCount(candidate.itemCount) === undefined
      ? {}
      : { itemCount: boundedCount(candidate.itemCount) }),
    ...(boundedCount(candidate.rowCount) === undefined
      ? {}
      : { rowCount: boundedCount(candidate.rowCount) }),
    ...(boundedCount(candidate.nullCount) === undefined
      ? {}
      : { nullCount: boundedCount(candidate.nullCount) }),
    truncated: candidate.truncated === true,
  };
}

function sourceFromEvent(event: RuntimeEvent): SourceLocation | undefined {
  const relativePath = metadataString(event, "sourceRelativePath");
  const startLine = metadataNumber(event, "sourceLine");
  if (!relativePath || !startLine || startLine < 1 || !Number.isInteger(startLine)) return undefined;
  const sourceLineEnd = metadataNumber(event, "sourceLineEnd");
  const endLine = sourceLineEnd && Number.isInteger(sourceLineEnd) && sourceLineEnd >= startLine
    ? sourceLineEnd
    : startLine;
  return {
    relativePath,
    startLine,
    startColumn: 1,
    endLine,
    endColumn: 1,
  };
}

function safeLabel(event: RuntimeEvent): string {
  return event.label.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 240) || event.kind;
}

function stageForEvent(event: RuntimeEvent, kind: DebugStepKind): DebugStage {
  if (kind === "response") return "response";
  const reported = metadataString(event, "flowStage");
  if (reported && ["trigger", "api", "backend", "service", "data", "external", "response"].includes(reported)) {
    return reported as DebugStage;
  }
  if (event.kind.startsWith("trace.database.")) return "data";
  if (event.kind.startsWith("trace.agent.") || event.kind.startsWith("trace.external.")) return "external";
  if (event.kind.startsWith("trace.http.server.")) {
    return event.kind.endsWith("end") ? "api" : "trigger";
  }
  return STAGE_BY_KIND[kind];
}

function debugStep(event: RuntimeEvent, index: number): DebugExecutionStep | undefined {
  const kind = DEBUG_KINDS.get(event.kind);
  if (!kind) return undefined;
  const sequence = metadataNumber(event, "sequence") ?? index;
  const stepId = metadataString(event, "stepId") ?? event.id;
  const workflowId = metadataString(event, "workflowId") ?? "runtime-history";
  const state = metadataString(event, "safePointState")
    ?? metadataString(event, "debugStatus")
    ?? metadataString(event, "state");
  const branchOutcome = metadataString(event, "branchOutcome");
  return {
    id: debugExecutionStepId(workflowId, stepId),
    reportedStepId: stepId,
    eventIds: [event.id],
    workflowId,
    runId: event.runId,
    debugSessionId: metadataString(event, "debugSessionId"),
    sequence,
    kind,
    stage: stageForEvent(event, kind),
    label: safeLabel(event),
    timestamp: event.timestamp,
    evidence: event.evidence,
    source: sourceFromEvent(event),
    parentStepId: metadataString(event, "parentStepId")
      ? debugExecutionStepId(workflowId, metadataString(event, "parentStepId")!)
      : undefined,
    state: state && ALLOWED_STATES.has(state) ? state : undefined,
    branchOutcome: branchOutcome && ALLOWED_BRANCH_OUTCOMES.has(branchOutcome)
      ? branchOutcome
      : undefined,
    operation: metadataString(event, "operation"),
    resource: metadataString(event, "resource"),
    durationMs: metadataNumber(event, "durationMs"),
    preview: parseDataShapePreview(event.metadata.dataPreview),
    repeatCount: 1,
    protocol: "AONE_DEBUG_V1",
  };
}

function traceStep(event: RuntimeEvent, index: number): DebugExecutionStep | undefined {
  if (!event.kind.startsWith("trace.")) return undefined;
  const phase = metadataString(event, "phase");
  if (phase === "end") return undefined;
  const spanId = metadataString(event, "spanId") ?? event.id;
  const workflowId = event.traceId ? `trace:${event.traceId}` : `trace:${spanId}`;
  const protocol = metadataString(event, "traceProtocol");
  const acceptedProtocol = protocol === "OTLP_HTTP_JSON"
    || protocol === "W3C_TRACE_CONTEXT"
    || protocol === "AONE_TRACE_V1"
    ? protocol
    : "AONE_TRACE_V1";
  return {
    id: `${workflowId}:${spanId}`,
    eventIds: [event.id],
    workflowId,
    runId: event.runId,
    sequence: index,
    kind: "trace",
    stage: stageForEvent(event, "trace"),
    label: safeLabel(event),
    timestamp: event.timestamp,
    evidence: event.evidence,
    source: sourceFromEvent(event),
    parentStepId: metadataString(event, "parentSpanId")
      ? `${workflowId}:${metadataString(event, "parentSpanId")}`
      : undefined,
    state: metadataString(event, "status"),
    durationMs: metadataNumber(event, "durationMs"),
    repeatCount: 1,
    protocol: acceptedProtocol,
  };
}

function boundaryStep(event: RuntimeEvent, index: number): DebugExecutionStep | undefined {
  if (!event.kind.startsWith("http.")) return undefined;
  const responseBoundary = event.kind === "http.response" || event.kind.endsWith("completed");
  const workflowId = event.traceId ? `boundary:${event.traceId}` : `boundary:${event.id}`;
  return {
    id: `boundary:${event.id}`,
    eventIds: [event.id],
    workflowId,
    runId: event.runId,
    sequence: index,
    kind: "boundary",
    stage: responseBoundary ? "api" : "trigger",
    label: safeLabel(event),
    timestamp: event.timestamp,
    evidence: event.evidence,
    durationMs: metadataNumber(event, "durationMs"),
    repeatCount: 1,
    protocol: "boundary",
  };
}

function repeatSignature(step: DebugExecutionStep): string {
  return [
    step.kind,
    step.label,
    step.source?.relativePath ?? "",
    step.source?.startLine ?? "",
    step.parentStepId ?? "",
    step.workflowId ?? "",
    step.branchOutcome ?? "",
  ].join("|");
}

export function collapseRepeatedSteps(steps: DebugExecutionStep[]): DebugExecutionStep[] {
  const collapsed: DebugExecutionStep[] = [];
  for (const step of steps) {
    const previous = collapsed[collapsed.length - 1];
    const collapsible = step.kind === "line" || step.branchOutcome === "loop";
    if (previous && collapsible && repeatSignature(previous) === repeatSignature(step)) {
      collapsed[collapsed.length - 1] = {
        ...previous,
        id: step.id,
        reportedStepId: step.reportedStepId,
        eventIds: [...previous.eventIds, ...step.eventIds],
        repeatCount: previous.repeatCount + step.repeatCount,
        timestamp: step.timestamp,
        sequence: step.sequence,
        state: step.state ?? previous.state,
        operation: step.operation ?? previous.operation,
        resource: step.resource ?? previous.resource,
        preview: step.preview ?? previous.preview,
      };
      continue;
    }
    collapsed.push(step);
  }
  return collapsed;
}

export function buildDebugExecutionTrace(
  events: RuntimeEvent[],
  runId?: string,
  session?: DebugSessionView | null,
): DebugExecutionTrace {
  const startedAt = session?.startedAt ? Date.parse(session.startedAt) : undefined;
  const scoped = events.filter((event) => {
    if (!runId) return true;
    if (event.runId === runId) return true;
    return !event.runId
      && event.kind.startsWith("http.")
      && (startedAt === undefined || Date.parse(event.timestamp) >= startedAt);
  });
  const sessionDebug = (session?.events ?? []).map<DebugExecutionStep>((event) => ({
    id: debugExecutionStepId(event.workflowId, event.stepId),
    reportedStepId: event.stepId,
    eventIds: [event.id],
    workflowId: event.workflowId,
    runId: session?.runId,
    debugSessionId: session?.debugSessionId,
    sequence: event.sequence,
    kind: event.kind,
    stage: event.kind === "response" ? "response" : event.flowStage,
    label: event.label.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 240) || event.kind,
    timestamp: event.timestamp,
    evidence: "observed",
    source: {
      relativePath: event.source.relativePath,
      startLine: event.source.line,
      startColumn: 1,
      endLine: event.source.lineEnd ?? event.source.line,
      endColumn: 1,
    },
    parentStepId: event.parentStepId
      ? debugExecutionStepId(event.workflowId, event.parentStepId)
      : undefined,
    state: event.safePointState,
    branchOutcome: event.branchOutcome,
    operation: event.operation,
    resource: event.resource,
    preview: event.dataPreview ? {
      kind: event.dataPreview.kind,
      typeName: event.dataPreview.typeName,
      fields: event.dataPreview.fields.slice(0, 24),
      itemCount: event.dataPreview.itemCount,
      rowCount: event.dataPreview.rowCount,
      nullCount: event.dataPreview.nullCount,
      truncated: event.dataPreview.truncated,
    } : undefined,
    repeatCount: 1,
    protocol: "AONE_DEBUG_V1",
  }));
  const debug = scoped
    .map(debugStep)
    .filter((step): step is DebugExecutionStep => Boolean(step))
    .sort((left, right) => left.sequence - right.sequence || left.timestamp.localeCompare(right.timestamp));
  const trace = scoped
    .map(traceStep)
    .filter((step): step is DebugExecutionStep => Boolean(step));
  const boundary = scoped
    .map(boundaryStep)
    .filter((step): step is DebugExecutionStep => Boolean(step));
  const canonicalDebug = sessionDebug.length > 0 ? sessionDebug : debug;
  const steps = canonicalDebug.length > 0 || trace.length > 0
    ? [...canonicalDebug, ...trace]
    : boundary;
  return {
    mode: canonicalDebug.length > 0
      ? "debug"
      : trace.length > 0
        ? "trace-only"
        : boundary.length > 0
          ? "boundary-only"
          : "empty",
    steps: collapseRepeatedSteps(steps),
    debugEventCount: canonicalDebug.length,
    traceEventCount: trace.length,
    boundaryEventCount: boundary.length,
    runId,
  };
}

export function criticalExecutionSteps(
  steps: DebugExecutionStep[],
  currentStepId?: string,
): DebugExecutionStep[] {
  return steps.filter((step) =>
    step.kind !== "line"
    || step.id === currentStepId
    || step.repeatCount > 1,
  );
}

export function debugWorkflowSummaries(steps: DebugExecutionStep[]): DebugWorkflowSummary[] {
  const summaries = new Map<string, DebugWorkflowSummary>();
  for (const step of steps) {
    const id = step.workflowId ?? "observed-history";
    const existing = summaries.get(id);
    const terminalState = step.state === "workflowCompleted" || step.state === "workflowFailed"
      ? step.state
      : existing?.terminalState;
    if (existing) {
      existing.lastSequence = Math.max(existing.lastSequence, step.sequence);
      existing.lastTimestamp = step.timestamp > existing.lastTimestamp ? step.timestamp : existing.lastTimestamp;
      existing.eventCount += 1;
      existing.terminalState = terminalState;
      continue;
    }
    summaries.set(id, {
      id,
      firstSequence: step.sequence,
      lastSequence: step.sequence,
      firstTimestamp: step.timestamp,
      lastTimestamp: step.timestamp,
      eventCount: 1,
      label: step.kind === "request" || step.kind === "trace"
        ? step.label
        : `Workflow from #${step.sequence}`,
      terminalState,
    });
  }
  return [...summaries.values()].sort((left, right) => right.lastSequence - left.lastSequence);
}

export function stepsForWorkflow(
  steps: DebugExecutionStep[],
  workflowId?: string,
): DebugExecutionStep[] {
  if (!workflowId) return steps;
  return steps.filter((step) => (step.workflowId ?? "observed-history") === workflowId);
}

export function debuggerCapabilities(session?: DebugSessionView | null): Record<DebuggerControlAction, boolean> {
  if (!session) return { pause: false, resume: false, stepOver: false, stepInto: false, stop: false };
  const cooperative = (capability: DebugActionCapabilityView) => capability.supported && capability.cooperative;
  return {
    pause: session.status === "running" && cooperative(session.capabilities.pause),
    resume: session.status === "paused" && cooperative(session.capabilities.resume),
    stepOver: session.status === "paused" && cooperative(session.capabilities.stepOver),
    stepInto: session.status === "paused" && cooperative(session.capabilities.stepInto),
    stop: ["starting", "running", "paused"].includes(session.status) && session.capabilities.stop.supported,
  };
}
