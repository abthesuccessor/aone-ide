import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ApiRequest, ApiResponse } from "../../types";
import { ApiClientPane } from "./ApiClientPane";
import { useApiClientSession } from "./useApiClientSession";

const response: ApiResponse = {
  requestId: "request-compact",
  status: 200,
  statusText: "OK",
  headers: [],
  body: "{}",
  durationMs: 9,
  truncated: false,
};

function CompactClient({ debugActive = false }: { debugActive?: boolean }) {
  const session = useApiClientSession({
    onSendRequest: vi.fn().mockResolvedValue(response),
    debugActive,
  });
  return (
    <ApiClientPane
      session={session}
      debugActive={debugActive}
      compact
      responseMode="none"
    />
  );
}

function EndpointClient({ onSendRequest }: {
  onSendRequest: (request: ApiRequest) => Promise<ApiResponse>;
}) {
  const session = useApiClientSession({
    onSendRequest,
    target: {
      id: "shipment",
      method: "GET",
      path: "/api/shipments/{shipmentId}",
    },
    initialDraft: { url: "http://127.0.0.1:4310/start" },
  });
  return <ApiClientPane session={session} compact responseMode="none" />;
}

describe("ApiClientPane compact mode", () => {
  it("renders one request-only column with a terse but complete privacy label", () => {
    const { container } = render(<CompactClient debugActive />);

    expect(container.querySelector(".api-console")).toHaveClass("is-compact", "is-request-only");
    expect(screen.getByLabelText("API request")).toBeVisible();
    expect(screen.queryByLabelText("API response")).not.toBeInTheDocument();
    expect(screen.getByText("Local · redacted")).toHaveAttribute(
      "title",
      expect.stringContaining("Debug requests allow up to 60 seconds"),
    );
  });

  it("switches between Body and Headers without losing the edited draft", () => {
    render(<CompactClient />);

    const bodyTab = screen.getByRole("tab", { name: "Body" });
    const headersTab = screen.getByRole("tab", { name: "Headers" });
    expect(bodyTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByLabelText("Request body")).toBeVisible();
    expect(screen.queryByLabelText("Request headers")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Request body"), {
      target: { value: '{"shipmentId":"SHP-1047"}' },
    });
    fireEvent.click(headersTab);
    expect(headersTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByLabelText("Request headers")).toBeVisible();
    expect(screen.queryByLabelText("Request body")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Request headers"), {
      target: { value: "accept: application/json" },
    });
    fireEvent.click(bodyTab);
    expect(screen.getByLabelText("Request body")).toHaveValue('{"shipmentId":"SHP-1047"}');
    fireEvent.click(headersTab);
    expect(screen.getByLabelText("Request headers")).toHaveValue("accept: application/json");
  });

  it("visibly blocks Send until every selected-route placeholder is replaced", async () => {
    const send = vi.fn().mockResolvedValue(response);
    render(<EndpointClient onSendRequest={send} />);

    const url = await screen.findByDisplayValue("http://127.0.0.1:4310/api/shipments/{shipmentId}");
    const button = screen.getByRole("button", { name: "Send" });
    expect(url).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Replace route parameters before sending: {shipmentId}",
    );
    expect(button).toBeDisabled();
    fireEvent.click(button);
    expect(send).not.toHaveBeenCalled();

    fireEvent.change(url, {
      target: { value: "http://127.0.0.1:4310/api/shipments/SHP-1047" },
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(button).toBeEnabled();
    fireEvent.click(button);
    await waitFor(() => expect(send).toHaveBeenCalledWith(expect.objectContaining({
      method: "GET",
      url: "http://127.0.0.1:4310/api/shipments/SHP-1047",
      body: undefined,
    })));
  });
});
