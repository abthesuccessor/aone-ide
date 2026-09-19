import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphSnapshot } from "../types";
import { queryGraph } from "../lib/bridge";
import { loadGraphSnapshots } from "./graphQueries";
import { EMPTY_GRAPH } from "./model";

vi.mock("../lib/bridge", () => ({ queryGraph: vi.fn() }));

const systemGraph: GraphSnapshot = {
  nodes: [{
    id: "entry:main",
    kind: "entrypoint",
    label: "main",
    evidence: "declared",
    metadata: { flowRoot: true },
  }],
  edges: [],
  truncated: false,
};

describe("loadGraphSnapshots", () => {
  beforeEach(() => vi.mocked(queryGraph).mockReset());

  it("keeps the indexed overview when optional detail projections fail", async () => {
    vi.mocked(queryGraph)
      .mockResolvedValueOnce(systemGraph)
      .mockRejectedValueOnce(new Error("optional projection unavailable"))
      .mockResolvedValueOnce(EMPTY_GRAPH);

    const [system, detail, flow] = await loadGraphSnapshots();
    expect(system).toEqual(systemGraph);
    expect(detail).toEqual(EMPTY_GRAPH);
    expect(flow.nodes).toEqual([]);
    expect(flow.edges).toEqual([]);
  });

  it("keeps the indexed overview and detail when the default sequence is unavailable", async () => {
    vi.mocked(queryGraph)
      .mockResolvedValueOnce(systemGraph)
      .mockResolvedValueOnce(systemGraph)
      .mockRejectedValueOnce(new Error("sequence unavailable"));

    await expect(loadGraphSnapshots()).resolves.toEqual([systemGraph, systemGraph, EMPTY_GRAPH]);
  });

  it("does not request optional projections for an empty overview", async () => {
    vi.mocked(queryGraph).mockResolvedValueOnce(EMPTY_GRAPH);

    await expect(loadGraphSnapshots()).resolves.toEqual([EMPTY_GRAPH, EMPTY_GRAPH, EMPTY_GRAPH]);
    expect(queryGraph).toHaveBeenCalledTimes(1);
  });
});
