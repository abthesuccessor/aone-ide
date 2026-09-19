import { useCallback, useEffect, useRef, useState } from "react";
import type { SourceLocation } from "../../types";
import type {
  DebugExecutionStep,
  DebuggerControlAction,
  DebugSessionView,
} from "./model";

export interface CooperativeBreakpoint {
  debugSessionId: string;
  workflowId: string;
  reportedStepId: string;
  label: string;
  source: SourceLocation;
}

type ControlRunner = (action: DebuggerControlAction, onRejected?: () => void) => void;

interface BreakpointOptions {
  runId?: string;
  workspaceGeneration: number;
  session?: DebugSessionView | null;
  processActive: boolean;
  pendingAction: DebuggerControlAction | null;
  selectedStep?: DebugExecutionStep;
  runControl: ControlRunner;
  onArm: () => void;
}

export function useCooperativeBreakpoint({
  runId,
  workspaceGeneration,
  session,
  processActive,
  pendingAction,
  selectedStep,
  runControl,
  onArm,
}: BreakpointOptions) {
  const [breakpoint, setBreakpoint] = useState<CooperativeBreakpoint>();
  const [advancing, setAdvancing] = useState(false);
  const advanceRef = useRef<string | undefined>(undefined);
  const reached = Boolean(
    breakpoint
    && session?.debugSessionId === breakpoint.debugSessionId
    && session.activeWorkflowId === breakpoint.workflowId
    && session.currentStepId === breakpoint.reportedStepId,
  );
  const location = breakpoint
    ? `${breakpoint.source.relativePath}:${breakpoint.source.startLine}`
    : undefined;
  const canToggle = Boolean(
    selectedStep?.protocol === "AONE_DEBUG_V1"
    && selectedStep.reportedStepId
    && selectedStep.source
    && selectedStep.workflowId
    && processActive
    && session?.debugSessionId
    && session.activeWorkflowId === selectedStep.workflowId
    && session.capabilities.stepOver.supported,
  );

  useEffect(() => {
    setBreakpoint(undefined);
    setAdvancing(false);
    advanceRef.current = undefined;
  }, [runId, session?.debugSessionId, workspaceGeneration]);

  const runToolbarControl = useCallback((action: DebuggerControlAction) => {
    if (!(
      action === "resume"
      && breakpoint
      && session?.status === "paused"
      && session.debugSessionId === breakpoint.debugSessionId
      && session.activeWorkflowId === breakpoint.workflowId
      && session.capabilities.stepOver.supported
    )) {
      runControl(action);
      return;
    }
    if (session.currentStepId === breakpoint.reportedStepId) {
      setAdvancing(false);
      return;
    }
    advanceRef.current = `${session.debugSessionId}:${session.controlEpoch}:${session.currentStepId ?? "none"}`;
    setAdvancing(true);
    runControl("stepOver", () => setAdvancing(false));
  }, [breakpoint, runControl, session]);

  useEffect(() => {
    if (!advancing) return;
    if (!breakpoint || !session || !processActive) {
      setAdvancing(false);
      return;
    }
    const terminal = ["completed", "failed", "stopped"].includes(session.status);
    const targetChanged = session.debugSessionId !== breakpoint.debugSessionId
      || session.activeWorkflowId !== breakpoint.workflowId;
    if (terminal || targetChanged || session.currentStepId === breakpoint.reportedStepId) {
      setAdvancing(false);
      return;
    }
    if (session.status !== "paused" || !session.capabilities.stepOver.supported || pendingAction || session.pendingAction) return;
    const acknowledgementKey = `${session.debugSessionId}:${session.controlEpoch}:${session.currentStepId ?? "none"}`;
    if (advanceRef.current === acknowledgementKey) return;
    advanceRef.current = acknowledgementKey;
    runControl("stepOver", () => setAdvancing(false));
  }, [advancing, breakpoint, pendingAction, processActive, runControl, session]);

  const toggle = useCallback(() => {
    if (!canToggle || !selectedStep?.reportedStepId || !selectedStep.source || !selectedStep.workflowId || !session) return;
    const same = breakpoint?.workflowId === selectedStep.workflowId
      && breakpoint.reportedStepId === selectedStep.reportedStepId;
    setAdvancing(false);
    advanceRef.current = undefined;
    setBreakpoint(same ? undefined : {
      debugSessionId: session.debugSessionId,
      workflowId: selectedStep.workflowId,
      reportedStepId: selectedStep.reportedStepId,
      label: selectedStep.label,
      source: selectedStep.source,
    });
    if (!same) onArm();
  }, [breakpoint, canToggle, onArm, selectedStep, session]);

  return {
    breakpoint,
    advancing,
    reached,
    location,
    runToolbarControl,
    selectedArmed: Boolean(
      breakpoint
      && selectedStep?.workflowId === breakpoint.workflowId
      && selectedStep.reportedStepId === breakpoint.reportedStepId
    ),
    toggle: canToggle ? toggle : undefined,
  };
}

export function debuggerLiveAnnouncement({
  breakpoint,
  breakpointReached,
  breakpointAdvancing,
  breakpointLocation,
  pendingAction,
  session,
  processActive,
  currentStep,
}: {
  breakpoint?: CooperativeBreakpoint;
  breakpointReached: boolean;
  breakpointAdvancing: boolean;
  breakpointLocation?: string;
  pendingAction: DebuggerControlAction | null;
  session?: DebugSessionView | null;
  processActive: boolean;
  currentStep?: DebugExecutionStep;
}): string {
  const announcedAction = pendingAction ?? session?.pendingAction;
  if (breakpointReached) return `Cooperative breakpoint reached at ${breakpointLocation}.`;
  if (breakpointAdvancing && breakpoint) return `Advancing through acknowledged safe points toward ${breakpointLocation}.`;
  if (announcedAction === "pause") return "Pause requested at the next cooperative safe point.";
  if (announcedAction === "resume") return "Continue requested.";
  if (announcedAction === "stepOver" || announcedAction === "stepInto") return "Safe-point step requested.";
  if (announcedAction === "stop") return "Native process-group stop requested.";
  if (session && !processActive && ["starting", "running", "paused"].includes(session.status)) {
    return "Managed process lifecycle changed. Refreshing final status.";
  }
  if (session?.status === "paused" && currentStep) {
    const source = currentStep.source
      ? `${currentStep.source.relativePath.split("/").pop()}:${currentStep.source.startLine}`
      : "No source candidate reported";
    return `Paused at acknowledged safe point ${currentStep.label}, ${source}.`;
  }
  if (session?.status === "completed") return "Debug session ended. History replay is available.";
  if (session?.status === "failed") return processActive
    ? "Debug protocol failed. The managed process may still be running."
    : "Debug protocol failed. History replay is available.";
  if (session?.status === "stopped") return "Managed leader exit observed after stop request. History replay is available.";
  return "";
}
