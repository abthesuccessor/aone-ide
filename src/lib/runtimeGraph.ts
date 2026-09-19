import type { GraphEdge, GraphNode, GraphSnapshot, RuntimeEvent } from "../types";

const MAX_RUNTIME_GRAPH_EVENTS = 80;

interface ProjectionCache {
  events: RuntimeEvent[];
  projection: GraphSnapshot;
}

const projectionCache = new WeakMap<GraphSnapshot, ProjectionCache>();

function isGraphRelevant(event: RuntimeEvent): boolean {
  if (event.kind.startsWith("trace.")) {
    const protocol = ["AONE_TRACE_V1", "OTLP_HTTP_JSON", "W3C_TRACE_CONTEXT"]
      .includes(event.metadata.traceProtocol as string);
    const spanId = event.metadata.spanId;
    const acceptedKind = /^trace\.(http\.server|http\.client|function|database|external|agent|event|job)\.(start|end|event)$/.test(event.kind);
    return protocol && typeof spanId === "string" && spanId.length > 0 && acceptedKind;
  }
  const ordinaryOutput = event.kind === "process.stdout" || event.kind === "process.stderr";
  return !ordinaryOutput || Boolean(event.sourceNodeId);
}

function sameEventInputs(previous: RuntimeEvent[], current: RuntimeEvent[]): boolean {
  return (
    previous.length === current.length &&
    previous.every((event, index) => event === current[index])
  );
}

function metadataString(event: RuntimeEvent, key: string): string | undefined {
  const value = event.metadata[key];
  return typeof value === "string" || typeof value === "number" ? String(value) : undefined;
}

function runtimeFlowStage(event: RuntimeEvent): "trigger" | "api" | "backend" | "service" | "data" | "external" | "response" {
  const reported = metadataString(event, "flowStage");
  if (reported && ["trigger", "api", "backend", "service", "data", "external", "response"].includes(reported)) {
    return reported as "trigger" | "api" | "backend" | "service" | "data" | "external" | "response";
  }
  const kind = event.kind.toLowerCase();
  if (kind.startsWith("trace.database.")) return "data";
  if (kind.startsWith("trace.external.")) return "external";
  if (
    kind.startsWith("trace.http.server.")
    || kind.startsWith("trace.http.client.")
    || kind.startsWith("trace.event.")
    || kind.startsWith("http.")
    || kind.startsWith("websocket.")
  ) return "api";
  if (
    kind.startsWith("trace.agent.")
    || kind.startsWith("trace.job.")
    || kind.startsWith("trace.function.")
  ) return "service";
  return "trigger";
}

function spanKey(event: RuntimeEvent, spanId: string | undefined): string | undefined {
  if (!event.traceId || !spanId) return undefined;
  return `${event.traceId}\u0000${spanId}`;
}

interface VisualSpan {
  event: RuntimeEvent;
  unambiguous: boolean;
}

function canonicalRuntimeEvents(events: RuntimeEvent[]): {
  events: RuntimeEvent[];
  spans: Map<string, VisualSpan>;
} {
  const grouped = new Map<string, RuntimeEvent[]>();
  const ordinary: RuntimeEvent[] = [];
  for (const event of events) {
    const key = spanKey(event, metadataString(event, "spanId"));
    if (key) grouped.set(key, [...(grouped.get(key) ?? []), event]);
    else ordinary.push(event);
  }
  const spans = new Map<string, VisualSpan>();
  for (const [key, spanEvents] of grouped) {
    const ordered = [...spanEvents].sort((left, right) => left.timestamp.localeCompare(right.timestamp) || left.id.localeCompare(right.id));
    const phases = new Map<string, number>();
    for (const event of ordered) {
      const phase = metadataString(event, "phase") ?? "single";
      phases.set(phase, (phases.get(phase) ?? 0) + 1);
    }
    const anchor = ordered.find((event) => metadataString(event, "phase") === "start") ?? ordered[0]!;
    const metadata = Object.assign({}, ...ordered.map((event) => event.metadata));
    spans.set(key, {
      event: { ...anchor, metadata },
      unambiguous: [...phases.values()].every((count) => count === 1),
    });
  }
  return { events: [...ordinary, ...[...spans.values()].map((span) => span.event)], spans };
}

function requestPath(value: string | undefined): string | undefined {
  if (!value) return undefined;
  try {
    return new URL(value).pathname.replace(/\/$/, "") || "/";
  } catch {
    const path = value.split(/[?#]/, 1)[0];
    return path ? path.replace(/\/$/, "") || "/" : undefined;
  }
}

function nodeRoute(node: GraphNode): string | undefined {
  const route = node.metadata.routePath ?? node.metadata.path ?? node.metadata.route;
  if (typeof route === "string") return requestPath(route);
  const match = node.label.match(/(?:GET|POST|PUT|PATCH|DELETE|OPTIONS|HEAD|CONNECT|TRACE)\s+(\/\S*)/i);
  return requestPath(match?.[1]);
}

function nodeMethod(node: GraphNode): string | undefined {
  const method = node.metadata.httpMethod ?? node.metadata.method;
  if (typeof method === "string") return method.toUpperCase();
  return node.label.match(/^(GET|POST|PUT|PATCH|DELETE|OPTIONS|HEAD|CONNECT|TRACE)\b/i)?.[1]?.toUpperCase();
}

function routeMatches(template: string, actual: string): boolean {
  if (template === actual) return true;
  const templateParts = template.split("/").filter(Boolean);
  const actualParts = actual.split("/").filter(Boolean);
  if (templateParts.length !== actualParts.length) return false;
  return templateParts.every((part, index) => (
    part === actualParts[index]
    || /^\{[^/{}]+\}$/.test(part)
    || /^:[^/]+$/.test(part)
    || part === "*"
  ));
}

interface RuntimeTarget {
  node: GraphNode;
  kind: "observedAt" | "correlatesTo";
  evidence: "observed" | "inferred";
  confidence: number;
  basis: "trustedSourceNode" | "processReportedSourceRange" | "uniqueMethodPath";
}

function matchingRuntimeTarget(event: RuntimeEvent, nodes: GraphNode[]): RuntimeTarget | undefined {
  if (event.sourceNodeId) {
    const node = nodes.find((candidate) => candidate.id === event.sourceNodeId);
    return node ? { node, kind: "observedAt", evidence: "observed", confidence: 1, basis: "trustedSourceNode" } : undefined;
  }
  const mappedNodeId = metadataString(event, "mappedNodeId");
  if (mappedNodeId) {
    const node = nodes.find((candidate) => candidate.id === mappedNodeId);
    return node ? {
      node,
      kind: "correlatesTo",
      evidence: "inferred",
      confidence: 0.8,
      basis: "processReportedSourceRange",
    } : undefined;
  }
  const path = requestPath(metadataString(event, "path") ?? metadataString(event, "url"));
  if (!path) return undefined;
  const method = metadataString(event, "method")?.toUpperCase();
  const candidates = nodes.filter((node) => {
    if (!new Set(["endpoint", "api", "external-api"]).has(node.kind)) return false;
    const route = nodeRoute(node);
    if (!route || !routeMatches(route, path)) return false;
    const candidateMethod = nodeMethod(node);
    return !method || !candidateMethod || candidateMethod === method;
  });
  return candidates.length === 1 ? {
    node: candidates[0]!,
    kind: "correlatesTo",
    evidence: "inferred",
    confidence: 0.8,
    basis: "uniqueMethodPath",
  } : undefined;
}

/**
 * Builds a renderer-only runtime projection. Events remain observed facts;
 * URL-to-endpoint matches are separate inferred edges unless the backend
 * supplied an explicit sourceNodeId.
 */
export function withRuntimeProjection(
  graph: GraphSnapshot,
  runtimeEvents: RuntimeEvent[],
): GraphSnapshot {
  const graphEvents = runtimeEvents.filter(isGraphRelevant).slice(-MAX_RUNTIME_GRAPH_EVENTS);
  if (graphEvents.length === 0) return graph;

  const cached = projectionCache.get(graph);
  if (cached && sameEventInputs(cached.events, graphEvents)) return cached.projection;
  const visual = canonicalRuntimeEvents(graphEvents);

  const nodes = [...graph.nodes];
  const edges = [...graph.edges];
  const existingNodeIds = new Set(nodes.map((node) => node.id));
  const existingEdgeIds = new Set(edges.map((edge) => edge.id));

  for (const event of visual.events) {
    const id = `runtime:${event.id}`;
    if (!existingNodeIds.has(id)) {
      const observedText = metadataString(event, "text");
      const node: GraphNode = {
        id,
        kind: "runtime-event",
        label: (observedText ?? event.label).slice(0, 240),
        evidence: "observed",
        metadata: {
          ...event.metadata,
          eventId: event.id,
          eventKind: event.kind,
          flowStage: runtimeFlowStage(event),
          timestamp: event.timestamp,
          ...(event.traceId ? { traceId: event.traceId } : {}),
          ...(event.sourceNodeId ? { sourceNodeId: event.sourceNodeId } : {}),
        },
      };
      nodes.push(node);
      existingNodeIds.add(id);
    }

    const target = matchingRuntimeTarget(event, graph.nodes);
    if (!target) continue;
    const edgeId = `runtime-edge:${event.id}:${target.node.id}`;
    if (existingEdgeIds.has(edgeId)) continue;
    const edge: GraphEdge = {
      id: edgeId,
      source: id,
      target: target.node.id,
      kind: target.kind,
      evidence: target.evidence,
      confidence: target.confidence,
      metadata: { eventId: event.id, correlationBasis: target.basis },
    };
    edges.push(edge);
    existingEdgeIds.add(edgeId);
  }

  for (const child of visual.spans.values()) {
    const event = child.event;
    if (!child.unambiguous) continue;
    const parentKey = spanKey(event, metadataString(event, "parentSpanId"));
    if (!parentKey) continue;
    const parentSpan = visual.spans.get(parentKey);
    if (!parentSpan?.unambiguous) continue;
    const parent = parentSpan.event;
    const edgeId = `runtime-span-edge:${parent.id}:${event.id}`;
    if (existingEdgeIds.has(edgeId)) continue;
    edges.push({
      id: edgeId,
      source: `runtime:${parent.id}`,
      target: `runtime:${event.id}`,
      kind: "parentSpan",
      evidence: "observed",
      confidence: 1,
      metadata: {
        parentEventId: parent.id,
        childEventId: event.id,
        observationBasis: "processReportedTrace",
      },
    });
    existingEdgeIds.add(edgeId);
  }

  const projection = { ...graph, nodes, edges };
  projectionCache.set(graph, { events: graphEvents, projection });
  return projection;
}
