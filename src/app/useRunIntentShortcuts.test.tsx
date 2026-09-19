import { fireEvent, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ActiveRun } from "./model";
import { useRunIntentShortcuts } from "./useAppShortcuts";

describe("useRunIntentShortcuts", () => {
  it("routes the advertised run and debug shortcuts through the shared intent", () => {
    const onRunIntent = vi.fn();
    renderHook(() => useRunIntentShortcuts(null, onRunIntent));

    fireEvent.keyDown(window, { key: "r", metaKey: true });
    fireEvent.keyDown(window, { key: "r", metaKey: true, shiftKey: true });

    expect(onRunIntent).toHaveBeenNthCalledWith(1, "run");
    expect(onRunIntent).toHaveBeenNthCalledWith(2, "debug");
  });

  it("does not start another profile while a process is active", () => {
    const onRunIntent = vi.fn();
    const activeRun = {
      id: "run-1",
      mode: "run",
      stopping: false,
    } as ActiveRun;
    renderHook(() => useRunIntentShortcuts(activeRun, onRunIntent));

    fireEvent.keyDown(window, { key: "r", metaKey: true });

    expect(onRunIntent).not.toHaveBeenCalled();
  });
});
