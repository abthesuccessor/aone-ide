import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { GraphSnapshot } from "../types";
import { EvidencePanel } from "./EvidencePanel";

const graph: GraphSnapshot = {
  truncated: false,
  nodes: [
    {
      id: "service",
      kind: "service",
      label: "OrderService",
      evidence: "resolved",
      language: "TypeScript",
      source: { relativePath: "src/orders.ts", startLine: 10, startColumn: 1, endLine: 18, endColumn: 2 },
      metadata: {
        parser: { z: 2, a: { first: true } },
        route: "/orders",
        method: "POST",
        layer: "Domain",
      },
    },
    { id: "observed-target", kind: "database", label: "Observed target", evidence: "observed", metadata: {} },
    { id: "declared-target", kind: "function", label: "Declared target", evidence: "declared", metadata: {} },
    { id: "resolved-source", kind: "endpoint", label: "Resolved source", evidence: "resolved", metadata: {} },
    { id: "inferred-source", kind: "risk", label: "Inferred source", evidence: "inferred", metadata: {} },
  ],
  edges: [
    { id: "out-declared", source: "service", target: "declared-target", kind: "calls", evidence: "declared", metadata: {} },
    { id: "in-inferred", source: "inferred-source", target: "service", kind: "concerns", evidence: "inferred", metadata: {} },
    { id: "out-observed", source: "service", target: "observed-target", kind: "query", evidence: "observed", metadata: {} },
    { id: "in-resolved", source: "resolved-source", target: "service", kind: "handles", evidence: "resolved", metadata: {} },
  ],
};

describe("EvidencePanel", () => {
  it("separates claim, basis, key facts, raw evidence, and directional relationships", () => {
    render(
      <EvidencePanel
        graph={graph}
        selectedNode={graph.nodes[0] ?? null}
        explanation={null}
        explaining={false}
        explanationError={null}
        onExplain={vi.fn()}
        onSelectNode={vi.fn()}
        onOpenSource={vi.fn()}
      />,
    );

    expect(screen.getByText("Current step")).toBeVisible();
    expect(screen.getByText("Evidence details").closest("details")).not.toHaveAttribute("open");
    fireEvent.click(screen.getByText("Evidence details"));
    expect(screen.getByText("Evidence basis")).toBeVisible();
    expect(screen.getByText("Resolved through static symbols or configuration.")).toBeVisible();
    expect(screen.getByText("Key facts")).toBeVisible();
    expect(screen.getByText("POST")).toBeVisible();
    expect(screen.getByText("/orders")).toBeVisible();

    const rawSummary = screen.getByText("Raw evidence");
    expect(rawSummary.closest("details")).not.toHaveAttribute("open");
    expect(screen.getByText("2 fields")).toBeVisible();
    expect(screen.getByText(/"a": \{"first": true\}/)).toBeInTheDocument();
    expect(screen.queryByText(/\[object Object\]/)).not.toBeInTheDocument();

    const outgoing = screen.getByRole("region", { name: /^Outgoing/ });
    const outgoingRows = within(outgoing).getAllByRole("button");
    expect(outgoingRows[0]).toHaveTextContent("Observed target");
    expect(outgoingRows[1]).toHaveTextContent("Declared target");

    const incoming = screen.getByRole("region", { name: /^Incoming/ });
    const incomingRows = within(incoming).getAllByRole("button");
    expect(incomingRows[0]).toHaveTextContent("Resolved source");
    expect(incomingRows[1]).toHaveTextContent("Inferred source");
  });
});
