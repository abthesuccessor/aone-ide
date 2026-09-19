import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type RefObject,
  type SetStateAction,
} from "react";
import type { ToastTone } from "../components/Toast";
import { subscribeToDesktopEvents } from "../lib/bridge";
import { rememberTerminalRun } from "../lib/runLifecycle";
import type { RuntimeEvent, ScanProgress, WebSocketEvent } from "../types";
import type { ActiveRun } from "./model";

interface RuntimeSubscriptionsOptions {
  loadFilesAndGraph: () => Promise<void>;
  notify: (text: string, tone?: ToastTone) => void;
  setScanProgress: Dispatch<SetStateAction<ScanProgress | null>>;
  setRuntimeEvents: Dispatch<SetStateAction<RuntimeEvent[]>>;
  setActiveRun: Dispatch<SetStateAction<ActiveRun | null>>;
  activeRunRef: RefObject<ActiveRun | null>;
  scanOperationActiveRef: RefObject<boolean>;
  terminalRunIdsRef: RefObject<Set<string>>;
  appendWebSocketEvent: (event: WebSocketEvent) => void;
}

const MAX_RUNTIME_EVENTS = 500;
const RUNTIME_EVENT_FLUSH_MS = 100;

export function useRuntimeSubscriptions({
  loadFilesAndGraph,
  notify,
  setScanProgress,
  setRuntimeEvents,
  setActiveRun,
  activeRunRef,
  scanOperationActiveRef,
  terminalRunIdsRef,
  appendWebSocketEvent,
}: RuntimeSubscriptionsOptions) {
  const workspaceReloadTimerRef = useRef<number | null>(null);
  const runtimeEventFlushTimerRef = useRef<number | null>(null);
  const pendingRuntimeEventsRef = useRef<RuntimeEvent[]>([]);
  const [eventSubscriptionReady, setEventSubscriptionReady] = useState(false);

  const cancelWorkspaceReload = useCallback(() => {
    if (workspaceReloadTimerRef.current === null) return;
    window.clearTimeout(workspaceReloadTimerRef.current);
    workspaceReloadTimerRef.current = null;
  }, []);

  const flushRuntimeEvents = useCallback(() => {
    runtimeEventFlushTimerRef.current = null;
    const pending = pendingRuntimeEventsRef.current;
    pendingRuntimeEventsRef.current = [];
    if (pending.length === 0) return;
    setRuntimeEvents((current) => [...current, ...pending].slice(-MAX_RUNTIME_EVENTS));
  }, [setRuntimeEvents]);

  const enqueueRuntimeEvent = useCallback(
    (event: RuntimeEvent) => {
      const pending = pendingRuntimeEventsRef.current;
      pending.push(event);
      if (pending.length > MAX_RUNTIME_EVENTS) {
        pending.splice(0, pending.length - MAX_RUNTIME_EVENTS);
      }
      if (runtimeEventFlushTimerRef.current === null) {
        runtimeEventFlushTimerRef.current = window.setTimeout(
          flushRuntimeEvents,
          RUNTIME_EVENT_FLUSH_MS,
        );
      }
    },
    [flushRuntimeEvents],
  );

  const cancelRuntimeEventFlush = useCallback(() => {
    if (runtimeEventFlushTimerRef.current !== null) {
      window.clearTimeout(runtimeEventFlushTimerRef.current);
      runtimeEventFlushTimerRef.current = null;
    }
    pendingRuntimeEventsRef.current = [];
  }, []);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: () => void = () => undefined;
    setEventSubscriptionReady(false);
    void subscribeToDesktopEvents({
      onScanProgress: (progress) => {
        if (!mounted || !scanOperationActiveRef.current) return;
        setScanProgress(progress.phase.toLowerCase() === "complete" ? null : progress);
      },
      onRuntimeEvent: (event) => {
        if (!mounted) return;
        const terminalRunId = rememberTerminalRun(terminalRunIdsRef.current, event);
        if (terminalRunId) {
          if (activeRunRef.current?.id === terminalRunId) {
            notify(
              event.kind === "process.exited"
                ? "Managed process exited"
                : "Process exit could not be observed reliably",
              event.kind === "process.exited" ? "info" : "warning",
            );
          }
          setActiveRun((current) => (current?.id === terminalRunId ? null : current));
        }
        enqueueRuntimeEvent(event);
      },
      onWorkspaceChanged: (paths) => {
        if (!mounted) return;
        notify(`${paths.length} workspace file${paths.length === 1 ? "" : "s"} changed`, "info");
        cancelWorkspaceReload();
        workspaceReloadTimerRef.current = window.setTimeout(() => {
          workspaceReloadTimerRef.current = null;
          if (mounted) void loadFilesAndGraph();
        }, 280);
      },
      onWebSocketEvent: (event) => {
        if (mounted) appendWebSocketEvent(event);
      },
    }).then((stop) => {
      if (mounted) {
        unsubscribe = stop;
        setEventSubscriptionReady(true);
      } else stop();
    }).catch((error: unknown) => {
      if (!mounted) return;
      notify(
        error instanceof Error
          ? `Desktop event listeners failed: ${error.message}`
          : "Desktop event listeners failed",
        "error",
      );
    });

    return () => {
      mounted = false;
      cancelWorkspaceReload();
      cancelRuntimeEventFlush();
      unsubscribe();
    };
  }, [
    cancelWorkspaceReload,
    cancelRuntimeEventFlush,
    appendWebSocketEvent,
    activeRunRef,
    enqueueRuntimeEvent,
    loadFilesAndGraph,
    notify,
    setActiveRun,
    setScanProgress,
    scanOperationActiveRef,
    terminalRunIdsRef,
  ]);

  return { cancelWorkspaceReload, cancelRuntimeEventFlush, eventSubscriptionReady };
}
