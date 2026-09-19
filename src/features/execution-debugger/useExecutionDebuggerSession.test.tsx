import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RuntimeEvent } from "../../types";
import type { DebugSessionTransport } from "./bridge";
import type { DebugSessionView, DebuggerStatus } from "./contracts";
import { useExecutionDebuggerSession } from "./useExecutionDebuggerSession";

function capability(supported = true, cooperative = true) {
  return { supported, cooperative };
}

function snapshot(
  sequence: number,
  status: DebuggerStatus = "paused",
  pendingAction?: DebugSessionView["pendingAction"],
): DebugSessionView {
  return {
    debugSessionId: "debug-1",
    runId: "run-1",
    status,
    eventCount: sequence,
    controlEpoch: pendingAction ? 3 : 2,
    acknowledgedControlEpoch: 2,
    currentStepId: `step-${sequence}`,
    activeWorkflowId: status === "stopped" ? undefined : "workflow-1",
    workflowCount: 1,
    startedAt: "2026-08-17T02:00:00Z",
    capabilities: {
      pause: capability(), resume: capability(), stepInto: capability(),
      stepOver: capability(), stop: capability(true, false),
    },
    limitation: "Cooperative safe points only.",
    events: Array.from({ length: sequence }, (_, index) => ({
      id: `event-${index + 1}`,
      workflowId: "workflow-1",
      sequence: index + 1,
      controlEpoch: pendingAction ? 3 : 2,
      stepId: `step-${index + 1}`,
      kind: index === 0 ? "request" : "method",
      label: `Safe point ${index + 1}`,
      flowStage: index === 0 ? "trigger" : "service",
      source: { relativePath: "src/service.rs", line: index + 1 },
      safePointState: status === "paused" ? "paused" : "running",
      timestamp: `2026-08-17T02:00:${String(index + 1).padStart(2, "0")}Z`,
    })),
    eventsTruncated: false,
    ...(pendingAction ? { pendingAction, pendingControlEpoch: 3 } : {}),
  };
}

function runtimeEvent(id: number, kind = "debug.method", state = "running"): RuntimeEvent {
  return {
    id: `runtime-${id}`,
    runId: "run-1",
    kind,
    timestamp: `2026-08-17T02:01:${String(id % 60).padStart(2, "0")}Z`,
    label: `Runtime ${id}`,
    evidence: "observed",
    metadata: { safePointState: state, sequence: id },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

function transport(getSession = vi.fn().mockResolvedValue(snapshot(1))): DebugSessionTransport {
  return {
    getSession,
    control: vi.fn().mockResolvedValue({ accepted: true, session: snapshot(1, "paused", "stepOver") }),
  };
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useExecutionDebuggerSession", () => {
  it("uses canonical expected sequence and keeps pending controls until acknowledgement", async () => {
    const api = transport();
    const { result } = renderHook(() => useExecutionDebuggerSession({
      workspaceGeneration: 1, events: [], transport: api,
    }));
    await act(async () => result.current.begin("run-1"));

    await act(async () => result.current.control("stepOver"));

    expect(api.control).toHaveBeenCalledWith({
      runId: "run-1",
      debugSessionId: "debug-1",
      action: "stepOver",
      expectedSequence: 1,
      expectedControlEpoch: 2,
    });
    expect(result.current.session?.pendingAction).toBe("stepOver");
  });

  it("coalesces sampled running reports and advances the canonical workflow", async () => {
    const getSession = vi.fn()
      .mockResolvedValueOnce(snapshot(1, "running"))
      .mockResolvedValue(snapshot(10, "running"));
    const api = transport(getSession);
    const { result, rerender } = renderHook(
      ({ events }) => useExecutionDebuggerSession({ workspaceGeneration: 1, events, transport: api }),
      { initialProps: { events: [] as RuntimeEvent[] } },
    );
    await act(async () => result.current.begin("run-1"));

    for (let index = 1; index <= 100; index += 1) {
      rerender({ events: Array.from({ length: index }, (_, item) => runtimeEvent(item + 1)) });
    }
    await flushPromises();

    expect(getSession.mock.calls.length).toBeLessThanOrEqual(2);
    await act(async () => { vi.advanceTimersByTime(250); });
    await flushPromises();
    expect(getSession.mock.calls.length).toBeLessThanOrEqual(3);
    expect(result.current.session?.eventCount).toBe(10);
  });

  it("retains the final canonical snapshot after process exit", async () => {
    const getSession = vi.fn()
      .mockResolvedValueOnce(snapshot(1, "running"))
      .mockResolvedValueOnce(snapshot(1, "stopped"));
    const api = transport(getSession);
    const { result, rerender } = renderHook(
      ({ events }) => useExecutionDebuggerSession({ workspaceGeneration: 1, events, transport: api }),
      { initialProps: { events: [] as RuntimeEvent[] } },
    );
    await act(async () => result.current.begin("run-1"));
    rerender({ events: [runtimeEvent(2, "process.exited")] });
    await flushPromises();

    expect(result.current.session?.status).toBe("stopped");
    expect(result.current.runId).toBe("run-1");
  });

  it("does not let an older running response replace a terminal snapshot", async () => {
    const older = deferred<DebugSessionView>();
    const newer = deferred<DebugSessionView>();
    const getSession = vi.fn()
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise);
    const api = transport(getSession);
    const { result, rerender } = renderHook(
      ({ events }) => useExecutionDebuggerSession({ workspaceGeneration: 1, events, transport: api }),
      { initialProps: { events: [] as RuntimeEvent[] } },
    );
    let beginning: Promise<void> = Promise.resolve();
    act(() => { beginning = result.current.begin("run-1"); });
    rerender({ events: [runtimeEvent(2, "process.exited")] });
    await flushPromises();
    expect(getSession).toHaveBeenCalledTimes(2);

    await act(async () => { newer.resolve(snapshot(1, "stopped")); await newer.promise; });
    expect(result.current.session?.status).toBe("stopped");
    await act(async () => { older.resolve(snapshot(1, "running")); await beginning; });

    expect(result.current.session?.status).toBe("stopped");
    expect(result.current.runId).toBe("run-1");
  });

  it("clears on workspace switch and rejects a late snapshot from the old generation", async () => {
    const pending = deferred<DebugSessionView>();
    const api = transport(vi.fn().mockReturnValue(pending.promise));
    const { result, rerender } = renderHook(
      ({ generation }) => useExecutionDebuggerSession({
        workspaceGeneration: generation, events: [], transport: api,
      }),
      { initialProps: { generation: 1 } },
    );
    let beginning: Promise<void> = Promise.resolve();
    act(() => { beginning = result.current.begin("run-1"); });
    rerender({ generation: 2 });
    await act(async () => { pending.resolve(snapshot(1)); await beginning; });

    expect(result.current.runId).toBeUndefined();
    expect(result.current.session).toBeNull();
    expect(result.current.error).toBeNull();
  });
});
