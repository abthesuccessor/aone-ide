import { useCallback, type Dispatch, type RefObject, type SetStateAction } from "react";
import type { ToastTone } from "../components/Toast";
import type { DebuggerControlAction, DebugSessionView } from "../features/execution-debugger/contracts";
import { isTauriRuntime, startRun, stopRun } from "../lib/bridge";
import { runAlreadyTerminated } from "../lib/runLifecycle";
import type { EnvLoadResult, RunProfile, RuntimeEvent, StartRunRequest } from "../types";
import type { ActiveRun, MainView } from "./model";
import { createBrowserRunEvent } from "./runtimeEvents";

interface DebugSessionActions {
  runId?: string;
  session: DebugSessionView | null;
  begin: (runId: string) => Promise<void>;
  control: (action: DebuggerControlAction) => Promise<void>;
}

interface RunActionsOptions {
  activeRun: ActiveRun | null;
  activeProfileId: string;
  runProfiles: RunProfile[];
  runEnv: EnvLoadResult | null;
  openingWorkspaceRef: RefObject<boolean>;
  terminalRunIdsRef: RefObject<Set<string>>;
  debuggerSession: DebugSessionActions;
  setActiveRun: Dispatch<SetStateAction<ActiveRun | null>>;
  setConsoleCollapsed: Dispatch<SetStateAction<boolean>>;
  setEnvOpen: Dispatch<SetStateAction<boolean>>;
  setMainView: Dispatch<SetStateAction<MainView>>;
  setRuntimeEvents: Dispatch<SetStateAction<RuntimeEvent[]>>;
  notify: (text: string, tone?: ToastTone) => void;
}

export function useRunActions({
  activeRun,
  activeProfileId,
  runProfiles,
  runEnv,
  openingWorkspaceRef,
  terminalRunIdsRef,
  debuggerSession,
  setActiveRun,
  setConsoleCollapsed,
  setEnvOpen,
  setMainView,
  setRuntimeEvents,
  notify,
}: RunActionsOptions) {
  const handleStart = useCallback(async (mode: ActiveRun["mode"]) => {
    if (openingWorkspaceRef.current) return;
    if (mode === "debug" && !isTauriRuntime()) {
      notify("Instrumented Debug requires the native desktop app and an explicitly instrumented managed child", "warning");
      return;
    }
    const profile = runProfiles.find((candidate) => candidate.id === activeProfileId);
    if (!profile) {
      notify("Choose a run profile first", "warning");
      return;
    }
    const loadedRunNames = new Set(runEnv?.names ?? []);
    const missingEnvironment = profile.requiredEnv.filter((name) => !loadedRunNames.has(name));
    if (missingEnvironment.length > 0) {
      setEnvOpen(true);
      notify(`This profile needs environment keys: ${missingEnvironment.join(", ")}`, "warning");
      return;
    }
    try {
      const request: StartRunRequest = {
        profileId: profile.id,
        envNames: runEnv?.names ?? [],
        observe: mode !== "run",
        debug: mode === "debug",
      };
      const result = await startRun(request);
      const alreadyExited = runAlreadyTerminated(terminalRunIdsRef.current, result.runId);
      setActiveRun(alreadyExited ? null : { id: result.runId, mode, stopping: false });
      setConsoleCollapsed(mode === "debug");
      if (mode === "debug") {
        setMainView("debugger");
        void debuggerSession.begin(result.runId);
      }
      notify(
        alreadyExited
          ? `${profile.name} exited before the start response completed`
          : mode === "debug"
            ? `Started ${profile.name} with cooperative safe-point debugging`
            : mode === "observe"
              ? `Started ${profile.name} with runtime observation`
              : `Running ${profile.name}`,
        alreadyExited ? "info" : "success",
      );
      if (!isTauriRuntime()) {
        const event = createBrowserRunEvent(result.runId, profile, mode);
        setRuntimeEvents((current) => [...current, event]);
      }
    } catch (error) {
      notify(error instanceof Error ? error.message : "Run failed to start", "error");
    }
  }, [
    activeProfileId,
    debuggerSession.begin,
    notify,
    openingWorkspaceRef,
    runEnv,
    runProfiles,
    setActiveRun,
    setConsoleCollapsed,
    setEnvOpen,
    setMainView,
    setRuntimeEvents,
    terminalRunIdsRef,
  ]);

  const handleStop = useCallback(async () => {
    if (!activeRun || activeRun.stopping) return;
    try {
      const result = await stopRun(activeRun.id);
      if (!result.stopped) throw new Error("The managed process did not accept the stop request");
      setActiveRun((current) => current?.id === activeRun.id ? { ...current, stopping: true } : current);
      notify("Stop requested; waiting for the managed process to exit", "info");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Unable to stop run", "error");
    }
  }, [activeRun, notify, setActiveRun]);

  const handleDebugControl = useCallback(async (action: DebuggerControlAction) => {
    try {
      await debuggerSession.control(action);
      if (action !== "stop") return;
      setActiveRun((current) => {
        if (!current || current.id !== debuggerSession.runId) return current;
        return { ...current, stopping: true };
      });
      notify("Native process-group stop requested; waiting for exit", "info");
    } catch (error) {
      notify(error instanceof Error ? error.message : "Debug control was rejected", "error");
      throw error;
    }
  }, [debuggerSession.control, debuggerSession.runId, notify, setActiveRun]);

  return { handleStart, handleStop, handleDebugControl };
}
