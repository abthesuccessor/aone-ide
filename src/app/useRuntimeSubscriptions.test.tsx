import { act, renderHook } from "@testing-library/react";
import type { SetStateAction } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeEvent, ScanProgress } from "../types";
import { useRuntimeSubscriptions } from "./useRuntimeSubscriptions";

const bridgeMocks = vi.hoisted(() => ({
  subscribeToDesktopEvents: vi.fn(),
}));

vi.mock("../lib/bridge", () => ({
  subscribeToDesktopEvents: bridgeMocks.subscribeToDesktopEvents,
}));

beforeEach(() => {
  bridgeMocks.subscribeToDesktopEvents.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function runtimeEvent(index: number, kind = "process.stdout"): RuntimeEvent {
  return {
    id: `event-${index}`,
    runId: "run-burst",
    kind,
    timestamp: new Date(index).toISOString(),
    label: String(index),
    evidence: "observed",
    metadata: { text: `line-${index}` },
  };
}

describe("useRuntimeSubscriptions", () => {
  it("reports readiness only after every desktop listener is installed", async () => {
    let resolveSubscription: ((stop: () => void) => void) | undefined;
    const stop = vi.fn();
    bridgeMocks.subscribeToDesktopEvents.mockReturnValue(new Promise((resolve) => {
      resolveSubscription = resolve;
    }));
    const options = {
      loadFilesAndGraph: vi.fn().mockResolvedValue(undefined),
      notify: vi.fn(),
      setScanProgress: vi.fn(),
      setRuntimeEvents: vi.fn(),
      setActiveRun: vi.fn(),
      activeRunRef: { current: null },
      scanOperationActiveRef: { current: true },
      terminalRunIdsRef: { current: new Set<string>() },
      appendWebSocketEvent: vi.fn(),
    };
    const { result, unmount } = renderHook(() => useRuntimeSubscriptions(options));

    expect(result.current.eventSubscriptionReady).toBe(false);
    await act(async () => resolveSubscription?.(stop));
    expect(result.current.eventSubscriptionReady).toBe(true);

    unmount();
    expect(stop).toHaveBeenCalledOnce();
  });

  it("clears terminal completion and ignores progress after the operation ends", async () => {
    let handlers: { onScanProgress?: (progress: ScanProgress) => void } = {};
    bridgeMocks.subscribeToDesktopEvents.mockImplementation(async (nextHandlers) => {
      handlers = nextHandlers;
      return () => undefined;
    });
    const scanOperationActiveRef = { current: true };
    let storedProgress: ScanProgress | null = null;
    const setScanProgress = vi.fn((next: SetStateAction<ScanProgress | null>) => {
      storedProgress = typeof next === "function" ? next(storedProgress) : next;
    });
    const options = {
      loadFilesAndGraph: vi.fn().mockResolvedValue(undefined),
      notify: vi.fn(),
      setScanProgress,
      setRuntimeEvents: vi.fn(),
      setActiveRun: vi.fn(),
      activeRunRef: { current: null },
      scanOperationActiveRef,
      terminalRunIdsRef: { current: new Set<string>() },
      appendWebSocketEvent: vi.fn(),
    };
    renderHook(() => useRuntimeSubscriptions(options));
    await act(async () => Promise.resolve());

    act(() => handlers.onScanProgress?.({ phase: "analyzing", completed: 1, total: 2 }));
    expect(storedProgress).toMatchObject({ phase: "analyzing" });
    act(() => handlers.onScanProgress?.({ phase: "committing", completed: 0, total: 0 }));
    expect(storedProgress).toMatchObject({ phase: "committing", total: 0 });
    act(() => handlers.onScanProgress?.({ phase: "complete", completed: 2, total: 2 }));
    expect(storedProgress).toBeNull();

    scanOperationActiveRef.current = false;
    act(() => handlers.onScanProgress?.({ phase: "committing", completed: 0, total: 0 }));
    expect(storedProgress).toBeNull();
    expect(setScanProgress).toHaveBeenCalledTimes(3);
  });

  it("coalesces 20 ms events, keeps the chronological 500 tail, and cleans up", async () => {
    vi.useFakeTimers();
    let handlers: { onRuntimeEvent?: (event: RuntimeEvent) => void } = {};
    const stop = vi.fn();
    bridgeMocks.subscribeToDesktopEvents.mockImplementation(async (nextHandlers) => {
      handlers = nextHandlers;
      return stop;
    });
    const clearTimeout = vi.spyOn(window, "clearTimeout");

    let storedEvents: RuntimeEvent[] = [];
    const setRuntimeEvents = vi.fn((update) => {
      storedEvents = typeof update === "function" ? update(storedEvents) : update;
    });
    const setActiveRun = vi.fn();
    const terminalRunIdsRef = { current: new Set<string>() };
    const options = {
      loadFilesAndGraph: vi.fn().mockResolvedValue(undefined),
      notify: vi.fn(),
      setScanProgress: vi.fn(),
      setRuntimeEvents,
      setActiveRun,
      activeRunRef: { current: { id: "run-burst", mode: "debug" as const, stopping: true } },
      scanOperationActiveRef: { current: true },
      terminalRunIdsRef,
      appendWebSocketEvent: vi.fn(),
    };
    const { unmount } = renderHook(() => useRuntimeSubscriptions(options));
    await act(async () => Promise.resolve());

    for (let index = 0; index < 5; index += 1) {
      act(() => {
        handlers.onRuntimeEvent?.(runtimeEvent(index));
        if (index < 4) vi.advanceTimersByTime(20);
      });
    }
    expect(setRuntimeEvents).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(20));
    expect(setRuntimeEvents).toHaveBeenCalledOnce();
    expect(storedEvents.map((event) => event.id)).toEqual([
      "event-0",
      "event-1",
      "event-2",
      "event-3",
      "event-4",
    ]);

    act(() => {
      for (let index = 1_000; index < 1_600; index += 1) {
        handlers.onRuntimeEvent?.(runtimeEvent(index));
      }
      vi.advanceTimersByTime(100);
    });
    expect(setRuntimeEvents).toHaveBeenCalledTimes(2);
    expect(storedEvents).toHaveLength(500);
    expect(storedEvents[0]?.id).toBe("event-1100");
    expect(storedEvents.at(-1)?.id).toBe("event-1599");

    act(() => handlers.onRuntimeEvent?.(runtimeEvent(1_600, "process.exited")));
    expect(terminalRunIdsRef.current).toContain("run-burst");
    expect(setActiveRun).toHaveBeenCalledOnce();
    expect(options.notify).toHaveBeenCalledWith("Managed process exited", "info");
    expect(setRuntimeEvents).toHaveBeenCalledTimes(2);

    unmount();
    expect(clearTimeout).toHaveBeenCalledTimes(1);
    expect(stop).toHaveBeenCalledOnce();
  });
});
