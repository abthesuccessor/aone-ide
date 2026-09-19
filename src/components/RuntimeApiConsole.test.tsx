import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type {
  ApiRequest,
  ApiResponse,
  RuntimeEvent,
  WebSocketConnectResult,
  WebSocketDisconnectResult,
  WebSocketSendResult,
} from "../types";
import { RuntimeApiConsole } from "./RuntimeApiConsole";

const baseProps = {
  isRunning: false,
  collapsed: false,
  onToggleCollapsed: vi.fn(),
  onSelectRuntimeNode: vi.fn(),
  webSocketEvents: [],
  webSocketEventSubscriptionReady: true,
  onWebSocketConnect: vi.fn().mockResolvedValue({
    sessionId: "test-session",
  } satisfies WebSocketConnectResult),
  onWebSocketSend: vi.fn().mockResolvedValue({
    accepted: true,
  } satisfies WebSocketSendResult),
  onWebSocketDisconnect: vi.fn().mockResolvedValue({
    disconnected: true,
  } satisfies WebSocketDisconnectResult),
};

describe("RuntimeApiConsole", () => {
  it("renders backend process output from metadata.text", () => {
    const event: RuntimeEvent = {
      id: "stdout-1",
      runId: "run-1",
      kind: "process.stdout",
      timestamp: "2026-08-16T10:00:00Z",
      label: "stdout",
      evidence: "observed",
      metadata: { text: "API listening on 127.0.0.1:4310" },
    };

    render(
      <RuntimeApiConsole
        {...baseProps}
        events={[event]}
        onSendRequest={vi.fn()}
      />,
    );

    expect(screen.getByText("API listening on 127.0.0.1:4310")).toBeVisible();
    expect(screen.queryByText("stdout")).not.toBeInTheDocument();
  });

  it("opens a boundary event in the System flow while leaving ordinary output non-interactive", () => {
    const select = vi.fn();
    const boundary: RuntimeEvent = {
      id: "http-1",
      kind: "http.completed",
      timestamp: "2026-08-16T10:00:00Z",
      label: "GET /orders",
      evidence: "observed",
      metadata: { method: "GET", path: "/orders" },
    };
    render(<RuntimeApiConsole {...baseProps} events={[boundary]} onSelectRuntimeNode={select} onSendRequest={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: /GET \/orders/ }));
    expect(select).toHaveBeenCalledWith("runtime:http-1");
  });

  it("sends GraphQL variables and displays actual response headers", async () => {
    const response: ApiResponse = {
      requestId: "request-1",
      status: 200,
      statusText: "OK",
      headers: [{ name: "x-trace-id", value: "trace-1047" }],
      body: "{\"data\":{}}",
      durationMs: 12,
      truncated: false,
    };
    const send = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>().mockResolvedValue(response);
    render(
      <RuntimeApiConsole
        {...baseProps}
        events={[]}
        initialTab="api"
        onSendRequest={send}
      />,
    );

    expect(screen.getByText("Sensitive headers are redacted in session evidence")).toBeVisible();
    fireEvent.click(screen.getByRole("tab", { name: "GraphQL" }));
    fireEvent.change(screen.getByLabelText("GraphQL variables"), {
      target: { value: '{"projectId":"project-1"}' },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(send).toHaveBeenCalledTimes(1));
    const request = send.mock.calls[0]?.[0];
    expect(request).toBeDefined();
    const body = JSON.parse(request?.body ?? "{}");
    expect(body.variables).toEqual({ projectId: "project-1" });
    expect(request?.headers).toContainEqual({ name: "content-type", value: "application/json" });
    expect(await screen.findByText("x-trace-id")).toBeVisible();
    expect(screen.getByText("trace-1047")).toBeVisible();
  });

  it("uses the native 60 second timeout while cooperative Debug is active", async () => {
    const send = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>().mockResolvedValue({
      requestId: "request-debug",
      status: 200,
      statusText: "OK",
      headers: [],
      body: "{}",
      durationMs: 20_000,
      truncated: false,
    });
    render(
      <RuntimeApiConsole
        {...baseProps}
        events={[]}
        initialTab="api"
        debugActive
        onSendRequest={send}
      />,
    );

    expect(screen.getByText(/Debug requests allow up to 60 seconds/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(send).toHaveBeenCalledWith(expect.objectContaining({ timeoutMs: 60_000 })));
  });

  it("clears API request and response state when the workspace scope changes", async () => {
    const send = vi.fn<(request: ApiRequest) => Promise<ApiResponse>>().mockResolvedValue({
      requestId: "request-auth",
      status: 200,
      statusText: "OK",
      headers: [],
      body: '{"token":"redacted"}',
      durationMs: 10,
      truncated: false,
    });
    const view = render(
      <RuntimeApiConsole
        {...baseProps}
        events={[]}
        initialTab="api"
        workspaceScope="auth:1"
        onSendRequest={send}
      />,
    );
    fireEvent.change(screen.getByLabelText("Request URL"), {
      target: { value: "http://127.0.0.1:4310/api/login" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText('{"token":"redacted"}')).toBeVisible();

    view.rerender(
      <RuntimeApiConsole
        {...baseProps}
        events={[]}
        initialTab="api"
        workspaceScope="trace-demo:2"
        onSendRequest={send}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText("Request URL"))
      .toHaveValue("http://127.0.0.1:3000/"));
    expect(screen.queryByText('{"token":"redacted"}')).not.toBeInTheDocument();
    expect(screen.getByText("Compose and send a local request.")).toBeVisible();
  });
});
