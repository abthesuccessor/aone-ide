import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ActiveRun } from "./model";
import { useRunActions } from "./useRunActions";

const bridge = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(),
  startRun: vi.fn(),
  stopRun: vi.fn(),
}));

vi.mock("../lib/bridge", () => bridge);

const profile = {
  id: "profile",
  name: "Dev",
  kind: "command" as const,
  executable: "npm",
  args: ["run", "dev"],
  requiredEnv: [],
  source: "package.json",
};

function options(activeRun: ActiveRun | null = null) {
  return {
    activeRun,
    activeProfileId: profile.id,
    runProfiles: [profile],
    runEnv: null,
    openingWorkspaceRef: { current: false },
    terminalRunIdsRef: { current: new Set<string>() },
    debuggerSession: {
      runId: "run-3",
      session: null,
      begin: vi.fn().mockResolvedValue(undefined),
      control: vi.fn().mockResolvedValue(undefined),
    },
    setActiveRun: vi.fn(),
    setConsoleCollapsed: vi.fn(),
    setEnvOpen: vi.fn(),
    setMainView: vi.fn(),
    setRuntimeEvents: vi.fn(),
    notify: vi.fn(),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  bridge.isTauriRuntime.mockReturnValue(true);
  bridge.startRun
    .mockResolvedValueOnce({ runId: "run-1" })
    .mockResolvedValueOnce({ runId: "run-2" })
    .mockResolvedValueOnce({ runId: "run-3" });
  bridge.stopRun.mockResolvedValue({ stopped: true });
});

describe("useRunActions", () => {
  it("treats one explicit Run or Debug action as authorization for the selected backend profile", async () => {
    const confirm = vi.spyOn(window, "confirm");
    const input = options();
    const { result } = renderHook(() => useRunActions(input));

    await act(async () => result.current.handleStart("run"));
    await act(async () => result.current.handleStart("observe"));
    await act(async () => result.current.handleStart("debug"));

    expect(bridge.startRun).toHaveBeenNthCalledWith(1, {
      profileId: "profile", envNames: [], observe: false, debug: false,
    });
    expect(bridge.startRun).toHaveBeenNthCalledWith(2, {
      profileId: "profile", envNames: [], observe: true, debug: false,
    });
    expect(bridge.startRun).toHaveBeenNthCalledWith(3, {
      profileId: "profile", envNames: [], observe: true, debug: true,
    });
    expect(confirm).not.toHaveBeenCalled();
    expect(input.debuggerSession.begin).toHaveBeenCalledWith("run-3");
    expect(input.setMainView).toHaveBeenCalledWith("debugger");
    expect(input.setConsoleCollapsed).toHaveBeenLastCalledWith(true);
  });

  it("does not simulate Instrumented Debug in the browser demo", async () => {
    bridge.isTauriRuntime.mockReturnValue(false);
    const input = options();
    const { result } = renderHook(() => useRunActions(input));

    await act(async () => result.current.handleStart("debug"));

    expect(bridge.startRun).not.toHaveBeenCalled();
    expect(input.notify).toHaveBeenCalledWith(expect.stringMatching(/native desktop app/i), "warning");
  });

  it("keeps the run active in a stopping state until a terminal event arrives", async () => {
    const active: ActiveRun = { id: "run-3", mode: "debug", stopping: false };
    const input = options(active);
    const { result } = renderHook(() => useRunActions(input));

    await act(async () => result.current.handleStop());

    const update = input.setActiveRun.mock.calls[0]?.[0] as (current: ActiveRun) => ActiveRun;
    expect(update(active)).toEqual({ ...active, stopping: true });
    expect(input.notify).toHaveBeenCalledWith(expect.stringMatching(/waiting.*exit/i), "info");
  });

  it("uses authoritative debug Stop and surfaces rejected controls", async () => {
    const active: ActiveRun = { id: "run-3", mode: "debug", stopping: false };
    const input = options(active);
    const { result } = renderHook(() => useRunActions(input));

    await act(async () => result.current.handleDebugControl("stop"));
    expect(input.debuggerSession.control).toHaveBeenCalledWith("stop");
    expect(input.notify).toHaveBeenCalledWith(expect.stringMatching(/process-group stop requested/i), "info");

    input.debuggerSession.control.mockRejectedValueOnce(new Error("stale debug control"));
    await expect(result.current.handleDebugControl("pause")).rejects.toThrow("stale debug control");
    expect(input.notify).toHaveBeenLastCalledWith("stale debug control", "error");
  });
});
