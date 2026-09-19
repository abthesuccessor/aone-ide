import {
  CaretLeft,
  CaretRight,
  Info,
  Pause,
  Play,
  SkipForward,
  Stop,
} from "@phosphor-icons/react";
import {
  debuggerCapabilities,
  type DebuggerControlAction,
  type DebugSessionView,
  type DebugWorkflowSummary,
} from "./model";

interface TraceToolbarProps {
  session?: DebugSessionView | null;
  replay: boolean;
  processActive: boolean;
  pendingAction: DebuggerControlAction | null;
  workflows: DebugWorkflowSummary[];
  workflowId?: string;
  onWorkflow: (workflowId: string) => void;
  selectedIndex: number;
  stepCount: number;
  hasPrevious: boolean;
  canAdvance: boolean;
  nextLabel: string;
  onPrevious: () => void;
  onNext: () => void;
  replayPlaying: boolean;
  onReplayPlay: () => void;
  onReplayPause: () => void;
  onControl: (action: DebuggerControlAction) => void;
  requestScoped: boolean;
}

function shortStatus(
  session: DebugSessionView | null | undefined,
  pendingAction: DebuggerControlAction | null,
  replay: boolean,
): string {
  if (pendingAction === "pause") return "Pausing…";
  if (pendingAction === "resume") return "Continuing…";
  if (pendingAction === "stepOver" || pendingAction === "stepInto") return "Stepping…";
  if (pendingAction === "stop") return "Stopping…";
  if (!session) return replay ? "History" : "Ready";
  if (replay && session.status === "running") return "History";
  if (session.status === "paused") return "Paused";
  if (session.status === "running") return "Running";
  if (session.status === "completed") return "Complete";
  if (session.status === "failed") return "Failed";
  if (session.status === "stopped") return "Stopped";
  return "Starting…";
}

export function TraceToolbar({
  session,
  replay,
  processActive,
  pendingAction,
  workflows,
  workflowId,
  onWorkflow,
  selectedIndex,
  stepCount,
  hasPrevious,
  canAdvance,
  nextLabel,
  onPrevious,
  onNext,
  replayPlaying,
  onReplayPlay,
  onReplayPause,
  onControl,
  requestScoped,
}: TraceToolbarProps) {
  const capabilities = debuggerCapabilities(session);
  const effectivePending = pendingAction ?? session?.pendingAction ?? null;
  const live = Boolean(
    processActive
    && !replay
    && session
    && ["starting", "running", "paused"].includes(session.status),
  );
  const status = shortStatus(session, effectivePending, replay);

  return (
    <header className="trace-toolbar">
      <div className="trace-toolbar-title">
        <span className={`trace-status-dot status-${session?.status ?? "ready"}`} aria-hidden="true" />
        <strong>API trace</strong>
        <span className="trace-status-label" role="status">{status}</span>
      </div>

      <label className="trace-workflow-select">
        <span className="visually-hidden">Workflow</span>
        <select
          aria-label="Workflow"
          value={workflowId ?? ""}
          disabled={workflows.length === 0}
          onChange={(event) => onWorkflow(event.target.value)}
        >
          {workflows.length === 0 && <option value="">Waiting for request path</option>}
          {workflows.map((workflow) => (
            <option key={workflow.id} value={workflow.id}>
              #{workflow.firstSequence}–{workflow.lastSequence} · {workflow.label.slice(0, 48)}
            </option>
          ))}
        </select>
      </label>

      <span className="trace-step-position" aria-label={`${Math.max(0, selectedIndex + 1)} of ${stepCount} safe points`}>
        {stepCount === 0 ? "0 / 0" : `${selectedIndex + 1} / ${stepCount}`}
      </span>

      <div className="trace-toolbar-controls" role="group" aria-label={live ? "Cooperative debug controls" : "Trace replay controls"}>
        <button type="button" onClick={onPrevious} disabled={!hasPrevious} aria-label="Previous event" title="Previous reported safe point">
          <CaretLeft size={14} aria-hidden="true" /><span>Prev</span>
        </button>

        {live ? (
          session?.status === "paused" ? (
            <button
              type="button"
              onClick={() => onControl("resume")}
              disabled={!capabilities.resume || effectivePending !== null}
              aria-keyshortcuts="F6"
            >
              <Play size={13} weight="fill" aria-hidden="true" /><span>Continue</span>
            </button>
          ) : (
            <button
              type="button"
              onClick={() => onControl("pause")}
              disabled={!capabilities.pause || effectivePending !== null}
              aria-keyshortcuts="F6"
            >
              <Pause size={13} weight="fill" aria-hidden="true" /><span>Pause</span>
            </button>
          )
        ) : replayPlaying ? (
          <button type="button" onClick={onReplayPause}>
            <Pause size={13} weight="fill" aria-hidden="true" /><span>Pause</span>
          </button>
        ) : (
          <button type="button" onClick={onReplayPlay} disabled={stepCount < 2}>
            <Play size={13} weight="fill" aria-hidden="true" /><span>Replay</span>
          </button>
        )}

        <button
          type="button"
          onClick={onNext}
          disabled={!canAdvance || effectivePending !== null}
          aria-label={nextLabel}
          title={nextLabel}
          aria-keyshortcuts={live ? "F10" : undefined}
        >
          <SkipForward size={14} aria-hidden="true" /><span>Next</span>
        </button>

        {live && session?.status === "paused" && (
          <button
            type="button"
            onClick={() => onControl("stepInto")}
            disabled={!capabilities.stepInto || effectivePending !== null}
            aria-label="Step into instrumentation hint"
            title="Step into instrumentation hint (F11)"
            aria-keyshortcuts="F11"
          >
            <CaretRight size={14} aria-hidden="true" />
          </button>
        )}

        {(live || processActive && session?.capabilities.stop.supported) && (
          <button
            type="button"
            className="is-danger"
            onClick={() => onControl("stop")}
            disabled={!session?.capabilities.stop.supported || effectivePending !== null}
            aria-label="Stop"
            title="Stop managed process (Shift+F5)"
            aria-keyshortcuts="Shift+F5"
          >
            <Stop size={13} weight="fill" aria-hidden="true" />
          </button>
        )}
      </div>

      <details className="trace-toolbar-info">
        <summary aria-label="Trace evidence details" title="Trace evidence details">
          <Info size={14} aria-hidden="true" />
        </summary>
        <div>
          <strong>{requestScoped ? "Events observed after Send" : "Reported workflow history"}</strong>
          <p>Source positions and internal data shapes are reported by the running process. They are cooperative safe points, not arbitrary line-by-line debugger stops.</p>
          {session?.limitation && <p>{session.limitation}</p>}
        </div>
      </details>
    </header>
  );
}
