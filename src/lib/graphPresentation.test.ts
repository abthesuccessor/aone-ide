import { describe, expect, it } from "vitest";
import type { GraphEdge, GraphNode } from "../types";
import {
  formatMetadataValue,
  graphLaneForNode,
  groupNodeRelations,
  presentMetadata,
  truncateGraphLabel,
} from "./graphPresentation";

function node(id: string, kind: string, label = id, metadata: GraphNode["metadata"] = {}): GraphNode {
  return { id, kind, label, evidence: "declared", metadata };
}

describe("graph presentation", () => {
  it("derives stable lanes from real analyzer kinds when layer metadata is absent", () => {
    expect(graphLaneForNode(node("file", "file")).id).toBe("source");
    expect(graphLaneForNode(node("endpoint", "endpoint")).id).toBe("interface");
    expect(graphLaneForNode(node("event", "event")).id).toBe("interface");
    expect(graphLaneForNode(node("service", "service")).id).toBe("domain");
    expect(graphLaneForNode(node("repo", "repository")).id).toBe("data");
    expect(graphLaneForNode(node("external", "external-api")).id).toBe("external");
    expect(graphLaneForNode(node("runtime", "runtime-event")).id).toBe("runtime");
  });

  it("honors a recognized explicit layer before the kind fallback", () => {
    expect(graphLaneForNode(node("endpoint", "endpoint", "endpoint", { layer: "Data" })).id).toBe("data");
    expect(graphLaneForNode(node("config", "file", "package.json", { systemLayer: "configuration" })).id).toBe("source");
    expect(graphLaneForNode(node("main", "function", "main", { systemLayer: "entry" })).id).toBe("source");
    expect(graphLaneForNode(node("service", "file", "service", { systemLayer: "application" })).id).toBe("domain");
  });

  it("normalizes whitespace and truncates labels without splitting Unicode characters", () => {
    expect(truncateGraphLabel("  alpha\n  beta ")).toBe("alpha beta");
    expect(truncateGraphLabel("abcdef😀ghij", 8)).toBe("abcdef😀…");
    expect(truncateGraphLabel("", 8)).toBe("Untitled");
  });

  it("formats nested metadata deterministically without object coercion", () => {
    const formatted = formatMetadataValue({ z: 2, a: [{ second: false, first: true }] });
    expect(formatted).toBe('{"a": [{"first": true, "second": false}], "z": 2}');
    expect(formatted).not.toContain("[object Object]");

    const presentation = presentMetadata({ parser: { z: 2, a: 1 }, route: "/orders", method: "GET", layer: "API" });
    expect(presentation.keyFacts.map((entry) => entry.key)).toEqual(["method", "route"]);
    expect(presentation.rawEvidence.map((entry) => entry.key)).toEqual(["layer", "parser"]);
  });

  it("groups and sorts directional relationships by evidence, kind, label, then id", () => {
    const nodes = new Map([
      ["current", node("current", "service")],
      ["alpha", node("alpha", "function", "Alpha")],
      ["beta", node("beta", "function", "Beta")],
      ["gamma", node("gamma", "function", "Gamma")],
    ]);
    const edges: GraphEdge[] = [
      { id: "out-declared", source: "current", target: "alpha", kind: "calls", evidence: "declared", metadata: {} },
      { id: "in-inferred", source: "gamma", target: "current", kind: "calls", evidence: "inferred", metadata: {} },
      { id: "out-observed", source: "current", target: "beta", kind: "entered", evidence: "observed", metadata: {} },
      { id: "in-resolved", source: "alpha", target: "current", kind: "imports", evidence: "resolved", metadata: {} },
    ];

    const grouped = groupNodeRelations(edges, "current", nodes);
    expect(grouped.outgoing.map((edge) => edge.id)).toEqual(["out-observed", "out-declared"]);
    expect(grouped.incoming.map((edge) => edge.id)).toEqual(["in-resolved", "in-inferred"]);
  });
});
