import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { OtlpReceiverSnapshot } from "../../types";
import { OtlpReceiverBar } from "./OtlpReceiverBar";

const running: OtlpReceiverSnapshot = {
  status: "running",
  endpoint: "http://127.0.0.1:4318/v1/traces",
  protocol: "http/json",
  authHeaderName: "x-aone-ingest-token",
  authHeaderValue: "ephemeral-token",
  acceptedSpans: 6,
  rejectedSpans: 1,
  traceCount: 1,
  startedAt: "2026-08-20T00:00:00Z",
  limitation: "Bounded local trace receiver.",
};

describe("OtlpReceiverBar", () => {
  it("shows exact exporter configuration and requires confirmation before trace deletion", () => {
    const onDeleteTrace = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(
      <OtlpReceiverBar
        snapshot={running}
        available
        loading={false}
        selectedTraceId="5b8efff798038103d269b633813fc60c"
        onStart={vi.fn()}
        onStop={vi.fn()}
        onDeleteTrace={onDeleteTrace}
      />,
    );

    expect(screen.getByText(/1 trace, 6 accepted, 1 rejected/)).toBeVisible();
    expect(screen.getByText(/OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http:\/\/127\.0\.0\.1:4318\/v1\/traces/)).toBeInTheDocument();
    expect(screen.getByText(/OTEL_EXPORTER_OTLP_TRACES_HEADERS=x-aone-ingest-token=ephemeral-token/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Delete trace" }));
    expect(window.confirm).toHaveBeenCalled();
    expect(onDeleteTrace).toHaveBeenCalledWith("5b8efff798038103d269b633813fc60c");
  });

  it("does not claim the browser demo can open a listener", () => {
    render(
      <OtlpReceiverBar
        snapshot={null}
        available={false}
        loading={false}
        onStart={vi.fn()}
        onStop={vi.fn()}
        onDeleteTrace={vi.fn()}
      />,
    );
    expect(screen.getByText("Desktop app only")).toBeVisible();
    expect(screen.getByRole("button", { name: "Start receiver" })).toBeDisabled();
  });

  it("keeps trace observation disabled until Debug is active", () => {
    render(
      <OtlpReceiverBar
        snapshot={null}
        available
        startEnabled={false}
        loading={false}
        onStart={vi.fn()}
        onStop={vi.fn()}
        onDeleteTrace={vi.fn()}
      />,
    );
    expect(screen.getByText("Start Debug first")).toBeVisible();
    expect(screen.getByRole("button", { name: "Start receiver" })).toBeDisabled();
  });
});
