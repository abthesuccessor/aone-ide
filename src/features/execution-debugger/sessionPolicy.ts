import type { RuntimeEvent } from "../../types";
import type { DebuggerControlAction, DebugSessionView } from "./contracts";

const TERMINAL_EVENT_KINDS = new Set(["process.exited", "process.wait_failed"]);
const TERMINAL_SESSION_STATUSES = new Set(["completed", "failed", "stopped"]);

export function snapshotIsCurrent(
  current: DebugSessionView | null,
  next: DebugSessionView,
): boolean {
  if (!current) return true;
  if (current.runId !== next.runId || current.debugSessionId !== next.debugSessionId) return false;
  if (TERMINAL_SESSION_STATUSES.has(current.status) && !TERMINAL_SESSION_STATUSES.has(next.status)) {
    return false;
  }
  return next.eventCount >= current.eventCount
    && next.controlEpoch >= current.controlEpoch
    && next.acknowledgedControlEpoch >= current.acknowledgedControlEpoch;
}

export function refreshPriorityFromEvents(
  events: RuntimeEvent[],
  runId: string,
  pendingAction: DebuggerControlAction | undefined,
): "immediate" | "coalesced" | null {
  let runningReport = false;
  for (const event of events) {
    if (event.runId !== runId) continue;
    if (TERMINAL_EVENT_KINDS.has(event.kind) || event.kind === "debug.rejected") return "immediate";
    if (!event.kind.startsWith("debug.")) continue;
    const state = event.metadata.safePointState;
    if (pendingAction !== undefined
      || state === "paused"
      || state === "workflowCompleted"
      || state === "workflowFailed") return "immediate";
    if (state === "running") runningReport = true;
  }
  return runningReport ? "coalesced" : null;
}
