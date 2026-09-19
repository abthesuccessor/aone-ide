import {
  ArrowBendDownRight,
  ArrowCounterClockwise,
  ArrowRight,
  CaretLeft,
  CaretRight,
  CrosshairSimple,
  Database,
  Globe,
  Minus,
  Pause,
  Play,
  Plus,
  Robot,
  SkipForward,
  Stop,
  Warning,
} from "@phosphor-icons/react";
import type { CSSProperties, ReactNode } from "react";
import { EvidenceBadge } from "../../components/EvidenceBadge";
import type { SourceLocation } from "../../types";
import { DEBUG_LANES } from "./layout";
import {
  debuggerCapabilities,
  type DataShapePreview,
  type DebugExecutionStep,
  type DebuggerControlAction,
  type DebugSessionView,
  type DebugWorkflowSummary,
} from "./model";

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() ?? path;
}

export function sourceLabel(step: DebugExecutionStep): string {
  return step.source
    ? `${basename(step.source.relativePath)}:${step.source.startLine}`
    : "No source candidate reported";
}

export function stepTypeLabel(step: DebugExecutionStep): string {
  if (step.protocol === "OTLP_HTTP_JSON") return "Observed OTLP span";
  if (step.protocol === "W3C_TRACE_CONTEXT") return "Observed API span";
  if (step.protocol === "AONE_TRACE_V1") return "Observed span";
  if (step.protocol === "boundary") return "Observed boundary";
  return step.kind === "line" ? "Safe point" : step.kind;
}

export function stageIcon(step: DebugExecutionStep): ReactNode {
  if (step.kind === "database") return <Database size={13} weight="duotone" />;
  if (step.kind === "agent") return <Robot size={13} weight="duotone" />;
  if (step.kind === "external") return <Globe size={13} weight="duotone" />;
  if (step.kind === "branch") return <ArrowBendDownRight size={13} weight="duotone" />;
  return <ArrowRight size={13} weight="duotone" />;
}

function safeShapeSummary(preview: DataShapePreview): string[] {
  const parts: string[] = [];
  if (preview.typeName) parts.push(`Type: ${preview.typeName}`);
  if (preview.rowCount !== undefined) parts.push(`Rows: ${preview.rowCount}`);
  if (preview.itemCount !== undefined) parts.push(`Items: ${preview.itemCount}`);
  if (preview.nullCount !== undefined) parts.push(`Null fields: ${preview.nullCount}`);
  if (preview.fields.length > 0) {
    parts.push(`Fields: ${preview.fields.map((field) => `${field.name}: ${field.valueType}${field.nullable ? "?" : ""}`).join(", ")}`);
  }
  if (preview.truncated) parts.push("Shape preview was truncated");
  return parts;
}

export function DebuggerLoading() {
  return (
    <section className="execution-debugger debugger-loading" aria-label="Instrumented Debug" aria-busy="true">
      <header className="debugger-toolbar">
        <span className="debugger-skeleton-bar is-wide" />
        <span className="debugger-skeleton-bar" />
        <span className="debugger-skeleton-bar" />
      </header>
      <div className="debugger-skeleton-canvas" role="status" aria-live="polite">
        <span className="visually-hidden">Loading instrumented debug session</span>
        {DEBUG_LANES.map((lane, laneIndex) => (
          <div key={lane.id} style={{ "--debug-lane": laneIndex } as CSSProperties}>
            <i />
            <i />
          </div>
        ))}
      </div>
    </section>
  );
}

export function DebuggerUnavailable({
  kind,
  message,
}: {
  kind: "empty" | "unsupported" | "error";
  message: string;
}) {
  return (
    <section className="execution-debugger debugger-unavailable" aria-label="Instrumented Debug">
      <div role={kind === "error" ? "alert" : "status"}>
        {kind === "error" ? <Warning size={28} weight="duotone" /> : <CrosshairSimple size={28} weight="thin" />}
        <strong>{kind === "error" ? "Debugger unavailable" : "No instrumented safe points yet"}</strong>
        <p>{message}</p>
        {kind !== "error" && (
          <small>
            Ordinary browser and API traffic proves only the request boundary. Internal method, branch, and query cards require explicit AONE_DEBUG_V1 instrumentation.
          </small>
        )}
      </div>
    </section>
  );
}

interface DebuggerToolbarProps {
  session?: DebugSessionView | null;
  replay: boolean;
  processActive: boolean;
  pendingAction: DebuggerControlAction | null;
  workflows: DebugWorkflowSummary[];
  workflowId?: string;
  onWorkflow: (workflowId: string) => void;
  hasPrevious: boolean;
  hasNext: boolean;
  granularity: "key" | "safe-points";
  onGranularity: (value: "key" | "safe-points") => void;
  onControl: (action: DebuggerControlAction) => void;
  onPrevious: () => void;
  onNext: () => void;
  replayPlaying: boolean;
  onReplayPlay: () => void;
  onReplayPause: () => void;
  onReplayRestart: () => void;
  onLatest: () => void;
  canLatest: boolean;
  onZoom: (factor: number) => void;
  onCenter: () => void;
}

export function DebuggerToolbar({
  session,
  replay,
  processActive,
  pendingAction,
  workflows,
  workflowId,
  onWorkflow,
  hasPrevious,
  hasNext,
  granularity,
  onGranularity,
  onControl,
  onPrevious,
  onNext,
  replayPlaying,
  onReplayPlay,
  onReplayPause,
  onReplayRestart,
  onLatest,
  canLatest,
  onZoom,
  onCenter,
}: DebuggerToolbarProps) {
  const capabilities = debuggerCapabilities(session);
  const effectivePendingAction = pendingAction ?? session?.pendingAction ?? null;
  const liveControls = processActive
    && !replay
    && session
    && ["starting", "running", "paused"].includes(session.status);
  const statusLabel = effectivePendingAction === "pause"
    ? "Pause requested, waiting for child acknowledgement"
    : effectivePendingAction === "resume"
      ? "Continue requested, waiting for next safe point"
      : effectivePendingAction === "stepOver" || effectivePendingAction === "stepInto"
        ? "Safe-point step requested, waiting for child acknowledgement"
        : effectivePendingAction === "stop"
          ? "Native process-group stop requested; waiting for process exit"
          : session && !processActive && ["starting", "running", "paused"].includes(session.status)
            ? "Managed process lifecycle changed; refreshing final status"
          : session?.status === "paused"
            ? "Paused at acknowledged safe point"
            : session?.status === "running"
              ? "Running; other threads or requests may continue"
              : session?.status === "completed"
                ? "Debug session ended: history replay"
                : session?.status === "failed"
                ? processActive
                  ? "Debug protocol failed; managed process may still be running"
                  : "Debug protocol failed: history replay"
                  : session?.status === "stopped"
                    ? "Managed leader exit observed after stop request: history replay"
                    : replay
                      ? "Observed history replay"
                      : session?.status ?? "Waiting";
  return (
    <header className="debugger-toolbar">
      <div className="debugger-identity">
        <strong>Instrumented Debug</strong>
        <span className={`debugger-session-status status-${session?.status ?? "replay"}`}>
          {statusLabel}
        </span>
      </div>

      <label className="debugger-workflow-select">
        <span>Workflow</span>
        <select value={workflowId ?? ""} disabled={workflows.length === 0} onChange={(event) => onWorkflow(event.target.value)}>
          {workflows.length === 0 && <option value="">Waiting for reported workflow</option>}
          {workflows.map((workflow) => (
            <option key={workflow.id} value={workflow.id}>
              #{workflow.firstSequence} to {workflow.lastSequence}: {workflow.label.slice(0, 38)}, {workflow.eventCount} cards{workflow.terminalState ? `, ${workflow.terminalState}` : ""}
            </option>
          ))}
        </select>
      </label>

      <div className="debugger-controls" role="group" aria-label={liveControls ? "Cooperative debug controls" : "Trace replay controls"}>
        {liveControls ? (
          <>
            {session.status === "paused" ? (
              <button type="button" onClick={() => onControl("resume")} disabled={!capabilities.resume || effectivePendingAction !== null} aria-keyshortcuts="F6" title="Continue from the acknowledged safe point">
                <Play size={13} weight="fill" /><span>Continue</span>
              </button>
            ) : (
              <button type="button" onClick={() => onControl("pause")} disabled={!capabilities.pause || effectivePendingAction !== null} aria-keyshortcuts="F6" title="Request pause at the next cooperative safe point">
                <Pause size={13} weight="fill" /><span>{effectivePendingAction === "pause" ? "Pause requested" : "Pause"}</span>
              </button>
            )}
            <button type="button" onClick={() => onControl("stepOver")} disabled={!capabilities.stepOver || effectivePendingAction !== null} aria-keyshortcuts="F10" title="Advance to the next cooperative safe point">
              <SkipForward size={13} /><span>Next safe point</span>
            </button>
            <button type="button" onClick={() => onControl("stepInto")} disabled={!capabilities.stepInto || effectivePendingAction !== null} aria-keyshortcuts="F11" title="Send the child a step-into instrumentation hint; stack entry is not proven">
              <ArrowBendDownRight size={13} /><span>Step into hint</span>
            </button>
            <button type="button" className="is-danger" onClick={() => onControl("stop")} disabled={!capabilities.stop || effectivePendingAction !== null} aria-keyshortcuts="Shift+F5">
              <Stop size={13} weight="fill" /><span>Stop</span>
            </button>
          </>
        ) : (
          <>
            <button type="button" onClick={onReplayRestart} disabled={!hasPrevious && !hasNext} title="Replay from the first retained event"><ArrowCounterClockwise size={13} /><span>Restart</span></button>
            {replayPlaying ? (
              <button type="button" onClick={onReplayPause}><Pause size={13} weight="fill" /><span>Pause replay</span></button>
            ) : (
              <button type="button" onClick={onReplayPlay} disabled={!hasNext && !hasPrevious}><Play size={13} weight="fill" /><span>Play replay</span></button>
            )}
            <button type="button" onClick={onPrevious} disabled={!hasPrevious}><CaretLeft size={13} /><span>Previous event</span></button>
            <button type="button" onClick={onNext} disabled={!hasNext}><CaretRight size={13} /><span>Next event</span></button>
            {processActive && session?.capabilities.stop.supported && (
              <button type="button" className="is-danger" onClick={() => onControl("stop")} disabled={effectivePendingAction !== null} aria-keyshortcuts="Shift+F5" title="Request authoritative native process-group stop">
                <Stop size={13} weight="fill" /><span>Stop process</span>
              </button>
            )}
          </>
        )}
      </div>

      <div className="debugger-granularity" role="group" aria-label="Safe point detail">
        <button type="button" className={granularity === "key" ? "is-active" : ""} aria-pressed={granularity === "key"} onClick={() => onGranularity("key")}>Key checkpoints</button>
        <button type="button" className={granularity === "safe-points" ? "is-active" : ""} aria-pressed={granularity === "safe-points"} onClick={() => onGranularity("safe-points")}>All safe points</button>
      </div>

      <div className="debugger-view-controls" role="group" aria-label="Debug canvas view">
        <button type="button" onClick={onLatest} disabled={!canLatest}>Latest</button>
        <button type="button" onClick={() => onZoom(1.2)} aria-label="Zoom in"><Plus size={13} /></button>
        <button type="button" onClick={() => onZoom(0.82)} aria-label="Zoom out"><Minus size={13} /></button>
        <button type="button" onClick={onCenter} aria-label="Center selected safe point"><CrosshairSimple size={13} /></button>
      </div>
    </header>
  );
}

export function StepInspector({
  step,
  session,
  onOpenSource,
  breakpointArmed = false,
  onToggleBreakpoint,
}: {
  step?: DebugExecutionStep;
  session?: DebugSessionView | null;
  onOpenSource: (source: SourceLocation) => void;
  breakpointArmed?: boolean;
  onToggleBreakpoint?: () => void;
}) {
  if (!step) return <aside className="debugger-inspector" aria-label="Safe point inspector"><div className="debugger-inspector-empty">Select a card to inspect its evidence and source.</div></aside>;
  const previewLines = step.preview ? safeShapeSummary(step.preview) : [];
  const current = Boolean(
    session?.activeWorkflowId
    && session.activeWorkflowId === step.workflowId
    && session.currentStepId === step.reportedStepId,
  );
  return (
    <aside className="debugger-inspector" aria-label="Safe point inspector">
      <header><span>{current ? "Current safe point" : "Selected event"}</span><EvidenceBadge evidence={step.evidence} compact /></header>
      <div className="debugger-inspector-body">
        <div className="debugger-inspector-kind">{stageIcon(step)} {stepTypeLabel(step)}</div>
        <h3>{step.label}</h3>
        <dl>
          <div><dt>Protocol</dt><dd>{step.protocol}</dd></div>
          <div><dt>Sequence</dt><dd>{step.sequence}</dd></div>
          {step.state && <div><dt>State</dt><dd>{step.state}</dd></div>}
          {step.branchOutcome && <div><dt>Outcome</dt><dd>{step.branchOutcome}</dd></div>}
          {step.operation && <div><dt>Reported operation</dt><dd>{step.operation}</dd></div>}
          {step.resource && <div><dt>Reported resource</dt><dd>{step.resource}</dd></div>}
          {step.durationMs !== undefined && <div><dt>Duration</dt><dd>{step.durationMs} ms</dd></div>}
          {step.repeatCount > 1 && <div><dt>Repeated</dt><dd>{step.repeatCount} checkpoints</dd></div>}
        </dl>
        <section className="debugger-source-proof">
          <h4>Process-reported source candidate</h4>
          {step.source ? (
            <button type="button" onClick={() => onOpenSource(step.source!)}>
              <span>{step.source.relativePath}</span>
              <strong>Line {step.source.startLine}{step.source.endLine !== step.source.startLine ? `-${step.source.endLine}` : ""}</strong>
            </button>
          ) : <p>No workspace source candidate was reported.</p>}
          {step.protocol === "AONE_DEBUG_V1" && step.reportedStepId && step.source && onToggleBreakpoint && (
            <button
              type="button"
              className={`debugger-breakpoint-button${breakpointArmed ? " is-armed" : ""}`}
              aria-pressed={breakpointArmed}
              onClick={onToggleBreakpoint}
            >
              <i aria-hidden="true" />
              <span>{breakpointArmed ? "Remove cooperative breakpoint" : "Break on this reported safe point"}</span>
            </button>
          )}
          <small>The file may have changed since launch because this session has no matching content hash.</small>
          {onToggleBreakpoint && (
            <small>Aone can stop only at stable AONE_DEBUG_V1 safe points reported by the running process. This is not an arbitrary source-line breakpoint.</small>
          )}
        </section>
        <section className="debugger-data-shape">
          <h4>Process-reported shape preview</h4>
          {previewLines.length > 0 ? <ul>{previewLines.map((line) => <li key={line}>{line}</li>)}</ul> : <p>No shape-only result metadata was captured for this safe point.</p>}
          <small>The protocol has no dedicated value, body, header, raw-query, bind, or arbitrary-metadata fields. Names are bounded and process-reported; known loaded secrets are rejected and sensitive field names are replaced, but instrumentation authors must keep identifiers data-free because Aone cannot independently verify their meaning.</small>
        </section>
      </div>
    </aside>
  );
}
