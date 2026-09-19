import {
  ArrowsOut,
  Crosshair,
  Minus,
  Plus,
} from "@phosphor-icons/react";
import { drag } from "d3-drag";
import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { select } from "d3-selection";
import { zoom, zoomIdentity, type ZoomBehavior } from "d3-zoom";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent,
} from "react";
import type { GraphEdge, GraphNode, GraphSnapshot } from "../../types";
import type { EditorOpenMode } from "../editor/model";
import { flowStageForNode, type FlowStageId } from "./flowModel";

interface ForceRelationshipGraphProps {
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode, mode?: EditorOpenMode) => void;
  onTraceNode?: (nodeId: string) => void;
  onLoadMore?: () => void;
  loadingMore?: boolean;
  applicationZoom?: number;
}

interface ForceNode extends SimulationNodeDatum {
  id: string;
  node: GraphNode;
  degree: number;
  radius: number;
  stage: FlowStageId;
  isNew: boolean;
}

interface ForceLink extends SimulationLinkDatum<ForceNode> {
  id: string;
  edge: GraphEdge;
  parallelOffset: number;
}

interface TooltipState {
  node: GraphNode;
  x: number;
  y: number;
}

const FALLBACK_WIDTH = 920;
const FALLBACK_HEIGHT = 620;
const MAX_VISIBLE_LABELS = 28;

function hash(value: string): number {
  let result = 2166136261;
  for (let index = 0; index < value.length; index += 1) {
    result ^= value.charCodeAt(index);
    result = Math.imul(result, 16777619);
  }
  return result >>> 0;
}

function seededRandom(seed: number): () => number {
  let state = seed || 1;
  return () => {
    state = Math.imul(1664525, state) + 1013904223;
    return (state >>> 0) / 4294967296;
  };
}

function shortLabel(label: string): string {
  return label.length <= 26 ? label : `${label.slice(0, 23)}...`;
}

function sourceLabel(node: GraphNode): string {
  if (!node.source) return "No exact source range";
  return `${node.source.relativePath}:${node.source.startLine}`;
}

function nodeAriaLabel(node: GraphNode): string {
  return `${node.label}, ${node.kind}, ${node.evidence} evidence, ${sourceLabel(node)}`;
}

function linkNodeId(endpoint: string | number | ForceNode): string {
  return typeof endpoint === "object" ? endpoint.id : String(endpoint);
}

function stageClass(stage: FlowStageId): string {
  return `is-stage-${stage}`;
}

export function ForceRelationshipGraph({
  graph,
  selectedNodeId,
  onSelectNode,
  onOpenSource,
  onTraceNode,
  onLoadMore,
  loadingMore = false,
  applicationZoom = 1,
}: ForceRelationshipGraphProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const viewportRef = useRef<SVGGElement>(null);
  const simulationRef = useRef<ReturnType<typeof forceSimulation<ForceNode>> | null>(null);
  const zoomRef = useRef<ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const previousNodeIdsRef = useRef<Set<string>>(new Set());
  const userMovedRef = useRef(false);
  const [size, setSize] = useState({ width: FALLBACK_WIDTH, height: FALLBACK_HEIGHT });
  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  const degree = useMemo(() => {
    const next = new Map<string, number>();
    for (const edge of graph.edges) {
      next.set(edge.source, (next.get(edge.source) ?? 0) + 1);
      next.set(edge.target, (next.get(edge.target) ?? 0) + 1);
    }
    return next;
  }, [graph.edges]);

  const selectedScope = useMemo(() => {
    if (!selectedNodeId) return null;
    const related = new Set([selectedNodeId]);
    for (const edge of graph.edges) {
      if (edge.source === selectedNodeId) related.add(edge.target);
      if (edge.target === selectedNodeId) related.add(edge.source);
    }
    return related;
  }, [graph.edges, selectedNodeId]);

  const data = useMemo(() => {
    const knownIds = new Set(graph.nodes.map((node) => node.id));
    const previousIds = previousNodeIdsRef.current;
    const centerX = size.width / 2;
    const centerY = size.height / 2;
    const radius = Math.min(size.width, size.height) * 0.32;
    const nodes: ForceNode[] = graph.nodes.map((node, index) => {
      const angle = ((hash(node.id) % 360) / 180) * Math.PI;
      const distance = radius * (0.22 + ((hash(`${node.id}:distance`) % 78) / 100));
      const nodeDegree = degree.get(node.id) ?? 0;
      return {
        id: node.id,
        node,
        degree: nodeDegree,
        radius: 5 + Math.min(11, Math.sqrt(nodeDegree + 1) * 2.2) + (node.evidence === "observed" ? 1.5 : 0),
        stage: flowStageForNode(node),
        isNew: !previousIds.has(node.id),
        x: centerX + Math.cos(angle + index * 0.013) * distance,
        y: centerY + Math.sin(angle + index * 0.013) * distance,
      };
    });
    const links: ForceLink[] = graph.edges
      .filter((edge) => knownIds.has(edge.source) && knownIds.has(edge.target))
      .map((edge) => ({ id: edge.id, source: edge.source, target: edge.target, edge, parallelOffset: 0 }));
    const groups = new Map<string, ForceLink[]>();
    for (const link of links) {
      const pair = [link.edge.source, link.edge.target].sort().join("\u0000");
      groups.set(pair, [...(groups.get(pair) ?? []), link]);
    }
    for (const group of groups.values()) {
      group.sort((left, right) => left.id.localeCompare(right.id));
      group.forEach((link, index) => { link.parallelOffset = (index - (group.length - 1) / 2) * 12; });
    }
    return { nodes, links };
  }, [degree, graph.edges, graph.nodes, size.height, size.width]);

  const labelNodes = useMemo(() => [...data.nodes]
    .filter((node) => node.degree > 0 || node.node.metadata.gitChangeMarker === true)
    .sort((left, right) => {
      const changeDifference = Number(right.node.metadata.gitChangeMarker === true)
        - Number(left.node.metadata.gitChangeMarker === true);
      if (changeDifference !== 0) return changeDifference;
      const evidenceDifference = Number(right.node.evidence === "observed") - Number(left.node.evidence === "observed");
      if (evidenceDifference !== 0) return evidenceDifference;
      const degreeDifference = right.degree - left.degree;
      if (degreeDifference !== 0) return degreeDifference;
      return left.id.localeCompare(right.id);
    })
    .slice(0, MAX_VISIBLE_LABELS), [data.nodes]);

  const fitGraph = useCallback(() => {
    const svg = svgRef.current;
    const behavior = zoomRef.current;
    const nodes = simulationRef.current?.nodes() ?? [];
    if (!svg || !behavior || nodes.length === 0) return;
    const left = Math.min(...nodes.map((node) => (node.x ?? 0) - node.radius));
    const right = Math.max(...nodes.map((node) => (node.x ?? 0) + node.radius));
    const top = Math.min(...nodes.map((node) => (node.y ?? 0) - node.radius));
    const bottom = Math.max(...nodes.map((node) => (node.y ?? 0) + node.radius));
    const graphWidth = Math.max(1, right - left);
    const graphHeight = Math.max(1, bottom - top);
    const padding = 46;
    const scale = Math.max(0.18, Math.min(2.2, (size.width - padding * 2) / graphWidth, (size.height - padding * 2) / graphHeight));
    const transform = zoomIdentity
      .translate(size.width / 2, size.height / 2)
      .scale(scale)
      .translate(-(left + right) / 2, -(top + bottom) / 2);
    userMovedRef.current = false;
    select(svg).call(behavior.transform, transform);
  }, [size.height, size.width]);

  const changeZoom = useCallback((factor: number) => {
    const svg = svgRef.current;
    const behavior = zoomRef.current;
    if (!svg || !behavior) return;
    userMovedRef.current = true;
    select(svg).call(behavior.scaleBy, factor);
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const measure = () => {
      const bounds = host.getBoundingClientRect();
      setSize({
        width: Math.max(320, Math.round(bounds.width) || FALLBACK_WIDTH),
        height: Math.max(260, Math.round(bounds.height) || FALLBACK_HEIGHT),
      });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(host);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const svg = svgRef.current;
    const viewport = viewportRef.current;
    if (!svg || !viewport) return;
    const behavior = zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.12, 4])
      .extent([[0, 0], [size.width, size.height]])
      .on("start", (event) => {
        if (event.sourceEvent) userMovedRef.current = true;
      })
      .on("zoom", (event) => select(viewport).attr("transform", event.transform.toString()));
    select(svg).call(behavior).on("dblclick.zoom", null);
    zoomRef.current = behavior;
    return () => {
      select(svg).on(".zoom", null);
      zoomRef.current = null;
    };
  }, [size.height, size.width]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || data.nodes.length === 0) return;
    const nodeSelection = select(viewport)
      .selectAll<SVGGElement, ForceNode>(".force-node")
      .data(data.nodes);
    const linkSelection = select(viewport)
      .selectAll<SVGPathElement, ForceLink>(".force-link")
      .data(data.links);
    const labelSelection = select(viewport)
      .selectAll<SVGTextElement, ForceNode>(".force-node-label")
      .data(labelNodes);
    const random = seededRandom(hash(data.nodes.map((node) => node.id).join("|")));
    const simulation = forceSimulation<ForceNode>(data.nodes)
      .randomSource(random)
      .force("link", forceLink<ForceNode, ForceLink>(data.links)
        .id((node) => node.id)
        .distance((link) => 42 + Math.min(58, (link.source as ForceNode).radius + (link.target as ForceNode).radius + 12))
        .strength((link) => link.edge.evidence === "observed" ? 0.3 : 0.18))
      .force("charge", forceManyBody<ForceNode>().strength((node) => -48 - node.radius * 5).distanceMax(420))
      .force("collide", forceCollide<ForceNode>().radius((node) => node.radius + 4).strength(0.92))
      .force("center", forceCenter(size.width / 2, size.height / 2).strength(0.11))
      .force("x", forceX<ForceNode>(size.width / 2).strength(0.018))
      .force("y", forceY<ForceNode>(size.height / 2).strength(0.018))
      .alphaDecay(0.045)
      .velocityDecay(0.34);
    simulationRef.current?.stop();
    simulationRef.current = simulation;
    const renderPositions = () => {
      nodeSelection.attr("transform", (node) => `translate(${node.x ?? 0},${node.y ?? 0})`);
      labelSelection
        .attr("x", (node) => (node.x ?? 0) + node.radius + 4)
        .attr("y", (node) => (node.y ?? 0) + 2.5);
      linkSelection.attr("d", (link) => {
        const source = link.source as ForceNode;
        const target = link.target as ForceNode;
        const sx = source.x ?? 0, sy = source.y ?? 0, tx = target.x ?? 0, ty = target.y ?? 0;
        if (source.id === target.id) {
          const radius = 18 + Math.abs(link.parallelOffset);
          return `M ${sx} ${sy} C ${sx + radius} ${sy - radius * 2} ${sx - radius} ${sy - radius * 2} ${sx} ${sy}`;
        }
        const canonical = source.id < target.id ? { x: sx, y: sy, tx, ty } : { x: tx, y: ty, tx: sx, ty: sy };
        const dx = canonical.tx - canonical.x, dy = canonical.ty - canonical.y;
        const length = Math.max(1, Math.hypot(dx, dy));
        const cx = (sx + tx) / 2 - (dy / length) * link.parallelOffset;
        const cy = (sy + ty) / 2 + (dx / length) * link.parallelOffset;
        return `M ${sx} ${sy} Q ${cx} ${cy} ${tx} ${ty}`;
      });
    };
    simulation.on("tick", renderPositions).on("end", () => {
      renderPositions();
      if (!userMovedRef.current) fitGraph();
    });
    const reducedMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    if (reducedMotion) {
      simulation.stop();
      for (let index = 0; index < 180; index += 1) simulation.tick();
      renderPositions();
      fitGraph();
    }
    const dragBehavior = drag<SVGGElement, ForceNode>()
      .on("start", (event, node) => {
        if (!event.active) simulation.alphaTarget(0.18).restart();
        node.fx = node.x;
        node.fy = node.y;
      })
      .on("drag", (event, node) => {
        node.fx = event.x;
        node.fy = event.y;
      })
      .on("end", (event, node) => {
        if (!event.active) simulation.alphaTarget(0);
        node.fx = null;
        node.fy = null;
      });
    nodeSelection.call(dragBehavior);
    previousNodeIdsRef.current = new Set(data.nodes.map((node) => node.id));
    return () => {
      simulation.stop();
      nodeSelection.on(".drag", null);
      if (simulationRef.current === simulation) simulationRef.current = null;
    };
  }, [data.links, data.nodes, fitGraph, labelNodes, size.height, size.width]);

  const showTooltip = (event: PointerEvent<SVGGElement>, node: GraphNode) => {
    const bounds = hostRef.current?.getBoundingClientRect();
    setTooltip({
      node,
      x: Math.min((bounds?.width ?? size.width) - 220, Math.max(12, event.clientX - (bounds?.left ?? 0) + 12)),
      y: Math.min((bounds?.height ?? size.height) - 72, Math.max(12, event.clientY - (bounds?.top ?? 0) + 12)),
    });
  };

  const nodeKeyDown = (event: KeyboardEvent<SVGGElement>, node: GraphNode) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelectNode(node.id);
    }
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && node.source) onOpenSource(node, "pinned");
  };

  const selectedNode = graph.nodes.find((node) => node.id === selectedNodeId);
  const hasGitChanges = graph.nodes.some((node) => node.metadata.gitChanged === true);
  const paging = graph as GraphSnapshot & { clientPagingLimited?: boolean };
  return (
    <div
      className={`force-graph${hasGitChanges ? " has-git-changes" : ""}`}
      ref={hostRef}
      data-testid="graph-canvas"
      style={{
        width: `${applicationZoom * 100}%`,
        height: `${applicationZoom * 100}%`,
        transform: `scale(${1 / applicationZoom})`,
        transformOrigin: "top left",
      }}
    >
      <svg
        ref={svgRef}
        className="force-graph-svg"
        viewBox={`0 0 ${size.width} ${size.height}`}
        role="group"
        aria-label={`Workspace relationships with ${graph.nodes.length} nodes and ${graph.edges.length} edges`}
      >
        <defs>
          <pattern id="force-dot-grid" width="24" height="24" patternUnits="userSpaceOnUse">
            <circle cx="1" cy="1" r="0.65" fill="var(--line-strong)" opacity="0.38" />
          </pattern>
        </defs>
        <rect className="force-graph-hit-area" width={size.width} height={size.height} onDoubleClick={fitGraph} />
        <g ref={viewportRef} data-testid="force-graph-viewport">
          <g className="force-links" aria-hidden="true">
            {data.links.map((link) => {
              const sourceId = linkNodeId(link.source);
              const targetId = linkNodeId(link.target);
              const highlighted = selectedNodeId === sourceId || selectedNodeId === targetId;
              return (
                <path
                  key={link.id}
                  className={`force-link${highlighted ? " is-highlighted" : ""}${selectedScope && !highlighted ? " is-dimmed" : ""}${link.edge.evidence === "observed" ? " is-observed" : ""}${link.edge.metadata.gitChanged === true ? " is-git-change" : ""}${link.edge.metadata.gitChangeKind === "insert" ? " is-git-insert" : ""}${link.edge.metadata.gitChangeKind === "delete" || link.edge.metadata.gitChangeKind === "conflict" ? " is-git-remove" : ""}`}
                  data-edge-id={link.id}
                />
              );
            })}
          </g>
          <g className="force-nodes">
            {data.nodes.map((datum) => {
              const selected = datum.id === selectedNodeId;
              const dimmed = Boolean(selectedScope && !selectedScope.has(datum.id));
              return (
                <g
                  key={datum.id}
                  className={`force-node ${stageClass(datum.stage)}${selected ? " is-selected" : ""}${dimmed ? " is-dimmed" : ""}${datum.node.evidence === "observed" ? " is-observed" : ""}${datum.node.metadata.gitChanged === true ? " is-git-changed" : ""}${datum.node.metadata.gitChangeKind === "insert" ? " is-git-insert" : ""}${datum.node.metadata.gitChangeKind === "delete" || datum.node.metadata.gitChangeKind === "conflict" ? " is-git-remove" : ""}${datum.isNew ? " is-new" : ""}`}
                  data-node-id={datum.id}
                  role="button"
                  tabIndex={0}
                  aria-label={nodeAriaLabel(datum.node)}
                  aria-pressed={selected}
                  onClick={() => onSelectNode(datum.id)}
                  onDoubleClick={() => datum.node.source && onOpenSource(datum.node, "pinned")}
                  onKeyDown={(event) => nodeKeyDown(event, datum.node)}
                  onPointerEnter={(event) => showTooltip(event, datum.node)}
                  onPointerLeave={() => setTooltip(null)}
                  onFocus={() => setTooltip({ node: datum.node, x: 14, y: 14 })}
                  onBlur={() => setTooltip(null)}
                >
                  {datum.node.evidence === "observed" && <circle className="force-node-halo" r={datum.radius + 4} />}
                  <circle className="force-node-bubble" r={datum.radius} />
                  {selected && <circle className="force-node-selection" r={datum.radius + 5} />}
                </g>
              );
            })}
          </g>
          <g className="force-labels" aria-hidden="true">
            {labelNodes.map((datum) => (
              <text
                key={datum.id}
                className={`force-node-label${datum.id === selectedNodeId ? " is-selected" : ""}${selectedScope && !selectedScope.has(datum.id) ? " is-dimmed" : ""}`}
              >
                {shortLabel(datum.node.label)}
              </text>
            ))}
          </g>
        </g>
      </svg>

      <div className="force-graph-controls" role="group" aria-label="Relationship graph controls">
        <button type="button" onClick={() => changeZoom(1.25)} aria-label="Zoom in"><Plus size={13} /></button>
        <button type="button" onClick={() => changeZoom(0.8)} aria-label="Zoom out"><Minus size={13} /></button>
        <button type="button" onClick={fitGraph} aria-label="Fit relationships"><ArrowsOut size={13} /></button>
        {selectedNode && onTraceNode && (
          <button type="button" onClick={() => onTraceNode(selectedNode.id)} aria-label="Load selected sequence" title="Load selected sequence">
            <Crosshair size={13} />
          </button>
        )}
      </div>

      {graph.nextCursor && !paging.clientPagingLimited && onLoadMore && (
        <button type="button" className="force-load-more" onClick={onLoadMore} disabled={loadingMore}>
          {loadingMore ? "Loading relationships..." : "Load more relationships"}
        </button>
      )}

      <div className="force-graph-legend" aria-label="Relationship graph legend">
        <span><i className="is-stage-api" /> API</span>
        <span><i className="is-stage-service" /> Logic</span>
        <span><i className="is-stage-data" /> Data</span>
        {hasGitChanges && <span><i className="is-git-modify" /> Modified</span>}
        {hasGitChanges && <span><i className="is-git-insert" /> Inserted</span>}
        <span><i className="is-observed" /> Observed</span>
      </div>

      {tooltip && (
        <div className="force-graph-tooltip" style={{ left: tooltip.x, top: tooltip.y }} role="tooltip">
          <strong>{tooltip.node.label}</strong>
          <span>{tooltip.node.kind} | {tooltip.node.evidence}</span>
          <small>{sourceLabel(tooltip.node)}</small>
        </div>
      )}

      <div className="visually-hidden" aria-live="polite">
        {selectedNode ? `Selected ${nodeAriaLabel(selectedNode)}` : "No relationship node selected"}
      </div>
    </div>
  );
}
