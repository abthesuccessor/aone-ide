import { useCallback, useEffect, useState } from "react";
import { isTauriRuntime } from "../../lib/bridge";
import {
  deleteRuntimeTrace,
  getOtlpReceiver,
  startOtlpReceiver,
  stopOtlpReceiver,
} from "./bridge";
import { failureMessage } from "../../lib/errorMessage";
import type { OtlpReceiverSnapshot } from "../../types";

interface UseOtlpReceiverOptions {
  workspaceId?: string;
  workspaceGeneration: number;
  onRuntimeEventsChanged: () => Promise<void>;
}

export function useOtlpReceiver({
  workspaceId,
  workspaceGeneration,
  onRuntimeEventsChanged,
}: UseOtlpReceiverOptions) {
  const [snapshot, setSnapshot] = useState<OtlpReceiverSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!workspaceId) {
      setSnapshot(null);
      return;
    }
    try {
      setSnapshot(await getOtlpReceiver());
      setError(null);
    } catch (cause) {
      setError(failureMessage(cause, "Could not read the local OTLP receiver state"));
    }
  }, [workspaceId]);

  useEffect(() => {
    setSnapshot(null);
    setError(null);
    void refresh();
  }, [refresh, workspaceGeneration]);

  useEffect(() => {
    if (snapshot?.status !== "running") return;
    const timer = window.setInterval(() => void refresh(), 1_000);
    return () => window.clearInterval(timer);
  }, [refresh, snapshot?.status]);

  const start = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const next = await startOtlpReceiver();
      setSnapshot(next);
    } catch (cause) {
      const message = failureMessage(cause, "Could not start the local OTLP receiver");
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const stop = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setSnapshot(await stopOtlpReceiver());
    } catch (cause) {
      const message = failureMessage(cause, "Could not stop the local OTLP receiver");
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const deleteTrace = useCallback(async (traceId: string) => {
    setLoading(true);
    setError(null);
    try {
      const result = await deleteRuntimeTrace(traceId);
      await onRuntimeEventsChanged();
      await refresh();
      if (result.deletedEvents === 0) setError("No retained events matched the selected trace");
    } catch (cause) {
      const message = failureMessage(cause, "Could not delete the retained runtime trace");
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [onRuntimeEventsChanged, refresh]);

  return {
    snapshot,
    loading,
    error,
    available: isTauriRuntime() && Boolean(workspaceId),
    refresh,
    start,
    stop,
    deleteTrace,
  };
}
