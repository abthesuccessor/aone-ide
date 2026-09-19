import { describe, expect, it, vi } from "vitest";
import type { GraphEdge, GraphNode, GraphSnapshot } from "../../types";
import {
  GRAPH_ARTIFACT_SCHEMA_VERSION,
  createGraphArtifactBundle,
  downloadGraphArtifactBundle,
  type GraphArtifactBundle,
  type GraphArtifactDocument,
} from "./artifacts";

const GENERATED_AT = "2026-09-05T04:30:00.000Z";
const COMMUNITY_ID = "community:0123456789abcdef0123456789abcdef";

function communityMetadata(size: number, complete: boolean): GraphNode["metadata"] {
  return {
    communityId: COMMUNITY_ID,
    communitySize: size,
    communityAlgorithm: "deterministicLabelPropagationV1",
    communityBasis: "boundedGraphSnapshot",
    communityComplete: complete,
  };
}

function node(
  id: string,
  metadata: GraphNode["metadata"] = {},
  evidence: GraphNode["evidence"] = "declared",
): GraphNode {
  return {
    id,
    kind: id === "service" ? "service" : "function",
    label: id === "service" ? "Order service" : id,
    evidence,
    metadata,
  };
}

function edge(id: string, source: string, target: string, kind: string): GraphEdge {
  return { id, source, target, kind, evidence: "resolved", metadata: {} };
}

function bundle(graph: GraphSnapshot, workspaceSafeName = "orders-workspace"): GraphArtifactBundle {
  return createGraphArtifactBundle({ workspaceSafeName, generatedAt: GENERATED_AT, graph });
}

function graphDocument(value: GraphArtifactBundle): GraphArtifactDocument {
  return JSON.parse(value.graphJson.content) as GraphArtifactDocument;
}

function embeddedDocument(value: GraphArtifactBundle): GraphArtifactDocument {
  const match = value.graphHtml.content.match(/<script id="graph-data" type="application\/json">([\s\S]*?)<\/script>/);
  if (!match?.[1]) throw new Error("graph-data JSON was not embedded");
  return JSON.parse(match[1]) as GraphArtifactDocument;
}

function runViewer(value: GraphArtifactBundle): Document {
  const parsed = new DOMParser().parseFromString(value.graphHtml.content, "text/html");
  const executable = parsed.querySelectorAll("script")[1]?.textContent;
  if (!executable) throw new Error("standalone viewer script was not embedded");
  Function("document", executable)(parsed);
  return parsed;
}

describe("graph artifact bundle", () => {
  it("is deterministic for equivalent snapshots and caller-provided time", () => {
    const first: GraphSnapshot = {
      nodes: [
        node("service", { z: 2, nested: { second: false, first: true }, a: [2, 1] }, "resolved"),
        node("handler", { role: "entry", ...communityMetadata(1, true) }),
      ],
      edges: [edge("z-edge", "handler", "service", "calls"), edge("a-edge", "service", "handler", "returns")],
      truncated: false,
    };
    const second: GraphSnapshot = {
      nodes: [
        node("handler", { communityComplete: true, role: "entry", communityBasis: "boundedGraphSnapshot", communityAlgorithm: "deterministicLabelPropagationV1", communitySize: 1, communityId: COMMUNITY_ID }),
        node("service", { a: [2, 1], nested: { first: true, second: false }, z: 2 }, "resolved"),
      ],
      edges: [edge("a-edge", "service", "handler", "returns"), edge("z-edge", "handler", "service", "calls")],
      truncated: false,
    };

    expect(bundle(first)).toEqual(bundle(second));
    expect(graphDocument(bundle(first)).graph.nodes.map((item) => item.id)).toEqual(["handler", "service"]);
  });

  it("escapes executable HTML boundaries and JavaScript line separators in embedded JSON", () => {
    const attack = "</script><img src=x onerror=alert(1)>&\u2028\u2029";
    const result = bundle({ nodes: [node("hostile", { attack })], edges: [], truncated: false }, attack);
    const html = result.graphHtml.content;

    expect(html).not.toContain("</script><img");
    expect(html).not.toContain("<img src=x");
    expect(html).not.toContain("\u2028");
    expect(html).not.toContain("\u2029");
    expect(html).toContain("\\u003c/script\\u003e\\u003cimg src=x onerror=alert(1)\\u003e\\u0026");
    expect(embeddedDocument(result).workspace.safeName).toBe(attack);
    expect(embeddedDocument(result).graph.nodes[0]?.metadata.attack).toBe(attack);
  });

  it("preserves and counts every parallel edge independently", () => {
    const result = bundle({
      nodes: [node("handler"), node("service", communityMetadata(1, true))],
      edges: [
        edge("calls-1", "handler", "service", "calls"),
        edge("calls-2", "handler", "service", "calls"),
        edge("reads-1", "handler", "service", "reads"),
      ],
      truncated: false,
    });
    const document = graphDocument(result);

    expect(document.graph.edges.map((item) => item.id)).toEqual(["calls-1", "calls-2", "reads-1"]);
    expect(document.counts.edgeCount).toBe(3);
    expect(document.counts.relationCounts).toEqual([{ name: "calls", count: 2 }, { name: "reads", count: 1 }]);
    expect(document.degreeRankedHubs).toEqual([
      { nodeId: "handler", label: "handler", kind: "function", inDegree: 0, outDegree: 3, totalDegree: 3 },
      { nodeId: "service", label: "Order service", kind: "service", inDegree: 3, outDegree: 0, totalDegree: 3 },
    ]);
    expect(embeddedDocument(result).graph.edges).toHaveLength(3);

    const viewer = runViewer(result);
    const paths = [...viewer.querySelectorAll<SVGPathElement>("path.edge")];
    expect(paths).toHaveLength(3);
    expect(new Set(paths.map((path) => path.getAttribute("d"))).size).toBe(3);
    const search = viewer.querySelector<HTMLInputElement>("#search");
    if (!search) throw new Error("viewer search input is missing");
    search.value = "reads";
    search.dispatchEvent(new Event("input"));
    expect(viewer.querySelectorAll("#edge-list li")).toHaveLength(1);
  });

  it("reports deterministic bounded communities and never falls back to SCC metadata", () => {
    const result = bundle({
      nodes: [
        node("alpha", communityMetadata(2, false)),
        node("beta", communityMetadata(2, false)),
        node("gamma", { stronglyConnectedComponent: 99 }),
      ],
      edges: [edge("a-b", "alpha", "beta", "calls")],
      truncated: true,
    });
    const document = graphDocument(result);

    expect(document.structuralCommunities).toEqual({
      metadataKeys: {
        id: "communityId",
        size: "communitySize",
        algorithm: "communityAlgorithm",
        basis: "communityBasis",
        complete: "communityComplete",
      },
      unassignedNodeCount: 1,
      communities: [{
        communityId: COMMUNITY_ID,
        returnedNodeCount: 2,
        communitySize: 2,
        communityAlgorithm: "deterministicLabelPropagationV1",
        communityBasis: "boundedGraphSnapshot",
        communityComplete: false,
        contractConsistent: true,
        nodeIds: ["alpha", "beta"],
      }],
    });
    expect(document.graph.nodes.find((item) => item.id === "gamma")?.metadata.stronglyConnectedComponent).toBe(99);
    expect(result.graphReport.content).toContain("deterministicLabelPropagationV1");
    expect(result.graphReport.content).toContain("boundedGraphSnapshot");
    expect(result.graphReport.content).toContain("never semantic or topic communities");
    expect(result.graphReport.content).toContain("SCC metadata is not used as a fallback");
  });

  it("makes truncation and continuation facts explicit without overstating completeness", () => {
    const truncated = bundle({
      nodes: [node("handler")],
      edges: [],
      truncated: true,
      nextCursor: "opaque-next-page",
      totalRootLinks: 12,
      omittedRootLinks: 7,
    });
    const completeForQuery = bundle({ nodes: [node("handler")], edges: [], truncated: false });

    expect(graphDocument(truncated).scope).toMatchObject({
      bounded: true,
      truncated: true,
      completeness: "bounded-query-truncated",
      nextCursorAvailable: true,
      totalRootLinks: 12,
      omittedRootLinks: 7,
    });
    expect(truncated.graphReport.content).toContain("- Truncated: yes");
    expect(truncated.graphReport.content).toContain("- Omitted root links: 7");
    expect(graphDocument(completeForQuery).scope.statement).toContain("not a whole-workspace completeness claim");
    expect(completeForQuery.graphReport.content).toContain("truncated=false applies only to the executed bounded query");
  });

  it("includes schema, counts, CSP, and an offline searchable SVG/list viewer", () => {
    const result = bundle({
      nodes: [node("handler", {}, "observed"), node("service", {}, "inferred")],
      edges: [edge("calls", "handler", "service", "calls")],
      truncated: false,
    });
    const document = graphDocument(result);

    expect(document.schemaVersion).toBe(GRAPH_ARTIFACT_SCHEMA_VERSION);
    expect(document.counts.nodeCount).toBe(2);
    expect(document.counts.evidenceCounts.nodes).toEqual({ declared: 0, resolved: 0, observed: 1, inferred: 1 });
    expect(result.graphHtml.content).toContain("Content-Security-Policy");
    expect(result.graphHtml.content).toContain("connect-src 'none'");
    expect(result.graphHtml.content).toContain("<svg id=\"graph\"");
    expect(result.graphHtml.content).toContain("id=\"search\"");
    expect(result.graphHtml.content).not.toMatch(/<(?:script|link|img)[^>]+(?:src|href)=["']https?:/i);
  });

  it("revokes every temporary browser URL after triggering downloads", () => {
    const result = bundle({ nodes: [], edges: [], truncated: false });
    const created: string[] = [];
    const revoked: string[] = [];
    const urlApi = {
      createObjectURL: vi.fn(() => {
        const value = `blob:artifact-${created.length}`;
        created.push(value);
        return value;
      }),
      revokeObjectURL: vi.fn((value: string) => revoked.push(value)),
    };
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);

    downloadGraphArtifactBundle(result, document, urlApi);

    expect(click).toHaveBeenCalledTimes(3);
    expect(revoked).toEqual(created);
    expect(document.querySelectorAll("a[download]")).toHaveLength(0);
    click.mockRestore();
  });
});
