import { Pulse } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { ApiRequest, ApiResponse, GraphSnapshot, RuntimeEvent, SourceLocation } from "../../types";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../../components/ui/Resizable";
import { ApiClientPane } from "../api-client/ApiClientPane";
import type { ApiClientTarget } from "../api-client/model";
import { useApiClientSession } from "../api-client/useApiClientSession";
import type { EditorOpenMode } from "../editor/model";
import { OtlpReceiverBar } from "../observability/OtlpReceiverBar";
import { useOtlpReceiver } from "../observability/useOtlpReceiver";
import "../../styles/execution-debugger.css";
import "../../styles/execution-debugger-states.css";
import {
  clampStepWindow,
  DEBUG_PAGE_LIMIT,
  latestStepWindow,
  windowContainingStep,
} from "./layout";
import {
  buildDebugExecutionTrace,
  debugExecutionStepId,
  debuggerCapabilities,
  debugWorkflowSummaries,
  stepsForWorkflow,
  type DebuggerControlAction,
  type DebugSessionView,
} from "./model";
import { TraceDataInspector } from "./TraceDataInspector";
import { TracePath } from "./TracePath";
import { TraceToolbar } from "./TraceToolbar";
import { debuggerLiveAnnouncement, useCooperativeBreakpoint } from "./useCooperativeBreakpoint";

interface ExecutionDebuggerProps {
  events: RuntimeEvent[];
  runId?: string;
  observeActive: boolean;
  processActive?: boolean;
  session?: DebugSessionView | null;
  loading?: boolean;
  error?: string | null;
  onControl?: (action: DebuggerControlAction) => Promise<void>;
  workspaceId?: string;
  workspaceGeneration?: number;
  onRuntimeEventsChanged?: () => Promise<void>;
  onOpenSource: (location: SourceLocation, mode?: EditorOpenMode) => void;
  onSendRequest?: (request: ApiRequest) => Promise<ApiResponse>;
  target?: ApiClientTarget | null;
  indexedGraph?: GraphSnapshot;
  indexedRootId?: string;
  indexedGraphLoading?: boolean;
  sourcePane?: ReactNode;
}

interface RequestScope {
  startedAt: number;
  sequenceFloor: number;
  workflowIdsBefore: ReadonlySet<string>;
}

async function unavailableApiTransport(): Promise<ApiResponse> {
  throw new Error("API requests are available in the active Aone workbench.");
}

function sequenceFloor(session?: DebugSessionView | null): number {
  return session?.events.reduce((maximum, event) => Math.max(maximum, event.sequence), 0) ?? 0;
}

export function ExecutionDebugger({
  events,
  runId,
  observeActive,
  processActive = false,
  session = null,
  loading = false,
  error = null,
  onControl,
  workspaceId,
  workspaceGeneration = 0,
  onRuntimeEventsChanged = async () => {},
  onOpenSource,
  onSendRequest = unavailableApiTransport,
  target = null,
  indexedGraph,
  indexedRootId,
  indexedGraphLoading = false,
  sourcePane,
}: ExecutionDebuggerProps) {
  const trace = useMemo(
    () => buildDebugExecutionTrace(events, runId, session),
    [events, runId, session],
  );
  const [requestScope, setRequestScope] = useState<RequestScope | null>(null);
  const [requestedWorkflowId, setRequestedWorkflowId] = useState<string>();
  const [windowStart, setWindowStart] = useState(0);
  const [followLatest, setFollowLatest] = useState(true);
  const [selectedStepId, setSelectedStepId] = useState<string>();
  const [replayPlaying, setReplayPlaying] = useState(false);
  const [pendingAction, setPendingAction] = useState<DebuggerControlAction | null>(null);
  const pendingControlRef = useRef<{
    action: DebuggerControlAction;
    acknowledgedControlEpoch: number;
    controlEpoch: number;
    sequence: number;
  } | null>(null);
  const lastOpenedStepRef = useRef<string | undefined>(undefined);

  useEffect(() => {
    setRequestScope(null);
    setRequestedWorkflowId(undefined);
    setSelectedStepId(undefined);
    setFollowLatest(true);
  }, [target?.id, target?.method, target?.path, target?.protocol, workspaceGeneration, workspaceId]);

  const sendRequest = useCallback(async (request: ApiRequest): Promise<ApiResponse> => {
    setRequestScope({
      startedAt: Date.now(),
      sequenceFloor: sequenceFloor(session),
      workflowIdsBefore: new Set(
        trace.steps.flatMap((step) => step.workflowId ? [step.workflowId] : []),
      ),
    });
    setRequestedWorkflowId(undefined);
    setSelectedStepId(undefined);
    setWindowStart(0);
    setFollowLatest(true);
    setReplayPlaying(false);
    return onSendRequest(request);
  }, [onSendRequest, session, trace.steps]);
  const apiClient = useApiClientSession({
    onSendRequest: sendRequest,
    debugActive: observeActive,
    target,
    scopeKey: JSON.stringify([workspaceId ?? null, workspaceGeneration]),
  });

  const scopedTraceSteps = useMemo(() => {
    if (target && !requestScope) return [];
    if (!requestScope) return trace.steps;
    return trace.steps.filter((step) => {
      const timestamp = Date.parse(step.timestamp);
      if (Number.isFinite(timestamp) && timestamp < requestScope.startedAt) return false;
      if (step.protocol !== "AONE_DEBUG_V1") return true;
      return step.sequence > requestScope.sequenceFloor
        && (!step.workflowId || !requestScope.workflowIdsBefore.has(step.workflowId));
    });
  }, [requestScope, target, trace.steps]);
  const workflows = useMemo(() => debugWorkflowSummaries(scopedTraceSteps), [scopedTraceSteps]);
  const activeWorkflowExists = workflows.some((workflow) => workflow.id === session?.activeWorkflowId);
  const requestedWorkflowExists = workflows.some((workflow) => workflow.id === requestedWorkflowId);
  const workflowId = requestedWorkflowExists
    ? requestedWorkflowId
    : activeWorkflowExists
      ? session?.activeWorkflowId
      : workflows[0]?.id;
  const allSteps = useMemo(
    () => stepsForWorkflow(scopedTraceSteps, workflowId),
    [scopedTraceSteps, workflowId],
  );
  const currentExecutionStepId = session?.activeWorkflowId
    && session.currentStepId
    && workflowId === session.activeWorkflowId
    ? debugExecutionStepId(session.activeWorkflowId, session.currentStepId)
    : undefined;
  const currentStep = allSteps.find((step) => step.id === currentExecutionStepId);
  const latestWindow = useMemo(() => latestStepWindow(allSteps), [allSteps]);
  const stepWindow = useMemo(
    () => followLatest ? latestWindow : clampStepWindow(allSteps, windowStart),
    [allSteps, followLatest, latestWindow, windowStart],
  );
  const selectedStep = allSteps.find((step) => step.id === selectedStepId)
    ?? currentStep
    ?? allSteps[allSteps.length - 1];
  const selectedIndex = selectedStep ? allSteps.findIndex((step) => step.id === selectedStep.id) : -1;
  const previousStep = selectedIndex > 0 ? allSteps[selectedIndex - 1] : undefined;
  const sessionLive = Boolean(
    processActive
    && session
    && ["starting", "running", "paused"].includes(session.status),
  );
  const selectedWorkflowIsLive = sessionLive
    && (!workflowId || workflowId === session?.activeWorkflowId);
  const replay = !selectedWorkflowIsLive;

  const selectStep = useCallback((stepId: string) => {
    setFollowLatest(false);
    setSelectedStepId(stepId);
    const containing = windowContainingStep(allSteps, stepId);
    if (containing.start !== stepWindow.start) setWindowStart(containing.start);
  }, [allSteps, stepWindow.start]);

  const latestFollowStep = currentStep ?? allSteps[allSteps.length - 1];
  useEffect(() => {
    if (!followLatest || !latestFollowStep) return;
    setSelectedStepId(latestFollowStep.id);
    const containing = windowContainingStep(allSteps, latestFollowStep.id);
    if (containing.start !== stepWindow.start) setWindowStart(containing.start);
  }, [allSteps, followLatest, latestFollowStep, stepWindow.start]);

  useEffect(() => {
    if (selectedStepId && allSteps.some((step) => step.id === selectedStepId)) return;
    setSelectedStepId(currentStep?.id ?? allSteps[allSteps.length - 1]?.id);
  }, [allSteps, currentStep?.id, selectedStepId]);

  useEffect(() => {
    if (!selectedStep?.source || lastOpenedStepRef.current === selectedStep.id) return;
    lastOpenedStepRef.current = selectedStep.id;
    onOpenSource(selectedStep.source, "preview");
  }, [onOpenSource, selectedStep]);

  useEffect(() => {
    if (!replayPlaying || !replay) return;
    if (selectedIndex < 0 || selectedIndex >= allSteps.length - 1) {
      setReplayPlaying(false);
      return;
    }
    const timer = window.setTimeout(() => selectStep(allSteps[selectedIndex + 1]!.id), 650);
    return () => window.clearTimeout(timer);
  }, [allSteps, replay, replayPlaying, selectStep, selectedIndex]);

  useEffect(() => {
    setReplayPlaying(false);
  }, [workflowId]);

  const runControl = useCallback((action: DebuggerControlAction, onRejected?: () => void) => {
    const available = action === "stop" && processActive && session?.capabilities.stop.supported
      ? true
      : debuggerCapabilities(session)[action];
    if (!session || !onControl || !available || pendingAction || session.pendingAction) return;
    pendingControlRef.current = {
      action,
      acknowledgedControlEpoch: session.acknowledgedControlEpoch,
      controlEpoch: session.controlEpoch,
      sequence: session.events[session.events.length - 1]?.sequence ?? 0,
    };
    setPendingAction(action);
    void onControl(action).catch(() => {
      pendingControlRef.current = null;
      setPendingAction(null);
      onRejected?.();
    });
  }, [onControl, pendingAction, processActive, session]);

  useEffect(() => {
    const pending = pendingControlRef.current;
    if (!pending || !session) return;
    const sequence = session.events[session.events.length - 1]?.sequence ?? 0;
    const terminal = ["completed", "failed", "stopped"].includes(session.status);
    const controlAcknowledged = session.acknowledgedControlEpoch > pending.acknowledgedControlEpoch;
    const terminalAcknowledged = terminal && session.controlEpoch > pending.controlEpoch;
    const processStopObserved = pending.action === "stop" && (!processActive || terminal);
    const acknowledged = processStopObserved || session.pendingAction === undefined
      && (controlAcknowledged || terminalAcknowledged)
      && (sequence > pending.sequence || terminal);
    if (!acknowledged) return;
    pendingControlRef.current = null;
    setPendingAction(null);
  }, [processActive, session]);

  const breakpointController = useCooperativeBreakpoint({
    runId,
    workspaceGeneration,
    session,
    processActive,
    pendingAction,
    selectedStep,
    runControl,
    onArm: () => undefined,
  });
  const runToolbarControl = breakpointController.runToolbarControl;

  useEffect(() => {
    if (replay && !processActive) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const element = event.target;
      if (element instanceof Element && element.closest("input, textarea, select, [contenteditable='true'], .monaco-editor")) return;
      const capabilities = debuggerCapabilities(session);
      const action = event.key === "F6"
        ? session?.status === "paused" ? "resume" : "pause"
        : event.key === "F10"
          ? "stepOver"
          : event.key === "F11"
            ? "stepInto"
            : event.key === "F5" && event.shiftKey
              ? "stop"
              : undefined;
      const available = action === "stop" && processActive && session?.capabilities.stop.supported
        ? true
        : action ? capabilities[action] : false;
      if (!action || !available || session?.pendingAction || pendingAction) return;
      event.preventDefault();
      runToolbarControl(action);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pendingAction, processActive, replay, runToolbarControl, session]);

  const previous = useCallback(() => {
    setReplayPlaying(false);
    if (selectedIndex > 0) selectStep(allSteps[selectedIndex - 1]!.id);
  }, [allSteps, selectStep, selectedIndex]);
  const hasNextSelection = selectedIndex >= 0 && selectedIndex < allSteps.length - 1;
  const canStepLive = Boolean(
    selectedWorkflowIsLive
    && session?.status === "paused"
    && debuggerCapabilities(session).stepOver,
  );
  const next = useCallback(() => {
    setReplayPlaying(false);
    if (hasNextSelection) {
      selectStep(allSteps[selectedIndex + 1]!.id);
      return;
    }
    if (canStepLive) runToolbarControl("stepOver");
  }, [allSteps, canStepLive, hasNextSelection, runToolbarControl, selectStep, selectedIndex]);
  const playReplay = useCallback(() => {
    if (selectedIndex >= allSteps.length - 1 && allSteps[0]) selectStep(allSteps[0].id);
    setReplayPlaying(allSteps.length > 1);
  }, [allSteps, selectStep, selectedIndex]);

  const otlpReceiver = useOtlpReceiver({
    workspaceId,
    workspaceGeneration,
    onRuntimeEventsChanged,
  });
  const selectedTraceId = workflowId?.startsWith("trace:")
    ? workflowId.slice("trace:".length)
    : undefined;
  const liveAnnouncement = debuggerLiveAnnouncement({
    breakpoint: breakpointController.breakpoint,
    breakpointReached: breakpointController.reached,
    breakpointAdvancing: breakpointController.advancing,
    breakpointLocation: breakpointController.location,
    pendingAction,
    session,
    processActive,
    currentStep,
  });

  return (
    <section className="execution-debugger" aria-label="API trace workbench" aria-busy={loading}>
      <TraceToolbar
        session={session}
        replay={replay}
        processActive={processActive}
        pendingAction={pendingAction}
        workflows={workflows}
        workflowId={workflowId}
        onWorkflow={(id) => {
          setReplayPlaying(false);
          setRequestedWorkflowId(id);
          setFollowLatest(true);
          setSelectedStepId(undefined);
        }}
        selectedIndex={selectedIndex}
        stepCount={allSteps.length}
        hasPrevious={selectedIndex > 0}
        canAdvance={hasNextSelection || canStepLive}
        nextLabel={hasNextSelection ? "Next event" : "Next safe point"}
        onPrevious={previous}
        onNext={next}
        replayPlaying={replayPlaying}
        onReplayPlay={playReplay}
        onReplayPause={() => setReplayPlaying(false)}
        onControl={runToolbarControl}
        requestScoped={requestScope !== null}
      />

      <ResizablePanelGroup
        orientation="horizontal"
        id="aone-api-trace-layout"
        className="trace-workbench-layout"
        defaultLayout={{
          "trace-api-panel": 21,
          "trace-source-panel": 36,
          "trace-path-panel": 24,
          "trace-data-panel": 19,
        }}
      >
        <ResizablePanel id="trace-api-panel" defaultSize="21%" minSize="200px" className="trace-workbench-panel">
          <section className="trace-api-pane" aria-label="API client">
            <header className="trace-pane-heading">
              <span>API client</span>
              <small>{target?.path ?? "Local request"}</small>
            </header>
            <div className="trace-api-scroll">
              <ApiClientPane session={apiClient} debugActive={observeActive} responseMode="none" compact />
              <details className="trace-otlp-disclosure">
                <summary><Pulse size={13} aria-hidden="true" /> OTLP input</summary>
                <OtlpReceiverBar
                  snapshot={otlpReceiver.snapshot}
                  available={otlpReceiver.available}
                  startEnabled={observeActive}
                  loading={otlpReceiver.loading}
                  error={otlpReceiver.error}
                  selectedTraceId={selectedTraceId}
                  onStart={otlpReceiver.start}
                  onStop={otlpReceiver.stop}
                  onDeleteTrace={otlpReceiver.deleteTrace}
                />
              </details>
            </div>
          </section>
        </ResizablePanel>
        <ResizableHandle aria-label="Resize API client and source editor" />
        <ResizablePanel id="trace-source-panel" defaultSize="36%" minSize="300px" className="trace-workbench-panel">
          {sourcePane ?? <div className="trace-pane-state">Select a safe point with source to open the editor.</div>}
        </ResizablePanel>
        <ResizableHandle aria-label="Resize source editor and execution path" />
        <ResizablePanel id="trace-path-panel" defaultSize="24%" minSize="230px" className="trace-workbench-panel">
          <TracePath
            allSteps={allSteps}
            stepWindow={stepWindow}
            selectedStepId={selectedStep?.id}
            currentStepId={replay ? selectedStep?.id : currentExecutionStepId}
            replay={replay}
            indexedGraph={indexedGraph}
            indexedRootId={indexedRootId}
            indexedGraphLoading={indexedGraphLoading}
            error={error}
            onSelect={selectStep}
            onOpenSource={onOpenSource}
            onEarlier={() => {
              setFollowLatest(false);
              setWindowStart(Math.max(0, stepWindow.start - DEBUG_PAGE_LIMIT));
            }}
            onLater={() => {
              const nextStart = Math.min(
                Math.max(0, allSteps.length - DEBUG_PAGE_LIMIT),
                stepWindow.start + DEBUG_PAGE_LIMIT,
              );
              setWindowStart(nextStart);
              setFollowLatest(nextStart + DEBUG_PAGE_LIMIT >= allSteps.length);
            }}
          />
        </ResizablePanel>
        <ResizableHandle aria-label="Resize execution path and trace data" />
        <ResizablePanel id="trace-data-panel" defaultSize="19%" minSize="200px" className="trace-workbench-panel">
          <TraceDataInspector
            step={selectedStep}
            previousStep={previousStep}
            request={apiClient.lastRequest}
            response={apiClient.response}
            requestError={apiClient.requestError}
            sending={apiClient.sending}
            onOpenSource={onOpenSource}
            breakpointArmed={breakpointController.selectedArmed}
            onToggleBreakpoint={breakpointController.toggle}
          />
        </ResizablePanel>
      </ResizablePanelGroup>

      <p className="visually-hidden" aria-live="polite" aria-atomic="true">{liveAnnouncement}</p>
    </section>
  );
}
