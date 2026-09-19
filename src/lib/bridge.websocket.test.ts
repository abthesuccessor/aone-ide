import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  connectWebSocket,
  disconnectWebSocket,
  sendWebSocketMessage,
  subscribeToDesktopEvents,
} from "./bridge";
import type { WebSocketEvent } from "../types";

const eventMocks = vi.hoisted(() => ({ listen: vi.fn() }));

vi.mock("@tauri-apps/api/event", () => ({ listen: eventMocks.listen }));

beforeEach(() => {
  eventMocks.listen.mockReset();
});

describe("browser-demo WebSocket bridge", () => {
  it("rejects destinations outside the desktop WebSocket URL boundary", async () => {
    await expect(connectWebSocket({
      url: "https://example.test/not-a-socket",
      headers: [],
      protocols: [],
    })).rejects.toThrow(/valid ws or wss URL/i);
  });

  it("emits deterministic lifecycle events and removes listeners on cleanup", async () => {
    const events: WebSocketEvent[] = [];
    const unsubscribe = await subscribeToDesktopEvents({
      onWebSocketEvent: (event) => events.push(event),
    });

    const connection = await connectWebSocket({
      url: "ws://127.0.0.1:3000/ws?token=private",
      headers: [{ name: "authorization", value: "Bearer private" }],
      protocols: ["json"],
      timeoutMs: 1_000,
    });
    expect(events.map((event) => event.kind)).toEqual(["connecting", "open"]);
    expect(events[0]?.destination).toBe("ws://127.0.0.1:3000");
    expect(JSON.stringify(events)).not.toContain("token=private");
    expect(JSON.stringify(events)).not.toContain("Bearer private");

    await sendWebSocketMessage({
      sessionId: connection.sessionId,
      data: "ping",
      encoding: "text",
    });
    expect(events.at(-1)).toMatchObject({
      kind: "message",
      data: "ping",
      encoding: "text",
      byteLength: 4,
    });

    await disconnectWebSocket({ sessionId: connection.sessionId });
    expect(events.at(-1)?.kind).toBe("closed");
    const eventCount = events.length;
    unsubscribe();

    const ignored = await connectWebSocket({
      url: "wss://example.test/events",
      headers: [],
      protocols: [],
    });
    expect(events).toHaveLength(eventCount);
    await disconnectWebSocket({ sessionId: ignored.sessionId });
  });

  it("rolls back listeners installed before a later desktop listener fails", async () => {
    const stopScan = vi.fn();
    const stopRuntime = vi.fn();
    eventMocks.listen
      .mockResolvedValueOnce(stopScan)
      .mockResolvedValueOnce(stopRuntime)
      .mockRejectedValueOnce(new Error("workspace listener failed"));
    window.__TAURI_INTERNALS__ = {};

    await expect(subscribeToDesktopEvents({
      onScanProgress: vi.fn(),
      onRuntimeEvent: vi.fn(),
      onWorkspaceChanged: vi.fn(),
      onWebSocketEvent: vi.fn(),
    })).rejects.toThrow("workspace listener failed");

    expect(stopRuntime).toHaveBeenCalledOnce();
    expect(stopScan).toHaveBeenCalledOnce();
    expect(eventMocks.listen).toHaveBeenCalledTimes(3);
  });
});
