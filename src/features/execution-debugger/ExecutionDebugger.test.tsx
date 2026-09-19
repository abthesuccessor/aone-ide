import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApiResponse, GraphSnapshot } from "../../types";
import { ExecutionDebugger } from "./ExecutionDebugger";
import { event, session } from "./ExecutionDebugger.testFixtures";

function reportedNodes(container: HTMLElement) {
  return container.querySelectorAll<HTMLButtonElement>(".trace-path-node[data-debug-step-id]");
}

describe("ExecutionDebugger API trace workbench", () => {
  it("renders four resizable panes and keeps the current safe point synchronized to source preview", async () => {
    const onOpenSource = vi.fn();
    const view = session([
      event(1),
      event(2, {
        kind: "database",
        flowStage: "data",
        label: "Load recommendation shape",
        operation: "selectRecommendations",
        resource: "recommendations",
        dataPreview: {
          kind: "rowSet",
          typeName: "Recommendation",
          fields: [{ name: "recommendation_id", valueType: "uuid", nullable: false }],
          rowCount: 4,
          truncated: false,
        },
      }),
    ]);
    const { container } = render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={view}
        onControl={vi.fn().mockResolvedValue(undefined)}
        onOpenSource={onOpenSource}
        sourcePane={<section aria-label="Source editor">Source</section>}
      />,
    );

    expect(screen.getByRole("region", { name: "API trace workbench" })).toBeVisible();
    expect(screen.getByRole("region", { name: "API client" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Source editor" })).toBeVisible();
    expect(screen.getByRole("region", { name: "API execution path" })).toBeVisible();
    expect(screen.getByRole("region", { name: "Trace data and values" })).toBeVisible();
    expect(screen.getAllByRole("separator")).toHaveLength(3);
    expect(reportedNodes(container)).toHaveLength(2);

    const current = container.querySelector("[data-debug-step-id$='step-2']");
    expect(current).toHaveClass("is-current");
    expect(current).toHaveAttribute("aria-current", "step");
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ relativePath: "src/service.rs", startLine: 42 }),
      "preview",
    ));

    const shape = screen.getByRole("region", { name: "Reported data shape" });
    expect(within(shape).getByText("recommendation_id")).toBeVisible();
    expect(within(shape).getByText("uuid")).toBeVisible();
    expect(within(shape).getByText("added")).toBeVisible();
    expect(screen.getByText("selectRecommendations")).toBeVisible();
    expect(screen.getByText("recommendations")).toBeVisible();
  });

  it("follows a newly arrived terminal safe point after the live current step is cleared", async () => {
    const onControl = vi.fn().mockResolvedValue(undefined);
    const onOpenSource = vi.fn();
    const initialEvents = [event(1), event(2)];
    const { container, rerender } = render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={session(initialEvents)}
        onControl={onControl}
        onOpenSource={onOpenSource}
      />,
    );

    expect(container.querySelector("[data-debug-step-id$='step-2']")).toHaveClass("is-current");
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ relativePath: "src/service.rs", startLine: 42 }),
      "preview",
    ));

    const terminalEvent = event(3, {
      kind: "response",
      flowStage: "response",
      label: "HTTP response complete",
      safePointState: "workflowCompleted",
      source: { relativePath: "src/server.rs", line: 88 },
    });
    rerender(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={session([...initialEvents, terminalEvent], {
          status: "completed",
          activeWorkflowId: undefined,
          currentStepId: undefined,
          endedAt: "2026-08-17T02:01:00Z",
        })}
        onControl={onControl}
        onOpenSource={onOpenSource}
      />,
    );

    await waitFor(() => {
      expect(container.querySelector("[data-debug-step-id$='step-3']"))
        .toHaveClass("is-replay-position");
    });
    expect(container.querySelector("[data-debug-step-id$='step-2']"))
      .not.toHaveClass("is-replay-position");
    expect(screen.getByLabelText("3 of 3 safe points")).toHaveTextContent("3 / 3");
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ relativePath: "src/server.rs", startLine: 88 }),
      "preview",
    ));
  });

  it("renders only the selected workflow and never lights a historical workflow as current", async () => {
    const onOpenSource = vi.fn();
    const view = session([
      event(1, {
        id: "old-event",
        workflowId: "workflow-old",
        stepId: "shared",
        label: "Old request",
      }),
      event(2, {
        id: "active-event",
        workflowId: "workflow-active",
        stepId: "shared",
        label: "Active request",
      }),
    ]);
    const { container } = render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={view}
        onControl={vi.fn().mockResolvedValue(undefined)}
        onOpenSource={onOpenSource}
      />,
    );
    const path = screen.getByRole("region", { name: "API execution path" });

    expect(within(path).getByRole("button", { name: /Active request/ })).toHaveClass("is-current");
    expect(within(path).queryByRole("button", { name: /Old request/ })).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Workflow"), { target: { value: "workflow-old" } });

    const historical = await within(path).findByRole("button", { name: /Old request/ });
    expect(historical).toHaveClass("is-replay-position");
    expect(historical).not.toHaveClass("is-current");
    expect(container.querySelectorAll(".trace-path-node.is-current")).toHaveLength(0);
    expect(within(path).queryByRole("button", { name: /Active request/ })).not.toBeInTheDocument();
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ startLine: 41 }),
      "preview",
    ));
  });

  it("moves replay selection with Prev and Next and previews each selected source line", async () => {
    const onOpenSource = vi.fn();
    const view = session([event(1), event(2), event(3)], {
      status: "completed",
      activeWorkflowId: undefined,
      currentStepId: undefined,
      endedAt: "2026-08-17T02:01:00Z",
    });
    const { container } = render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive={false}
        session={view}
        onOpenSource={onOpenSource}
      />,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Complete");
    expect(container.querySelector("[data-debug-step-id$='step-3']")).toHaveClass("is-replay-position");
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ startLine: 43 }),
      "preview",
    ));

    fireEvent.click(screen.getByRole("button", { name: "Previous event" }));
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ startLine: 42 }),
      "preview",
    ));
    expect(container.querySelector("[data-debug-step-id$='step-2']")).toHaveClass("is-replay-position");

    fireEvent.click(screen.getByRole("button", { name: "Next event" }));
    await waitFor(() => expect(onOpenSource).toHaveBeenLastCalledWith(
      expect.objectContaining({ startLine: 43 }),
      "preview",
    ));
    expect(container.querySelector("[data-debug-step-id$='step-3']")).toHaveClass("is-replay-position");
  });

  it("uses Next at the live tail as one cooperative step-over request", () => {
    const onControl = vi.fn().mockResolvedValue(undefined);
    render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={session([event(1), event(2)])}
        onControl={onControl}
        onOpenSource={vi.fn()}
      />,
    );

    const next = screen.getByRole("button", { name: "Next safe point" });
    expect(next).toBeEnabled();
    fireEvent.click(next);
    expect(onControl).toHaveBeenCalledTimes(1);
    expect(onControl).toHaveBeenCalledWith("stepOver");
  });

  it("blocks duplicate controls while the backend is awaiting acknowledgement", () => {
    const onControl = vi.fn().mockResolvedValue(undefined);
    const view = session([event(1)], {
      status: "running",
      pendingAction: "pause",
      pendingControlEpoch: 3,
    });
    render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={view}
        onControl={onControl}
        onOpenSource={vi.fn()}
      />,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Pausing…");
    expect(screen.getByRole("button", { name: "Pause" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next safe point" })).toBeDisabled();
    fireEvent.keyDown(window, { key: "F10" });
    expect(onControl).not.toHaveBeenCalled();
  });

  it("renders one bounded 120-node page and can page to earlier safe points", () => {
    const events = Array.from({ length: 240 }, (_, index) => event(index + 1));
    const { container } = render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive={false}
        session={session(events)}
        onOpenSource={vi.fn()}
      />,
    );

    expect(reportedNodes(container)).toHaveLength(120);
    expect(screen.getByText("121–240 / 240")).toBeVisible();
    expect(container.querySelector("[data-debug-step-id$='step-240']")).toBeInTheDocument();
    expect(container.querySelector("[data-debug-step-id$='step-1']")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Earlier safe points" }));
    expect(reportedNodes(container)).toHaveLength(120);
    expect(screen.getByText("1–120 / 240")).toBeVisible();
    expect(container.querySelector("[data-debug-step-id$='step-1']")).toBeInTheDocument();
    expect(container.querySelector("[data-debug-step-id$='step-240']")).not.toBeInTheDocument();
  });

  it("separates reported shape metadata from API request and response values", async () => {
    const response: ApiResponse = {
      requestId: "request-1",
      status: 200,
      statusText: "OK",
      headers: [{ name: "content-type", value: "application/json" }],
      body: '{"shipment":{"status":"in_transit"}}',
      durationMs: 18,
      truncated: false,
    };
    const onSendRequest = vi.fn().mockResolvedValue(response);
    const view = session([
      event(1),
      event(2, {
        kind: "database",
        flowStage: "data",
        operation: "SELECT",
        resource: "shipments",
        dataPreview: {
          kind: "rowSet",
          typeName: "Shipment",
          fields: [{ name: "status", valueType: "string", nullable: false }],
          rowCount: 1,
          nullCount: 0,
          truncated: false,
        },
      }),
    ]);
    render(
      <ExecutionDebugger
        events={[]}
        runId="run-debug"
        observeActive
        processActive
        session={view}
        onControl={vi.fn().mockResolvedValue(undefined)}
        onOpenSource={vi.fn()}
        onSendRequest={onSendRequest}
      />,
    );

    expect(screen.getByText("Runtime internals are shape and count metadata only. Request and response tabs show the values available to the API client.")).toBeVisible();
    const shape = screen.getByRole("region", { name: "Reported data shape" });
    expect(within(shape).getByText("status")).toBeVisible();
    expect(within(shape).getByText("string")).toBeVisible();
    expect(screen.queryByText("in_transit")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("HTTP method"), { target: { value: "POST" } });
    fireEvent.change(screen.getByLabelText("Request body"), {
      target: { value: '{"cartId":"cart_99","authorization":"body-visible"}' },
    });
    const requestEditor = screen.getByRole("tablist", { name: "Request editor" });
    fireEvent.click(within(requestEditor).getByRole("tab", { name: "Headers" }));
    fireEvent.change(screen.getByLabelText("Request headers"), {
      target: {
        value: "content-type: application/json\nauthorization: Bearer hidden-value\nx-debug: visible",
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() => expect(onSendRequest).toHaveBeenCalledWith(expect.objectContaining({
      body: '{"cartId":"cart_99","authorization":"body-visible"}',
      timeoutMs: 60_000,
    })));

    const dataPane = screen.getByRole("region", { name: "Trace data and values" });
    fireEvent.click(within(dataPane).getByRole("tab", { name: "Request" }));
    expect(within(dataPane).getByText(/^cartId:/)).toBeVisible();
    expect(within(dataPane).getByText('"cart_99"')).toBeVisible();
    expect(within(dataPane).getByText('"body-visible"')).toBeVisible();
    fireEvent.click(within(dataPane).getByText("3 request headers"));
    expect(within(dataPane).getByText("[hidden in trace view]")).toBeVisible();
    expect(within(dataPane).queryByText("Bearer hidden-value")).not.toBeInTheDocument();

    fireEvent.click(within(dataPane).getByRole("tab", { name: /Response/ }));
    expect(within(dataPane).getByText("200 OK")).toBeVisible();
    expect(within(dataPane).getByText('"in_transit"')).toBeVisible();
    expect(within(dataPane).queryByText(/line-local|stack local/i)).not.toBeInTheDocument();
  });

  it("replays retained OTLP observations without presenting the replay position as live", async () => {
    vi.useFakeTimers();
    try {
      const traceId = "5b8efff798038103d269b633813fc60c";
      const spans = [
        { id: "root", spanId: "1111111111111111", label: "GET /shipments" },
        {
          id: "child",
          spanId: "2222222222222222",
          parentSpanId: "1111111111111111",
          label: "ShipmentService.get",
        },
      ].map((span, index) => ({
        id: span.id,
        runId: "otlp:receiver",
        traceId,
        kind: index === 0 ? "trace.http.server.event" : "trace.function.event",
        timestamp: `2026-08-20T00:00:0${index}Z`,
        label: span.label,
        evidence: "observed" as const,
        metadata: {
          traceProtocol: "OTLP_HTTP_JSON",
          spanId: span.spanId,
          phase: "event",
          ...(span.parentSpanId ? { parentSpanId: span.parentSpanId } : {}),
        },
      }));
      const { container } = render(
        <ExecutionDebugger events={spans} observeActive={false} onOpenSource={vi.fn()} />,
      );

      expect(screen.getByRole("group", { name: "Trace replay controls" })).toBeVisible();
      fireEvent.click(screen.getByRole("button", { name: "Replay" }));
      expect(screen.getByRole("button", { name: "Pause" })).toBeEnabled();
      const first = container.querySelector("[data-debug-step-id$='1111111111111111']");
      expect(first).toHaveClass("is-replay-position");
      expect(first).not.toHaveClass("is-current");

      await act(async () => vi.advanceTimersByTime(700));

      const second = container.querySelector("[data-debug-step-id$='2222222222222222']");
      expect(second).toHaveClass("is-replay-position");
      expect(second).not.toHaveClass("is-current");
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows an indexed API path as static evidence without a live light", () => {
    const indexedGraph: GraphSnapshot = {
      truncated: false,
      nodes: [
        {
          id: "endpoint",
          kind: "endpoint",
          label: "GET /api/shipments/:id",
          evidence: "declared",
          metadata: { flowRoot: true, flowStage: "api" },
          source: {
            relativePath: "src/server.js",
            startLine: 8,
            startColumn: 1,
            endLine: 8,
            endColumn: 20,
          },
        },
        {
          id: "repository",
          kind: "repository",
          label: "ShipmentRepository.findById",
          evidence: "resolved",
          metadata: { flowStage: "data" },
        },
      ],
      edges: [{
        id: "calls-repository",
        source: "endpoint",
        target: "repository",
        kind: "calls",
        evidence: "resolved",
        confidence: 1,
        metadata: {},
      }],
    };
    const { container } = render(
      <ExecutionDebugger
        events={[]}
        observeActive={false}
        indexedGraph={indexedGraph}
        indexedRootId="endpoint"
        target={{ id: "endpoint", method: "GET", path: "/api/shipments/:id", handler: { id: "repository" } }}
        onOpenSource={vi.fn()}
      />,
    );

    expect(screen.getByRole("list", { name: "Indexed API code path" })).toBeVisible();
    expect(screen.getByText(/lights appear only for reported runtime safe points/i)).toBeVisible();
    expect(container.querySelectorAll(".trace-path-node.is-indexed")).toHaveLength(2);
    expect(container.querySelectorAll(".trace-path-node.is-current")).toHaveLength(0);
  });

});
