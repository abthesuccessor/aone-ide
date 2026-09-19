import { describe, expect, it } from "vitest";
import type { GraphSnapshot } from "../types";
import { boundedEvidenceNodeIds } from "./useAiExplanation";

function graph(): GraphSnapshot {
  return {
    nodes: ["root", "alpha", "beta", "second-hop", "unrelated"].map((id) => ({
      id,
      kind: id === "second-hop" ? "sentence" : "function",
      label: id,
      evidence: "declared" as const,
      metadata: {},
    })),
    edges: [
      { id: "z", source: "root", target: "beta", kind: "calls", evidence: "resolved", metadata: {} },
      { id: "a", source: "alpha", target: "root", kind: "calls", evidence: "resolved", metadata: {} },
      { id: "b", source: "beta", target: "second-hop", kind: "precedes", evidence: "declared", metadata: {} },
    ],
    truncated: false,
  };
}

describe("bounded AI graph evidence", () => {
  it("selects the root then a deterministic two-hop corridor", () => {
    expect(boundedEvidenceNodeIds(graph(), "root")).toEqual([
      "root",
      "alpha",
      "beta",
      "second-hop",
    ]);
  });

  it("honors the evidence cap and rejects an unavailable selection", () => {
    expect(boundedEvidenceNodeIds(graph(), "root", 2)).toEqual(["root", "alpha"]);
    expect(boundedEvidenceNodeIds(graph(), "missing")).toEqual([]);
  });
});
