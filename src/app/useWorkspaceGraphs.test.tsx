import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphSnapshot } from "../types";
import { useWorkspaceGraphs } from "./useWorkspaceGraphs";

const bridge = vi.hoisted(() => ({ queryGraph: vi.fn() }));

vi.mock("../lib/bridge", () => ({ queryGraph: bridge.queryGraph }));

function snapshot(id: string): GraphSnapshot {
  return {
    nodes: [{ id, kind: "sentence", label: id, evidence: "declared", metadata: {} }],
    edges: [],
    truncated: false,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

function harness() {
  const generationRef = { current: 1 };
  const options = {
    generationRef,
    setLens: vi.fn(),
    setMainView: vi.fn(),
    setSelectedNodeId: vi.fn(),
    notify: vi.fn(),
  };
  return { generationRef, options, hook: renderHook(() => useWorkspaceGraphs(options)) };
}

beforeEach(() => bridge.queryGraph.mockReset());

describe("workspace relationship search", () => {
  it("queries and selects a bounded two-hop neighborhood", async () => {
    bridge.queryGraph.mockResolvedValue(snapshot("sentence:result"));
    const { hook, options } = harness();

    await act(async () => hook.result.current.search("  deployment evidence  "));

    expect(bridge.queryGraph).toHaveBeenCalledWith({
      projection: "neighborhood",
      query: "deployment evidence",
      depth: 2,
      limit: 500,
    });
    expect(hook.result.current.searchQuery).toBe("deployment evidence");
    expect(hook.result.current.searchResult.nodes[0]?.id).toBe("sentence:result");
    expect(options.setSelectedNodeId).toHaveBeenCalledWith("sentence:result");
  });

  it("ignores a superseded graph query and clears without IPC", async () => {
    const first = deferred<GraphSnapshot>();
    const second = deferred<GraphSnapshot>();
    bridge.queryGraph
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const { hook } = harness();

    let firstRequest!: Promise<void>;
    let secondRequest!: Promise<void>;
    act(() => {
      firstRequest = hook.result.current.search("first");
      secondRequest = hook.result.current.search("second");
    });
    await act(async () => {
      second.resolve(snapshot("second"));
      await secondRequest;
    });
    await act(async () => {
      first.resolve(snapshot("first"));
      await firstRequest;
    });
    expect(hook.result.current.searchResult.nodes[0]?.id).toBe("second");

    await act(async () => hook.result.current.search(""));
    expect(hook.result.current.searchQuery).toBe("");
    expect(hook.result.current.searchResult.nodes).toHaveLength(0);
    expect(bridge.queryGraph).toHaveBeenCalledTimes(2);
  });
});

describe("workspace execution flow", () => {
  it("displays the resolved handler root while paging with the canonical query root", async () => {
    bridge.queryGraph
      .mockResolvedValueOnce({ ...snapshot("endpoint:checkout"), nextCursor: "1.cursor" })
      .mockResolvedValueOnce(snapshot("service:checkout"));
    const { hook, options } = harness();

    await act(async () => hook.result.current.traceNode(
      "api:post:/api/checkout",
      "endpoint:checkout",
      "debugger",
    ));

    expect(hook.result.current.flowRootId).toBe("endpoint:checkout");
    expect(options.setSelectedNodeId).toHaveBeenCalledWith("endpoint:checkout");

    await act(async () => hook.result.current.loadNext());

    expect(bridge.queryGraph.mock.calls[1]?.[0]).toEqual(expect.objectContaining({
      rootIds: ["api:post:/api/checkout"],
      cursor: "1.cursor",
    }));
  });
});
