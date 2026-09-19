import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ExecutionDebugger } from "./ExecutionDebugger";

describe("ExecutionDebugger workspace isolation", () => {
  it("does not carry an API draft from one workspace into the next", async () => {
    const authTarget = {
      id: "auth-login",
      method: "POST",
      path: "/api/v1/login",
    };
    const view = render(
      <ExecutionDebugger
        events={[]}
        observeActive={false}
        workspaceId="auth"
        workspaceGeneration={1}
        target={authTarget}
        onOpenSource={vi.fn()}
      />,
    );

    const requestUrl = screen.getByLabelText("Request URL");
    await waitFor(() => expect(requestUrl).toHaveValue("http://127.0.0.1:3000/api/v1/login"));
    fireEvent.change(requestUrl, { target: { value: "http://127.0.0.1:4310/private" } });
    fireEvent.change(screen.getByLabelText("Request body"), {
      target: { value: '{"stale":true}' },
    });

    view.rerender(
      <ExecutionDebugger
        events={[]}
        observeActive={false}
        workspaceId="trace-demo"
        workspaceGeneration={2}
        target={authTarget}
        onOpenSource={vi.fn()}
      />,
    );
    view.rerender(
      <ExecutionDebugger
        events={[]}
        observeActive={false}
        workspaceId="trace-demo"
        workspaceGeneration={2}
        target={null}
        onOpenSource={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByLabelText("Request URL"))
      .toHaveValue("http://127.0.0.1:3000/"));
    expect(screen.getByLabelText("HTTP method")).toHaveValue("GET");
    expect(screen.getByLabelText("Request body")).toHaveValue("");
    const requestEditor = screen.getByRole("tablist", { name: "Request editor" });
    fireEvent.click(within(requestEditor).getByRole("tab", { name: "Headers" }));
    expect(screen.getByLabelText("Request headers")).toHaveValue("");
  });
});
