import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RuntimeEvent } from "../../types";
import type { DebuggerControlAction, DebugSessionView } from "./contracts";
import type { DebugSessionTransport } from "./bridge";

interface DebugSessionHookOptions {
  workspaceGeneration: number;
  events: RuntimeEvent[];
  transport?: DebugSessionTransport;
}

interface DebugTarget {
  runId: string;
  workspaceGeneration: number;
  requestGeneration: number;
}

const RUNNING_REFRESH_INTERVAL_MS = 250;
const lazyNativeDebugSessionTransport: DebugSessionTransport = {
  async getSession(runId) {
    const { nativeDebugSessionTransport } = await import("./bridge");
    return nativeDebugSessionTransport.getSession(runId);
  },
  async control(request) {
    const { nativeDebugSessionTransport } = await import("./bridge");
    return nativeDebugSessionTransport.control(request);
  },
};

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "Instrumented debug session request failed";
}

function currentSequence(session: DebugSessionView): number {
  return session.events[session.events.length - 1]?.sequence ?? 0;
}

function isTargetCurrent(target: DebugTarget, current: DebugTarget | null): boolean {
  return Boolean(
    current
    && current.runId === target.runId
    && current.workspaceGeneration === target.workspaceGeneration
    && current.requestGeneration === target.requestGeneration,
  );
}

export function useExecutionDebuggerSession({
  workspaceGeneration,
  events,
  transport = lazyNativeDebugSessionTransport,
}: DebugSessionHookOptions) {
  const [runId, setRunId] = useState<string>();
  const [session, setSession] = useState<DebugSessionView | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspaceRef = useRef(workspaceGeneration);
  const targetRef = useRef<DebugTarget | null>(null);
  const requestGenerationRef = useRef(0);
  const snapshotRequestRef = useRef(0);
  const committedSnapshotRequestRef = useRef(0);
  const sessionRef = useRef<DebugSessionView | null>(null);
  const eventsRef = useRef(events);
  const eventCursorRef = useRef<string | undefined>(undefined);
  const controlTimeoutRef = useRef<number | null>(null);
  const coalesceTimerRef = useRef<number | null>(null);
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);
  const lastRefreshAtRef = useRef(0);
  const transportRef = useRef(transport);
  eventsRef.current = events;
  transportRef.current = transport;

  const cancelTimeoutRefresh = useCallback(() => {
    if (controlTimeoutRef.current !== null) {
      window.clearTimeout(controlTimeoutRef.current);
      controlTimeoutRef.current = null;
    }
    if (coalesceTimerRef.current !== null) {
      window.clearTimeout(coalesceTimerRef.current);
      coalesceTimerRef.current = null;
    }
    refreshQueuedRef.current = false;
  }, []);

  const clear = useCallback(() => {
    cancelTimeoutRefresh();
    requestGenerationRef.current += 1;
    targetRef.current = null;
    sessionRef.current = null;
    eventCursorRef.current = eventsRef.current[eventsRef.current.length - 1]?.id;
    setRunId(undefined);
    setSession(null);
    setLoading(false);
    setError(null);
  }, [cancelTimeoutRefresh]);

  useLayoutEffect(() => {
    if (workspaceRef.current === workspaceGeneration) return;
    workspaceRef.current = workspaceGeneration;
    clear();
  }, [clear, workspaceGeneration]);

  useEffect(() => () => cancelTimeoutRefresh(), [cancelTimeoutRefresh]);

  const commit = useCallback(async (
    target: DebugTarget,
    next: DebugSessionView,
    snapshotRequest: number,
  ) => {
    if (!isTargetCurrent(target, targetRef.current) || next.runId !== target.runId) return false;
    if (snapshotRequest < committedSnapshotRequestRef.current) return false;
    const { snapshotIsCurrent } = await import("./sessionPolicy");
    if (!isTargetCurrent(target, targetRef.current)) return false;
    if (!snapshotIsCurrent(sessionRef.current, next)) return false;
    committedSnapshotRequestRef.current = snapshotRequest;
    sessionRef.current = next;
    setSession(next);
    setError(null);
    return true;
  }, []);

  const fetchTarget = useCallback(async (target: DebugTarget, initial = false) => {
    const snapshotRequest = ++snapshotRequestRef.current;
    if (initial) setLoading(true);
    try {
      const next = await transportRef.current.getSession(target.runId);
      await commit(target, next, snapshotRequest);
    } catch (nextError) {
      if (isTargetCurrent(target, targetRef.current)) setError(errorText(nextError));
    } finally {
      if (initial && isTargetCurrent(target, targetRef.current)) setLoading(false);
    }
  }, [commit]);

  const performRefresh = useCallback(async (target: DebugTarget) => {
    if (!isTargetCurrent(target, targetRef.current)) return;
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      return;
    }
    refreshInFlightRef.current = true;
    lastRefreshAtRef.current = Date.now();
    await fetchTarget(target);
    refreshInFlightRef.current = false;
    if (!refreshQueuedRef.current || !isTargetCurrent(target, targetRef.current)) return;
    refreshQueuedRef.current = false;
    coalesceTimerRef.current = window.setTimeout(() => {
      coalesceTimerRef.current = null;
      void performRefresh(target);
    }, RUNNING_REFRESH_INTERVAL_MS);
  }, [fetchTarget]);

  const scheduleRefresh = useCallback((target: DebugTarget, immediate: boolean) => {
    if (!isTargetCurrent(target, targetRef.current)) return;
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      return;
    }
    if (coalesceTimerRef.current !== null) {
      if (!immediate) return;
      window.clearTimeout(coalesceTimerRef.current);
      coalesceTimerRef.current = null;
    }
    const delay = immediate
      ? 0
      : Math.max(0, RUNNING_REFRESH_INTERVAL_MS - (Date.now() - lastRefreshAtRef.current));
    if (delay === 0) void performRefresh(target);
    else {
      coalesceTimerRef.current = window.setTimeout(() => {
        coalesceTimerRef.current = null;
        void performRefresh(target);
      }, delay);
    }
  }, [performRefresh]);

  const begin = useCallback(async (nextRunId: string) => {
    cancelTimeoutRefresh();
    const target: DebugTarget = {
      runId: nextRunId,
      workspaceGeneration,
      requestGeneration: ++requestGenerationRef.current,
    };
    targetRef.current = target;
    sessionRef.current = null;
    eventCursorRef.current = eventsRef.current[eventsRef.current.length - 1]?.id;
    setRunId(nextRunId);
    setSession(null);
    setError(null);
    await fetchTarget(target, true);
  }, [cancelTimeoutRefresh, fetchTarget, workspaceGeneration]);

  const refresh = useCallback(async () => {
    const target = targetRef.current;
    if (target) await performRefresh(target);
  }, [performRefresh]);

  useEffect(() => {
    const target = targetRef.current;
    const lastEvent = events[events.length - 1];
    if (!target || !lastEvent || eventCursorRef.current === lastEvent.id) return;
    const cursorIndex = eventCursorRef.current
      ? events.findIndex((event) => event.id === eventCursorRef.current)
      : -1;
    const additions = cursorIndex < 0 ? events : events.slice(cursorIndex + 1);
    eventCursorRef.current = lastEvent.id;
    void import("./sessionPolicy").then(({ refreshPriorityFromEvents }) => {
      if (!isTargetCurrent(target, targetRef.current)) return;
      const priority = refreshPriorityFromEvents(additions, target.runId, sessionRef.current?.pendingAction);
      if (!priority) return;
      if (priority === "immediate" && controlTimeoutRef.current !== null) {
        window.clearTimeout(controlTimeoutRef.current);
        controlTimeoutRef.current = null;
      }
      scheduleRefresh(target, priority === "immediate");
    });
  }, [events, scheduleRefresh]);

  const control = useCallback(async (action: DebuggerControlAction) => {
    const target = targetRef.current;
    const current = sessionRef.current;
    if (!target || !current || current.runId !== target.runId) {
      throw new Error("No current instrumented debug session is available");
    }
    if (current.pendingAction) throw new Error("A debug control is already awaiting acknowledgement");
    try {
      const snapshotRequest = ++snapshotRequestRef.current;
      const result = await transportRef.current.control({
        runId: target.runId,
        debugSessionId: current.debugSessionId,
        action,
        expectedSequence: currentSequence(current),
        expectedControlEpoch: current.controlEpoch,
      });
      if (!result.accepted) throw new Error("The managed process did not accept the debug control");
      await commit(target, result.session, snapshotRequest);
      if (action !== "stop" && result.session.pendingAction) {
        cancelTimeoutRefresh();
        controlTimeoutRef.current = window.setTimeout(() => {
          controlTimeoutRef.current = null;
          if (isTargetCurrent(target, targetRef.current)) scheduleRefresh(target, true);
        }, 30_500);
      }
    } catch (nextError) {
      if (isTargetCurrent(target, targetRef.current)) setError(errorText(nextError));
      throw nextError;
    }
  }, [cancelTimeoutRefresh, commit, scheduleRefresh]);

  return { runId, session, loading, error, begin, refresh, control, clear };
}
