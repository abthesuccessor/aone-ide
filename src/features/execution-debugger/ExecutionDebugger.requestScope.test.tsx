import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApiResponse } from "../../types";
import { ExecutionDebugger } from "./ExecutionDebugger";
import { event, session } from "./ExecutionDebugger.testFixtures";

describe("ExecutionDebugger request scope", () => {
  it("excludes workflows that already existed when Send was clicked", async () => {
    let completeRequest!: (response: ApiResponse) => void;
    const onSendRequest = vi.fn(() => new Promise<ApiResponse>((resolve) => {
      completeRequest = resolve;
    }));
    const oldEvent = event(1, {
      workflowId: "workflow-old",
      stepId: "old-entry",
      label: "Older request",
    });
    const props = {
      events: [],
      runId: "run-debug",
      observeActive: true,
      processActive: true,
      onControl: vi.fn().mockResolvedValue(undefined),
      onOpenSource: vi.fn(),
      onSendRequest,
    };
    const view = render(<ExecutionDebugger {...props} session={session([oldEvent])} />);

    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(onSendRequest).toHaveBeenCalledOnce());

    const afterSend = new Date(Date.now() + 1_000).toISOString();
    const oldTail = event(2, {
      workflowId: "workflow-old",
      stepId: "old-complete",
      label: "Older request completed",
      timestamp: afterSend,
      safePointState: "workflowCompleted",
    });
    const newEntry = event(3, {
      workflowId: "workflow-new",
      stepId: "new-entry",
      label: "Requested API entry",
      timestamp: afterSend,
    });
    view.rerender(<ExecutionDebugger {...props} session={session([oldEvent, oldTail, newEntry])} />);

    const path = screen.getByRole("region", { name: "API execution path" });
    expect(within(path).getByRole("button", { name: /Requested API entry/ })).toBeVisible();
    expect(within(path).queryByRole("button", { name: /Older request/ })).not.toBeInTheDocument();

    await act(async () => completeRequest({
      requestId: "request-new",
      status: 200,
      statusText: "OK",
      headers: [],
      body: "{}",
      durationMs: 1,
      truncated: false,
    }));
  });
});
