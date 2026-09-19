import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WebSocketEvent } from "../../types";
import { useWebSocketController } from "./useWebSocketController";

function messageEvent(index: number): WebSocketEvent {
  return {
    sessionId: "burst-session",
    kind: "message",
    timestamp: new Date(index).toISOString(),
    destination: "ws://127.0.0.1:3000",
    data: String(index),
    encoding: "text",
    byteLength: String(index).length,
    truncated: false,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useWebSocketController", () => {
  it("batches burst events into one animation frame and keeps a hard cap", () => {
    let scheduledFrame: FrameRequestCallback | undefined;
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      scheduledFrame = callback;
      return 42;
    });
    const cancelFrame = vi.fn();
    vi.stubGlobal("requestAnimationFrame", requestFrame);
    vi.stubGlobal("cancelAnimationFrame", cancelFrame);

    const { result, unmount } = renderHook(() => useWebSocketController());
    act(() => {
      for (let index = 0; index < 350; index += 1) {
        result.current.appendWebSocketEvent(messageEvent(index));
      }
    });

    expect(requestFrame).toHaveBeenCalledTimes(1);
    expect(result.current.webSocketEvents).toHaveLength(0);
    act(() => scheduledFrame?.(0));
    expect(result.current.webSocketEvents).toHaveLength(300);
    expect(result.current.webSocketEvents[0]?.data).toBe("50");

    act(() => result.current.appendWebSocketEvent(messageEvent(351)));
    unmount();
    expect(cancelFrame).toHaveBeenCalledWith(42);
  });
});

