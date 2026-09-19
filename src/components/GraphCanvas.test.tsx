import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { GraphNode, GraphSnapshot } from "../types";
import { GraphCanvas } from "./GraphCanvas";

function at(relativePath: string, startLine: number) {
  return { relativePath, startLine, startColumn: 1, endLine: startLine + 1, endColumn: 1 };
}

const nodes: GraphNode[] = [
  { id: "ui", kind: "button", label: "Submit order", evidence: "declared", source: at("web/Checkout.tsx", 44), metadata: { flowStage: "frontend" } },
  { id: "api", kind: "endpoint", label: "POST /orders", evidence: "declared", source: at("api/routes.ts", 17), metadata: { flowStage: "api" } },
  { id: "handler", kind: "handler", label: "create_order", evidence: "resolved", source: at("backend/orders.rs", 28), metadata: { flowStage: "backend" } },
  { id: "service", kind: "trait", label: "OrderService.create", evidence: "resolved", source: at("backend/order_service.rs", 51), metadata: { flowStage: "service" } },
  { id: "db", kind: "repository", label: "OrderRepository.insert", evidence: "declared", source: at("data/orders.rs", 12), metadata: { flowStage: "data" } },
  { id: "cloud", kind: "external-api", label: "Billing API", evidence: "inferred", source: at("backend/billing.rs", 8), metadata: { flowStage: "external" } },
];

const graph: GraphSnapshot = {
  truncated: false,
  nodes,
  edges: [
    { id: "e1", source: "ui", target: "api", kind: "requests", evidence: "resolved", metadata: {} },
    { id: "e2", source: "api", target: "handler", kind: "handledBy", evidence: "resolved", metadata: {} },
    { id: "e3", source: "handler", target: "service", kind: "calls", evidence: "resolved", metadata: {} },
    { id: "e4", source: "service", target: "db", kind: "writes", evidence: "declared", metadata: {} },
    { id: "e5", source: "service", target: "cloud", kind: "http", evidence: "inferred", metadata: {} },
  ],
};

const baseProps = {
  lens: "all" as const,
  selectedNodeId: null,
  onSelectNode: vi.fn(),
  onOpenSource: vi.fn(),
};

describe("GraphCanvas force relationships", () => {
  it("renders every loaded node and edge as one D3 force surface", () => {
    render(<GraphCanvas graph={graph} {...baseProps} />);
    expect(screen.getByRole("group", { name: "Workspace relationships with 6 nodes and 5 edges" })).toBeVisible();
    expect(document.querySelectorAll(".force-node")).toHaveLength(6);
    expect(document.querySelectorAll(".force-link")).toHaveLength(5);
    expect(document.querySelectorAll(".flow-node")).toHaveLength(0);
    expect(screen.queryByText(/code map/i)).not.toBeInTheDocument();
  });

  it("draws parallel and reverse relationships on distinct paths", async () => {
    render(<GraphCanvas graph={{
      truncated: false,
      nodes: nodes.slice(0, 2),
      edges: [
        { id: "declared", source: "ui", target: "api", kind: "requests", evidence: "declared", metadata: {} },
        { id: "resolved", source: "ui", target: "api", kind: "calls", evidence: "resolved", metadata: {} },
        { id: "reverse", source: "api", target: "ui", kind: "responds", evidence: "resolved", metadata: {} },
      ],
    }} {...baseProps} />);
    await waitFor(() => {
      const paths = [...document.querySelectorAll<SVGPathElement>(".force-link")];
      expect(new Set(paths.map((path) => path.getAttribute("d"))).size).toBe(3);
    });
  });

  it("uses stable semantic colors for entry, API, handler, logic, data, and external nodes", () => {
    render(<GraphCanvas graph={graph} {...baseProps} />);
    expect(document.querySelector('[data-node-id="ui"]')).toHaveClass("is-stage-trigger");
    expect(document.querySelector('[data-node-id="api"]')).toHaveClass("is-stage-api");
    expect(document.querySelector('[data-node-id="handler"]')).toHaveClass("is-stage-backend");
    expect(document.querySelector('[data-node-id="service"]')).toHaveClass("is-stage-service");
    expect(document.querySelector('[data-node-id="db"]')).toHaveClass("is-stage-data");
    expect(document.querySelector('[data-node-id="cloud"]')).toHaveClass("is-stage-external");
  });

  it("highlights only the selected node corridor and dims unrelated relationships", () => {
    render(<GraphCanvas graph={graph} {...baseProps} selectedNodeId="service" />);
    expect(document.querySelector('[data-node-id="service"]')).toHaveClass("is-selected");
    expect(document.querySelector('[data-node-id="handler"]')).not.toHaveClass("is-dimmed");
    expect(document.querySelector('[data-node-id="db"]')).not.toHaveClass("is-dimmed");
    expect(document.querySelector('[data-node-id="cloud"]')).not.toHaveClass("is-dimmed");
    expect(document.querySelector('[data-node-id="ui"]')).toHaveClass("is-dimmed");
    expect(document.querySelector('[data-edge-id="e3"]')).toHaveClass("is-highlighted");
    expect(document.querySelector('[data-edge-id="e1"]')).toHaveClass("is-dimmed");
  });

  it("selects a bubble and opens only its backend-provided source range", () => {
    const onSelectNode = vi.fn();
    const onOpenSource = vi.fn();
    render(<GraphCanvas graph={graph} {...baseProps} onSelectNode={onSelectNode} onOpenSource={onOpenSource} />);
    const api = screen.getByRole("button", { name: /POST \/orders, endpoint, declared evidence, api\/routes\.ts:17/ });
    fireEvent.click(api);
    expect(onSelectNode).toHaveBeenCalledWith("api");
    fireEvent.doubleClick(api);
    expect(onOpenSource).toHaveBeenCalledWith(nodes[1], "pinned");
  });

  it("supports keyboard selection and exact source opening", () => {
    const onSelectNode = vi.fn();
    const onOpenSource = vi.fn();
    render(<GraphCanvas graph={graph} {...baseProps} onSelectNode={onSelectNode} onOpenSource={onOpenSource} />);
    const service = screen.getByRole("button", { name: /OrderService\.create/ });
    fireEvent.keyDown(service, { key: " " });
    expect(onSelectNode).toHaveBeenCalledWith("service");
    fireEvent.keyDown(service, { key: "Enter", metaKey: true });
    expect(onOpenSource).toHaveBeenCalledWith(nodes[3], "pinned");
  });

  it("distinguishes observed runtime nodes and edges without changing static evidence", () => {
    const runtime: GraphNode = {
      id: "runtime", kind: "runtime-event", label: "POST /orders observed", evidence: "observed",
      metadata: { flowStage: "api", eventKind: "http.completed" },
    };
    render(<GraphCanvas graph={{
      truncated: false,
      nodes: [...nodes, runtime],
      edges: [...graph.edges, { id: "runtime-edge", source: "runtime", target: "api", kind: "correlatesTo", evidence: "observed", metadata: {} }],
    }} {...baseProps} />);
    expect(document.querySelector('[data-node-id="runtime"]')).toHaveClass("is-observed");
    expect(document.querySelector('[data-node-id="runtime"] .force-node-halo')).toBeInTheDocument();
    expect(document.querySelector('[data-node-id="api"]')).not.toHaveClass("is-observed");
    expect(document.querySelector('[data-edge-id="runtime-edge"]')).toHaveClass("is-observed");
  });

  it("adds watcher-refreshed nodes without replacing the force graph shell", () => {
    const rendered = render(<GraphCanvas graph={graph} {...baseProps} />);
    const shell = screen.getByRole("group", { name: /Workspace relationships/ });
    const added: GraphNode = { id: "new-worker", kind: "worker", label: "InvoiceWorker", evidence: "resolved", metadata: {} };
    rendered.rerender(<GraphCanvas graph={{
      ...graph,
      nodes: [...graph.nodes, added],
      edges: [...graph.edges, { id: "new-edge", source: "service", target: added.id, kind: "enqueues", evidence: "resolved", metadata: {} }],
    }} {...baseProps} />);
    expect(screen.getByRole("group", { name: "Workspace relationships with 7 nodes and 6 edges" })).toBe(shell);
    expect(document.querySelector('[data-node-id="new-worker"]')).toHaveClass("is-new");
    expect(document.querySelectorAll(".force-node")).toHaveLength(7);
  });

  it("keeps every node while bounding persistent labels for large workspaces", () => {
    const largeNodes = Array.from({ length: 40 }, (_, index): GraphNode => ({
      id: `node-${index}`,
      kind: "function",
      label: `Function ${index}`,
      evidence: "resolved",
      metadata: {},
    }));
    const largeEdges = largeNodes.slice(1).map((node, index) => ({
      id: `edge-${index}`,
      source: largeNodes[index]!.id,
      target: node.id,
      kind: "calls",
      evidence: "resolved" as const,
      metadata: {},
    }));
    render(<GraphCanvas graph={{ nodes: largeNodes, edges: largeEdges, truncated: false }} {...baseProps} />);
    expect(document.querySelectorAll(".force-node")).toHaveLength(40);
    expect(document.querySelectorAll(".force-node-label")).toHaveLength(28);
  });

  it("keeps the current graph visible while indexed relationships refresh", () => {
    render(<GraphCanvas graph={graph} {...baseProps} loading />);
    expect(document.querySelectorAll(".force-node")).toHaveLength(6);
    expect(screen.getByRole("status")).toHaveTextContent("Refreshing indexed relationships");
  });

  it("loads a selected sequence and bounded continuation from compact controls", () => {
    const onTraceNode = vi.fn();
    const onLoadMore = vi.fn();
    render(<GraphCanvas graph={{ ...graph, nextCursor: "next" }} {...baseProps} selectedNodeId="api"
      onTraceNode={onTraceNode} onLoadMore={onLoadMore} />);
    fireEvent.click(screen.getByRole("button", { name: "Load selected sequence" }));
    fireEvent.click(screen.getByRole("button", { name: "Load more relationships" }));
    expect(onTraceNode).toHaveBeenCalledWith("api");
    expect(onLoadMore).toHaveBeenCalledTimes(1);
  });

  it("renders honest loading, empty, and error states when no indexed graph is available", () => {
    const empty: GraphSnapshot = { nodes: [], edges: [], truncated: false };
    const rendered = render(<GraphCanvas graph={empty} {...baseProps} loading />);
    expect(screen.getByText("Building the workspace graph")).toBeVisible();
    rendered.rerender(<GraphCanvas graph={empty} {...baseProps} />);
    expect(screen.getByText("No relationships in this lens")).toBeVisible();
    rendered.rerender(<GraphCanvas graph={empty} {...baseProps} error="Index query failed" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Index query failed");
  });
});
