import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiCatalog } from "./ApiCatalog";
import type {
  ApiEndpointInventoryItem,
  ApiEndpointInventoryPage,
  ApiEndpointInventoryRequest,
} from "./model";
import { clearApiCatalogCache } from "./useApiCatalog";

function endpoint(index: number): ApiEndpointInventoryItem {
  const isStatus = index === 250;
  const covered = index < 105 || isStatus;
  return {
    id: isStatus ? "openapi:GET:/api/v1/graph/status" : `openapi:POST:/api/v1/demands/${index}`,
    label: isStatus ? "Graph status" : `Demand operation ${index}`,
    method: isStatus ? "GET" : "POST",
    path: isStatus ? "/api/v1/graph/status" : `/api/v1/demands/${index}`,
    role: "producer",
    protocol: "http",
    sourceFile: "openapi/openapi.json",
    source: {
      relativePath: "openapi/openapi.json",
      startLine: index + 1,
      startColumn: 1,
      endLine: index + 2,
      endColumn: 1,
    },
    language: "json",
    framework: "OpenAPI",
    operationId: isStatus ? "graphStatus" : `demandOperation${index}`,
    evidence: "declared",
    grouping: {
      key: "TYSON API / Demand",
      segments: ["TYSON API", "Demand"],
      basis: "openApiTag",
    },
    clientCoverage: covered ? {
      count: 1,
      firstSource: {
        relativePath: `tyson-frontend/src/api/client-${index}.ts`,
        startLine: 3,
        startColumn: 1,
        endLine: 5,
        endColumn: 1,
      },
    } : { count: 0 },
    occurrenceCount: covered ? 2 : 1,
  };
}

const inventory = Array.from({ length: 305 }, (_, index) => endpoint(index));

function loader(request: ApiEndpointInventoryRequest): Promise<ApiEndpointInventoryPage> {
  const query = request.query?.toLocaleLowerCase();
  const matches = inventory.filter((item) => !query || [
    item.method, item.path, item.label, item.sourceFile,
  ].join(" ").toLocaleLowerCase().includes(query));
  const offset = Number(request.cursor ?? 0);
  const limit = Math.min(request.limit ?? 100, 200);
  const endpoints = matches.slice(offset, offset + limit);
  const next = offset + endpoints.length;
  return Promise.resolve({
    workspaceId: request.workspaceId,
    endpoints,
    total: matches.length,
    indexedTotal: inventory.length,
    returned: endpoints.length,
    nextCursor: next < matches.length ? String(next) : undefined,
    truncated: next < matches.length,
    counts: {
      producer: matches.length,
      consumerOnly: 0,
      clientCovered: matches.filter((item) => item.clientCoverage.count > 0).length,
      unknown: 0,
    },
  });
}

beforeEach(() => clearApiCatalogCache());

describe("wide API catalog", () => {
  it("loads every page, searches beyond page one, restores totals, and opens provenance", async () => {
    const loadPage = vi.fn(loader);
    const onOpenSource = vi.fn();
    const onTraceFlow = vi.fn();
    render(
      <ApiCatalog
        workspaceId="workspace-tyson"
        workspaceGeneration={1}
        loadPage={loadPage}
        onOpenSource={onOpenSource}
        onTraceFlow={onTraceFlow}
      />,
    );

    expect(await screen.findByText("Loaded 305 of 305")).toBeVisible();
    expect(screen.getByRole("button", { name: /Server 305/ })).toBeVisible();
    expect(screen.getByRole("button", { name: /Client coverage 106/ })).toBeVisible();
    expect(loadPage).toHaveBeenCalledWith(expect.objectContaining({ limit: 200 }));
    expect(loadPage).toHaveBeenCalledWith(expect.objectContaining({ cursor: "200" }));
    await waitFor(() => expect(document.querySelectorAll("[data-api-endpoint]")).toHaveLength(80));
    while (screen.queryByRole("button", { name: /Show next \d+ operations/ })) {
      fireEvent.click(screen.getByRole("button", { name: /Show next \d+ operations/ }));
    }
    const allRows = document.querySelectorAll<HTMLElement>("[data-api-endpoint]");
    expect(allRows).toHaveLength(305);
    expect(new Set(Array.from(allRows, (row) => row.dataset.apiEndpoint)).size).toBe(305);
    allRows[0]?.focus();
    fireEvent.keyDown(allRows[0] as HTMLElement, { key: "End" });
    expect(allRows[304]).toHaveFocus();
    fireEvent.click(allRows[0] as HTMLElement);
    expect(document.querySelector(".api-selection-bar")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("HTTP method"), { target: { value: "GET" } });
    await waitFor(() => expect(document.querySelector(".api-selection-bar")).not.toBeInTheDocument());
    fireEvent.change(screen.getByLabelText("HTTP method"), { target: { value: "ALL" } });

    fireEvent.change(screen.getByPlaceholderText("Search method, route, handler, or source"), {
      target: { value: "GET /api/v1/graph/status" },
    });
    expect(screen.queryByText("Loaded 305 of 305")).not.toBeInTheDocument();
    expect(await screen.findByText("Loaded 1 of 1", {}, { timeout: 2_000 })).toBeVisible();
    const status = screen.getByRole("button", {
      name: /GET \/api\/v1\/graph\/status, server endpoint with client coverage/,
    });
    fireEvent.click(status);
    fireEvent.click(screen.getByRole("button", { name: "Trace flow" }));
    expect(onTraceFlow).toHaveBeenCalledWith(expect.objectContaining({ id: "openapi:GET:/api/v1/graph/status" }));
    fireEvent.doubleClick(status);
    expect(onOpenSource).toHaveBeenCalledWith(
      "openapi/openapi.json",
      expect.objectContaining({ startLine: 251 }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Clear API search" }));
    await waitFor(() => expect(screen.getByText("Loaded 305 of 305")).toBeVisible());
  });

  it("uses sourceFile when the indexed operation has no range", async () => {
    const withoutRange = { ...endpoint(0), source: undefined, handler: undefined };
    const loadPage = vi.fn(async (request: ApiEndpointInventoryRequest) => ({
      workspaceId: request.workspaceId,
      endpoints: [withoutRange],
      total: 1,
      indexedTotal: 1,
      returned: 1,
      truncated: false,
      counts: { producer: 1, consumerOnly: 0, clientCovered: 1, unknown: 0 },
    }));
    const onOpenSource = vi.fn();
    render(<ApiCatalog workspaceId="source-fallback" workspaceGeneration={1} loadPage={loadPage} onOpenSource={onOpenSource} onTraceFlow={vi.fn()} />);
    const row = await screen.findByRole("button", { name: /POST \/api\/v1\/demands\/0, server endpoint with client coverage/ });
    fireEvent.doubleClick(row);
    expect(onOpenSource).toHaveBeenCalledWith("openapi/openapi.json", undefined);
  });

  it("keeps broad search results paged in the DOM while preserving reachability", async () => {
    const loadPage = vi.fn(loader);
    render(<ApiCatalog workspaceId="broad-search" workspaceGeneration={1} loadPage={loadPage} onOpenSource={vi.fn()} onTraceFlow={vi.fn()} />);
    expect(await screen.findByText("Loaded 305 of 305")).toBeVisible();
    fireEvent.change(screen.getByPlaceholderText("Search method, route, handler, or source"), {
      target: { value: "/api/v1" },
    });
    const catalog = screen.getByRole("region", { name: "API catalog" });
    await waitFor(() => expect(catalog).toHaveAttribute("aria-busy", "false"));
    expect(document.querySelectorAll("[data-api-endpoint]")).toHaveLength(80);
    while (screen.queryByRole("button", { name: /Show next \d+ operations/ })) {
      fireEvent.click(screen.getByRole("button", { name: /Show next \d+ operations/ }));
    }
    expect(document.querySelectorAll("[data-api-endpoint]")).toHaveLength(305);
  });

  it("keeps legacy facts unclassified until a rescan provides direction", async () => {
    const legacy = { ...endpoint(0), role: "unknown" as const, clientCoverage: { count: 0 } };
    const loadPage = vi.fn(async (request: ApiEndpointInventoryRequest) => ({
      workspaceId: request.workspaceId,
      endpoints: [legacy],
      total: 1,
      indexedTotal: 1,
      returned: 1,
      truncated: false,
      counts: { producer: 0, consumerOnly: 0, clientCovered: 0, unknown: 1 },
    }));
    render(<ApiCatalog workspaceId="legacy" workspaceGeneration={1} loadPage={loadPage} onOpenSource={vi.fn()} onTraceFlow={vi.fn()} />);

    expect(await screen.findByRole("button", { name: "Unclassified 1" })).toBeVisible();
    expect(screen.getByText("1 unclassified. Scan workspace to classify legacy API facts.")).toBeVisible();
    expect(screen.queryByText("Server declared")).not.toBeInTheDocument();
  });
});
