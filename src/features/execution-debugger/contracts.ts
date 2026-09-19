import type { EvidenceKind, SourceLocation } from "../../types";

export type DebuggerStatus =
  | "starting"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "stopped";

export type DebugSafePointState =
  | "running"
  | "paused"
  | "workflowCompleted"
  | "workflowFailed";

export type DebuggerControlAction =
  | "pause"
  | "resume"
  | "stepOver"
  | "stepInto"
  | "stop";

export interface DebugActionCapabilityView {
  supported: boolean;
  cooperative: boolean;
  limitation?: string;
}

export interface DebuggerCapabilitiesView {
  pause: DebugActionCapabilityView;
  resume: DebugActionCapabilityView;
  stepInto: DebugActionCapabilityView;
  stepOver: DebugActionCapabilityView;
  stop: DebugActionCapabilityView;
}

export interface DebugSourceView {
  relativePath: string;
  line: number;
  lineEnd?: number;
}

export interface DebugDataFieldView {
  name: string;
  valueType: string;
  nullable: boolean;
}

export interface DebugDataPreviewView {
  kind: "none" | "scalar" | "object" | "array" | "rowSet";
  typeName?: string;
  fields: DebugDataFieldView[];
  itemCount?: number;
  rowCount?: number;
  nullCount?: number;
  truncated: boolean;
}

export interface DebugWorkflowEventView {
  id: string;
  workflowId: string;
  sequence: number;
  controlEpoch: number;
  stepId: string;
  parentStepId?: string;
  kind: "request" | "method" | "line" | "branch" | "database" | "agent" | "external" | "response";
  label: string;
  flowStage: "trigger" | "api" | "backend" | "service" | "data" | "external" | "response";
  source: DebugSourceView;
  safePointState: DebugSafePointState;
  branchOutcome?: "then" | "else" | "case" | "loop" | "shortCircuit" | "unknown";
  operation?: string;
  resource?: string;
  dataPreview?: DebugDataPreviewView;
  timestamp: string;
}

export interface DebugSessionView {
  debugSessionId: string;
  runId: string;
  status: DebuggerStatus;
  eventCount: number;
  controlEpoch: number;
  acknowledgedControlEpoch: number;
  currentStepId?: string;
  currentSource?: DebugSourceView;
  pendingAction?: DebuggerControlAction;
  pendingControlEpoch?: number;
  activeWorkflowId?: string;
  workflowCount: number;
  lastWorkflowState?: DebugSafePointState;
  startedAt: string;
  endedAt?: string;
  error?: string;
  capabilities: DebuggerCapabilitiesView;
  limitation: string;
  events: DebugWorkflowEventView[];
  eventsTruncated: boolean;
  eventsStartSequence?: number;
}

export type DebugStepKind =
  | "request"
  | "method"
  | "line"
  | "branch"
  | "database"
  | "agent"
  | "external"
  | "response"
  | "trace"
  | "boundary";

export type DebugStage = "trigger" | "api" | "backend" | "service" | "data" | "external" | "response";

export interface DataShapePreview {
  kind: string;
  typeName?: string;
  fields: DebugDataFieldView[];
  itemCount?: number;
  rowCount?: number;
  nullCount?: number;
  truncated: boolean;
}

export interface DebugExecutionStep {
  id: string;
  reportedStepId?: string;
  eventIds: string[];
  workflowId?: string;
  runId?: string;
  debugSessionId?: string;
  sequence: number;
  kind: DebugStepKind;
  stage: DebugStage;
  label: string;
  timestamp: string;
  evidence: EvidenceKind;
  source?: SourceLocation;
  parentStepId?: string;
  state?: DebugSafePointState | string;
  branchOutcome?: string;
  operation?: string;
  resource?: string;
  durationMs?: number;
  preview?: DataShapePreview;
  repeatCount: number;
  protocol:
    | "AONE_DEBUG_V1"
    | "AONE_TRACE_V1"
    | "OTLP_HTTP_JSON"
    | "W3C_TRACE_CONTEXT"
    | "boundary";
}

export type DebugTraceMode = "debug" | "trace-only" | "boundary-only" | "empty";

export interface DebugExecutionTrace {
  mode: DebugTraceMode;
  steps: DebugExecutionStep[];
  debugEventCount: number;
  traceEventCount: number;
  boundaryEventCount: number;
  runId?: string;
}

export interface DebugWorkflowSummary {
  id: string;
  firstSequence: number;
  lastSequence: number;
  firstTimestamp: string;
  lastTimestamp: string;
  eventCount: number;
  label: string;
  terminalState?: "workflowCompleted" | "workflowFailed";
}
