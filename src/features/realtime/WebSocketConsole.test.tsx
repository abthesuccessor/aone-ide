import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
  WebSocketConnectRequest,
  WebSocketEvent,
  WebSocketSendRequest,
} from "../../types";
import { WebSocketConsole } from "./WebSocketConsole";

function event(
  sessionId: string,
  kind: WebSocketEvent["kind"],
  detail?: string,
): WebSocketEvent {
  return {
    sessionId,
    kind,
    timestamp: new Date().toISOString(),
    destination: "ws://127.0.0.1:3000",
    ...(kind === "message"
      ? { data: detail, encoding: "text", byteLength: detail?.length ?? 0 }
      : { detail }),
    truncated: false,
  };
}

describe("WebSocketConsole", () => {
  it("derives open/message/closed state from events while running the full client flow", async () => {
    const connect = vi.fn<(request: WebSocketConnectRequest) => Promise<{ sessionId: string }>>();
    const send = vi.fn<(request: WebSocketSendRequest) => Promise<{ accepted: boolean }>>();
    const disconnect = vi.fn<() => Promise<{ disconnected: boolean }>>();

    function Harness() {
      const [events, setEvents] = useState<WebSocketEvent[]>([]);
      connect.mockImplementationOnce(async () => {
        setEvents([event("session-1", "connecting", "Awaiting handshake")]);
        await Promise.resolve();
        setEvents((current) => [...current, event("session-1", "open", "Handshake complete")]);
        return { sessionId: "session-1" };
      });
      send.mockImplementationOnce(async (request) => {
        setEvents((current) => [...current, event(request.sessionId, "message", request.data)]);
        return { accepted: true };
      });
      disconnect.mockImplementationOnce(async () => {
        setEvents((current) => [...current, event("session-1", "closed", "Client closed")]);
        return { disconnected: true };
      });
      return (
        <WebSocketConsole
          events={events}
          eventSubscriptionReady
          onConnect={connect}
          onSend={send}
          onDisconnect={disconnect}
        />
      );
    }

    render(<Harness />);
    expect(screen.getByText("Not connected")).toBeVisible();
    expect(screen.getByText(/native consent before connecting/i)).toBeVisible();

    fireEvent.change(screen.getByLabelText("WebSocket subprotocols"), {
      target: { value: "json, telemetry, json" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(connect).toHaveBeenCalledTimes(1));
    expect(connect.mock.calls[0]?.[0].protocols).toEqual(["json", "telemetry"]);
    expect(await screen.findByText("Open")).toBeVisible();
    expect(screen.getByText("Handshake complete")).toBeVisible();

    fireEvent.change(screen.getByLabelText("WebSocket message"), {
      target: { value: "hello from Aone" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(send).toHaveBeenCalledWith({
      sessionId: "session-1",
      data: "hello from Aone",
      encoding: "text",
    }));
    expect(within(screen.getByRole("log")).getByText("hello from Aone")).toBeVisible();
    expect(screen.getByText("Open")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    await waitFor(() => expect(disconnect).toHaveBeenCalledWith({ sessionId: "session-1" }));
    expect(await screen.findByText("Closed")).toBeVisible();
  });

  it("keeps connect disabled until event subscription is ready", () => {
    const { rerender } = render(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady={false}
        onConnect={vi.fn()}
        onSend={vi.fn()}
        onDisconnect={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    expect(screen.getByText(/preparing the real-time event listener/i)).toBeVisible();
    rerender(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady
        onConnect={vi.fn()}
        onSend={vi.fn()}
        onDisconnect={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
  });

  it("rejects a timeout outside the native contract before invoking connect", async () => {
    const connect = vi.fn();
    render(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady
        onConnect={connect}
        onSend={vi.fn()}
        onDisconnect={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText("WebSocket timeout"), {
      target: { value: "100" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText(/integer from 250 to 60000/i)).toBeVisible();
    expect(connect).not.toHaveBeenCalled();
  });

  it("uses a successful connect result when lifecycle events were missed", async () => {
    const send = vi.fn().mockResolvedValue({ accepted: true });
    const disconnect = vi.fn().mockResolvedValue({ disconnected: true });
    render(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady
        onConnect={vi.fn().mockResolvedValue({ sessionId: "quiet-session" })}
        onSend={send}
        onDisconnect={disconnect}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Open")).toBeVisible();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    await waitFor(() => expect(disconnect).toHaveBeenCalledWith({ sessionId: "quiet-session" }));
    expect(screen.getByText("Closed")).toBeVisible();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeDisabled();
  });

  it("allows retry when connect rejects after only a connecting event", async () => {
    function Harness() {
      const [events, setEvents] = useState<WebSocketEvent[]>([]);
      return (
        <WebSocketConsole
          events={events}
          eventSubscriptionReady
          onConnect={vi.fn().mockImplementation(async () => {
            setEvents([event("failed-session", "connecting", "Awaiting handshake")]);
            throw new Error("Handshake rejected");
          })}
          onSend={vi.fn()}
          onDisconnect={vi.fn()}
        />
      );
    }

    render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Handshake rejected")).toBeVisible();
    expect(screen.getByText("Error")).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
  });

  it("clears command-confirmed open state when a terminal event arrives", async () => {
    const props = {
      eventSubscriptionReady: true,
      onConnect: vi.fn().mockResolvedValue({ sessionId: "terminal-session" }),
      onSend: vi.fn(),
      onDisconnect: vi.fn(),
    };
    const { rerender } = render(<WebSocketConsole {...props} events={[]} />);

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Open")).toBeVisible();
    rerender(
      <WebSocketConsole
        {...props}
        events={[event("terminal-session", "closed", "Peer closed")]}
      />,
    );
    expect(screen.getByText("Closed")).toBeVisible();

    rerender(<WebSocketConsole {...props} events={[]} />);
    expect(screen.getByText("Not connected")).toBeVisible();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("renders bounded dropped-message summaries without closing the session", () => {
    render(
      <WebSocketConsole
        events={[{
          ...event("busy-session", "dropped"),
          droppedCount: 17,
        }]}
        eventSubscriptionReady
        onConnect={vi.fn()}
        onSend={vi.fn()}
        onDisconnect={vi.fn()}
      />,
    );

    expect(screen.getByText("Open")).toBeVisible();
    expect(screen.getByText("17 inbound messages omitted")).toBeVisible();
    expect(screen.getByRole("button", { name: "Send" })).toBeEnabled();
  });

  it("preserves safe string rejections from every Tauri WebSocket command", async () => {
    const send = vi.fn().mockRejectedValue("Message exceeds the desktop limit");
    const disconnect = vi.fn().mockRejectedValue("Session ownership changed");
    const { unmount } = render(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady
        onConnect={vi.fn().mockRejectedValue("Destination blocked by desktop policy")}
        onSend={send}
        onDisconnect={disconnect}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Destination blocked by desktop policy")).toBeVisible();
    unmount();

    render(
      <WebSocketConsole
        events={[]}
        eventSubscriptionReady
        onConnect={vi.fn().mockResolvedValue({ sessionId: "error-session" })}
        onSend={send}
        onDisconnect={disconnect}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Open")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("Message exceeds the desktop limit")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(await screen.findByText("Session ownership changed")).toBeVisible();
  });
});
