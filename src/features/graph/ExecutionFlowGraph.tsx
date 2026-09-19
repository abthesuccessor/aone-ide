import {
  ArrowsOut,
  CaretDown,
  CaretRight,
  Crosshair,
  MagnifyingGlass,
  Minus,
  Plus,
  ShieldCheck,
  Warning,
} from "@phosphor-icons/react";
import { select } from "d3-selection";
import { zoom, zoomIdentity, type ZoomBehavior, type ZoomTransform } from "d3-zoom";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { EvidenceBadge } from "../../components/EvidenceBadge";
import { truncateGraphLabel } from "../../lib/graphPresentation";
import { evidenceDisplayKind, type GraphEdge, type GraphNode, type GraphSnapshot } from "../../types";
import {
  createExecutionFlowLayout,
  executionFlowEdgePath,
  FLOW_VIEW_HEIGHT,
  FLOW_VIEW_WIDTH,
  nextFlowNodeByGeometry,
  type ExecutionFlowLayout,
  type FlowLayoutMode,
  type PositionedFlowNode,
} from "./flowLayout";
import {
  FLOW_STAGES,
  flowStageForNode,
  latestObservedFlowNodeId,
  searchFlowNodes,
  traceScopeNodeIds,
} from "./flowModel";

interface ExecutionFlowGraphProps {
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode) => void;
  onTraceNode?: (nodeId: string) => void;
  onLoadMore?: () => void;
  loadingMore?: boolean;
  mode: FlowLayoutMode;
}

function basename(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() ?? path;
}

function sourceSummary(node: GraphNode): string {
  if (!node.source) return "No exact source range";
  return `${basename(node.source.relativePath)}:${node.source.startLine}`;
}

function fitTransform(layout: ExecutionFlowLayout): ZoomTransform {
  const padding = 44;
  const scale = Math.max(0.12, Math.min(
    (FLOW_VIEW_WIDTH - padding * 2) / layout.width,
    (FLOW_VIEW_HEIGHT - padding * 2) / layout.height,
    1,
  ));
  return zoomIdentity
    .translate(
      (FLOW_VIEW_WIDTH - layout.width * scale) / 2,
      (FLOW_VIEW_HEIGHT - layout.height * scale) / 2,
    )
    .scale(scale);
}

function readableTransform(layout: ExecutionFlowLayout, selected?: PositionedFlowNode): ZoomTransform {
  const scale = selected ? 0.9 : Math.min(0.9, (FLOW_VIEW_WIDTH - 24) / layout.width);
  if (!selected) return zoomIdentity.translate((FLOW_VIEW_WIDTH - layout.width * scale) / 2, 12).scale(scale);
  const centerX = selected.x + selected.width / 2;
  const centerY = selected.y + selected.height / 2;
  return zoomIdentity
    .translate(FLOW_VIEW_WIDTH / 2 - centerX * scale, FLOW_VIEW_HEIGHT / 2 - centerY * scale)
    .scale(scale);
}

function focusNode(svg: SVGSVGElement, nodeId: string) {
  const nodes = Array.from(svg.querySelectorAll<SVGGElement>(".flow-node"));
  nodes.find((node) => node.dataset.nodeId === nodeId)?.focus();
}

function nodeKeyDown(
  event: KeyboardEvent<SVGGElement>,
  positioned: PositionedFlowNode,
  layout: ExecutionFlowLayout,
  onSelectNode: (nodeId: string) => void,
  onOpenSource: (node: GraphNode) => void,
) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    onSelectNode(positioned.node.id);
  }
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && positioned.node.source) {
    onOpenSource(positioned.node);
  }
  if (!["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const next = nextFlowNodeByGeometry(layout.nodes, positioned.node.id, event.key);
  if (!next) return;
  onSelectNode(next.node.id);
  const svg = event.currentTarget.ownerSVGElement;
  if (svg) focusNode(svg, next.node.id);
}

function SelectedFlowSummary({ graph, selectedNodeId }: { graph: GraphSnapshot; selectedNodeId: string | null }) {
  const selected = graph.nodes.find((node) => node.id === selectedNodeId);
  if (!selected) return null;
  const nodes = new Map(graph.nodes.map((node) => [node.id, node]));
  const relations = graph.edges.filter((edge) => edge.source === selected.id || edge.target === selected.id);
  return (
    <section className="visually-hidden" aria-label="Selected execution flow" aria-live="polite" aria-atomic="true">
      <h3>{selected.label}</h3>
      <p>{flowStageForNode(selected)} stage. {selected.evidence} evidence. {sourceSummary(selected)}.</p>
      <ul>
        {relations.length > 0 ? relations.map((edge) => {
          const incoming = edge.target === selected.id;
          const neighborId = incoming ? edge.source : edge.target;
          return (
            <li key={edge.id}>
              {incoming ? "Incoming" : "Outgoing"} {edge.kind} {incoming ? "from" : "to"} {nodes.get(neighborId)?.label ?? neighborId}. {edge.evidence} evidence.
            </li>
          );
        }) : <li>No direct relationships in the loaded graph.</li>}
      </ul>
    </section>
  );
}

function FlowSearch({ graph, onSelectNode }: Pick<ExecutionFlowGraphProps, "graph" | "onSelectNode">) {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const results = useMemo(() => searchFlowNodes(graph, query), [graph, query]);
  const choose = (node: GraphNode) => {
    setQuery(node.label);
    setOpen(false);
    onSelectNode(node.id);
  };
  return (
    <div className="flow-search">
      <MagnifyingGlass size={13} aria-hidden="true" />
      <label className="visually-hidden" htmlFor="flow-node-search">Find an API, runtime event, method, class, trait, repository, or source file</label>
      <input
        id="flow-node-search"
        value={query}
        type="search"
        autoComplete="off"
        placeholder="Find API, method, trait, repository..."
        aria-expanded={open && results.length > 0}
        aria-controls="flow-search-results"
        onFocus={() => setOpen(true)}
        onChange={(event) => { setQuery(event.target.value); setOpen(true); }}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && results[0]) { event.preventDefault(); choose(results[0]); }
        }}
      />
      {open && query.trim() && (
        <div id="flow-search-results" className="flow-search-results" role="listbox" aria-label="Execution flow search results">
          {results.length > 0 ? results.map((node) => (
            <button
              key={node.id}
              type="button"
              role="option"
              aria-selected="false"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => choose(node)}
            >
              <strong>{node.label}</strong>
              <span>{FLOW_STAGES.find((stage) => stage.id === flowStageForNode(node))?.label} | {sourceSummary(node)}</span>
            </button>
          )) : <p>No loaded flow nodes match this search.</p>}
        </div>
      )}
    </div>
  );
}

function edgeIsObserved(edge: GraphEdge): boolean {
  return edge.evidence === "observed";
}

export function ExecutionFlowGraph({
  graph,
  selectedNodeId,
  onSelectNode,
  onOpenSource,
  onTraceNode,
  onLoadMore,
  loadingMore = false,
  mode,
}: ExecutionFlowGraphProps) {
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(() => new Set());
  const rootNodeId = selectedNodeId && graph.nodes.some((node) => node.id === selectedNodeId)
    ? selectedNodeId
    : null;
  const layout = useMemo(() => createExecutionFlowLayout(graph, {
    rootNodeId,
    expandedGroups,
    mode,
  }), [expandedGroups, graph, mode, rootNodeId]);
  const positionedById = useMemo(() => new Map(layout.nodes.map((node) => [node.node.id, node])), [layout.nodes]);
  const selected = rootNodeId ? positionedById.get(rootNodeId) : undefined;
  const selectedTraceIds = useMemo(
    () => mode === "overview" && rootNodeId ? traceScopeNodeIds(graph, rootNodeId) : null,
    [graph, mode, rootNodeId],
  );
  const latestObservedNodeId = useMemo(() => latestObservedFlowNodeId(graph), [graph]);
  const svgRef = useRef<SVGSVGElement>(null);
  const zoomRef = useRef<ZoomBehavior<SVGSVGElement, unknown> | null>(null);
  const viewRef = useRef<ZoomTransform>(zoomIdentity);
  const layoutRef = useRef(layout);
  const selectedRef = useRef(selected);
  const focusKey = `${mode}:${rootNodeId ?? "workspace"}`;
  const focusKeyRef = useRef(focusKey);
  const previousFocusKeyRef = useRef<string | null>(null);
  layoutRef.current = layout;
  selectedRef.current = selected;
  focusKeyRef.current = focusKey;
  const paging = graph as GraphSnapshot & {
    nextCursor?: string;
    clientPagingLimited?: boolean;
  };
  const nextCursor = paging.nextCursor;
  const selectedGraphNode = rootNodeId ? graph.nodes.find((node) => node.id === rootNodeId) : undefined;
  const runtimeTraceTarget = selectedGraphNode?.kind === "runtime-event"
    ? selectedGraphNode.metadata.mappedNodeId ?? selectedGraphNode.metadata.sourceNodeId
    : rootNodeId;
  const traceTargetId = typeof runtimeTraceTarget === "string" ? runtimeTraceTarget : null;

  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    const initialLayout = layoutRef.current;
    const behavior = zoom<SVGSVGElement, unknown>()
      .extent([[0, 0], [FLOW_VIEW_WIDTH, FLOW_VIEW_HEIGHT]])
      .translateExtent([[-260, -260], [initialLayout.width + 260, initialLayout.height + 260]])
      .scaleExtent([0.12, 2.4])
      .on("zoom", (event) => {
        viewRef.current = event.transform;
        select(svg).select<SVGGElement>("[data-testid='graph-viewport']")
          .attr("transform", event.transform.toString());
      });
    const svgSelection = select(svg);
    svgSelection.call(behavior).on("dblclick.zoom", null);
    zoomRef.current = behavior;
    previousFocusKeyRef.current = focusKeyRef.current;
    svgSelection.call(behavior.transform, readableTransform(initialLayout, selectedRef.current));
    return () => {
      svgSelection.on(".zoom", null);
      zoomRef.current = null;
    };
  }, []);

  useEffect(() => {
    zoomRef.current?.translateExtent([[-260, -260], [layout.width + 260, layout.height + 260]]);
  }, [layout.height, layout.width]);

  useEffect(() => {
    if (previousFocusKeyRef.current === focusKey) return;
    previousFocusKeyRef.current = focusKey;
    if (!svgRef.current || !zoomRef.current) return;
    select(svgRef.current).call(zoomRef.current.transform, readableTransform(layout, selected));
  }, [focusKey, layout, selected]);

  const changeZoom = (factor: number) => {
    if (svgRef.current && zoomRef.current) select(svgRef.current).call(zoomRef.current.scaleBy, factor);
  };
  const fitGraph = () => {
    if (svgRef.current && zoomRef.current) select(svgRef.current).call(zoomRef.current.transform, fitTransform(layout));
  };
  const focusSelection = () => {
    if (!selected || !svgRef.current || !zoomRef.current) return;
    select(svgRef.current).call(zoomRef.current.transform, readableTransform(layout, selected));
  };
  const toggleGroup = (key: string) => setExpandedGroups((current) => {
    const next = new Set(current);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    return next;
  });

  return (
    <div className={`graph-canvas-wrap execution-flow is-${mode}`} data-testid="graph-canvas">
      <div className="flow-command-bar">
        <FlowSearch graph={graph} onSelectNode={onSelectNode} />
        <div className="flow-mode-summary" role="status">
          {rootNodeId ? `Focused trace: ${graph.nodes.find((node) => node.id === rootNodeId)?.label ?? rootNodeId}` : "Workspace execution map"}
        </div>
        {traceTargetId && onTraceNode && (
          <button className="flow-command-action" type="button" disabled={loadingMore} onClick={() => onTraceNode(traceTargetId)}>
            {selectedGraphNode?.kind !== "runtime-event"
              ? "Trace from card"
              : selectedGraphNode.metadata.sourceNodeId ? "Trace observed source" : "Trace mapped source (inferred)"}
          </button>
        )}
        {nextCursor && !paging.clientPagingLimited && onLoadMore && (
          <button className="flow-command-action" type="button" disabled={loadingMore} onClick={onLoadMore}>
            {loadingMore ? "Loading…" : "Load next"}
          </button>
        )}
      </div>
      {(graph.truncated || paging.clientPagingLimited) && (
        <div className="graph-truncated-warning flow-truncated-warning" role="status">
          <Warning size={13} weight="fill" />
          <span>{paging.clientPagingLimited
            ? "Client display cap reached; additional nodes or relationships were summarized. Choose a narrower API or root to continue."
            : nextCursor
            ? "The root has more relationships. Use Load next; search covers only loaded nodes."
            : "The loaded result is bounded. Search covers loaded nodes; omitted facts are not presented as complete."}</span>
        </div>
      )}
      <div className="graph-zoom-controls" aria-label="Graph zoom controls">
        <button type="button" onClick={() => changeZoom(1.15)} aria-label="Zoom in"><Plus size={13} /></button>
        <button type="button" onClick={() => changeZoom(0.87)} aria-label="Zoom out"><Minus size={13} /></button>
        <button type="button" onClick={focusSelection} aria-label="Zoom to selected node" disabled={!selected}><Crosshair size={13} /></button>
        <button type="button" onClick={fitGraph} aria-label="Fit graph"><ArrowsOut size={13} /></button>
      </div>
      <svg
        ref={svgRef}
        className="graph-svg flow-svg"
        viewBox={`0 0 ${FLOW_VIEW_WIDTH} ${FLOW_VIEW_HEIGHT}`}
        role="group"
        aria-label={`Execution flow with ${layout.nodes.length} visible cards from ${layout.scopeNodeCount} loaded trace nodes`}
      >
        <defs>
          {(["static", "observed", "inferred"] as const).map((evidence) => (
            <marker key={evidence} id={`flow-arrow-${mode}-${evidence}`} viewBox="0 0 10 10" refX="9" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" className={`marker-${evidence}`} />
            </marker>
          ))}
        </defs>
        <rect className="graph-hit-area" width={FLOW_VIEW_WIDTH} height={FLOW_VIEW_HEIGHT} />
        <g data-testid="graph-viewport">
          <g className="flow-stages" aria-hidden="true">
            {layout.stages.map((positionedStage) => {
              const stage = FLOW_STAGES.find((candidate) => candidate.id === positionedStage.id)!;
              return (
                <g key={stage.id} className="flow-stage" data-stage={stage.id}>
                  <rect
                    x={positionedStage.x}
                    y={positionedStage.y}
                    width={positionedStage.width}
                    height={positionedStage.height}
                    rx={7}
                  />
                  <text className="flow-stage-title" x={positionedStage.x + 12} y={positionedStage.y + 21}>{stage.label}</text>
                  <text
                    className="flow-stage-count"
                    x={positionedStage.x + positionedStage.width - 12}
                    y={positionedStage.y + 21}
                    textAnchor="end"
                  >
                    {layout.stageVisibleCounts[stage.id]}/{layout.stageTruthCounts[stage.id]}
                  </text>
                </g>
              );
            })}
          </g>
          <g className="flow-groups">
            {layout.groups.map((group) => (
              <g key={group.key} className="flow-group" data-group-key={group.key}>
                <rect className="flow-group-surface" x={group.x} y={group.y} width={group.width} height={group.height} rx={7} />
                <g
                  className="flow-group-toggle"
                  role="button"
                  tabIndex={0}
                  aria-expanded={group.expanded}
                  aria-label={`${group.expanded ? "Collapse" : "Expand"} ${group.label}, ${group.truthCount} nodes`}
                  transform={`translate(${group.x + 10} ${group.y + 11})`}
                  onClick={() => toggleGroup(group.key)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") { event.preventDefault(); toggleGroup(group.key); }
                  }}
                >
                  {group.expanded ? <CaretDown size={12} /> : <CaretRight size={12} />}
                  <text x={18} y={10}>{truncateGraphLabel(group.label, 25)}</text>
                  <text className="flow-group-count" x={group.width - 31} y={10} textAnchor="end">
                    {group.observedCount > 0 ? `${group.observedCount} observed | ` : ""}{group.truthCount}
                  </text>
                </g>
                {(group.hiddenCount > 0 || group.expanded) && (
                  <text className="flow-group-more" x={group.x + 12} y={group.y + group.height - 10}>
                    {group.hiddenCount > 0 ? `${group.hiddenCount} more. Expand or search.` : "All loaded nodes shown. Select to trace."}
                  </text>
                )}
              </g>
            ))}
          </g>
          <g className="flow-edges" aria-hidden="true">
            {layout.edges.map((edge) => {
              const source = positionedById.get(edge.source);
              const target = positionedById.get(edge.target);
              if (!source || !target) return null;
              const evidence = evidenceDisplayKind(edge.evidence);
              const selectedEdge = Boolean(rootNodeId && (edge.source === rootNodeId || edge.target === rootNodeId));
              const dimmed = Boolean(selectedTraceIds && (!selectedTraceIds.has(edge.source) || !selectedTraceIds.has(edge.target)));
              const observed = edgeIsObserved(edge);
              const livePath = observed && edge.target === latestObservedNodeId;
              return (
                <g key={edge.id} className={`${selectedEdge ? "is-selected" : ""}${dimmed ? " is-dimmed" : ""}${livePath ? " is-live-path" : ""}`}>
                  <title>{`${source.node.label} ${edge.kind} ${target.node.label}. ${edge.evidence} evidence.`}</title>
                  <path
                    className={`graph-edge flow-edge evidence-${evidence}${observed ? " is-observed" : ""}`}
                    d={executionFlowEdgePath(source, target)}
                    fill="none"
                    markerEnd={`url(#flow-arrow-${mode}-${evidence})`}
                  />
                  {(selectedEdge || observed) && (
                    <text
                      className={`edge-label evidence-${evidence}`}
                      x={(source.x + source.width / 2 + target.x + target.width / 2) / 2}
                      y={(source.y + source.height / 2 + target.y + target.height / 2) / 2 - 6}
                      textAnchor="middle"
                    >
                      {truncateGraphLabel(`${edge.kind} | ${edge.evidence}`, 32)}
                    </text>
                  )}
                </g>
              );
            })}
          </g>
          <g className="flow-nodes">
            {layout.nodes.map((positioned, index) => {
              const node = positioned.node;
              const evidence = evidenceDisplayKind(node.evidence);
              const selectedNode = node.id === rootNodeId;
              const liveNode = node.id === latestObservedNodeId;
              const dimmed = Boolean(selectedTraceIds && !selectedTraceIds.has(node.id));
              return (
                <g
                  key={node.id}
                  className={`graph-node flow-node evidence-${evidence}${positioned.observed ? " is-observed" : ""}${liveNode ? " is-live" : ""}${selectedNode ? " is-selected" : ""}${dimmed ? " is-dimmed" : ""}`}
                  transform={`translate(${positioned.x} ${positioned.y})`}
                  role="button"
                  tabIndex={selectedNode || (!rootNodeId && index === 0) ? 0 : -1}
                  data-node-id={node.id}
                  data-stage={positioned.stage}
                  data-observed={positioned.observed}
                  aria-pressed={selectedNode}
                  aria-label={`${node.label}, ${FLOW_STAGES.find((stage) => stage.id === positioned.stage)?.label}, ${node.kind}, ${node.evidence} evidence${liveNode ? ", latest observed event" : ""}, ${sourceSummary(node)}`}
                  onClick={() => onSelectNode(node.id)}
                  onDoubleClick={() => { if (node.source) onOpenSource(node); }}
                  onKeyDown={(event) => nodeKeyDown(event, positioned, layout, onSelectNode, onOpenSource)}
                >
                  <title>{`${node.label}\n${positioned.stage} | ${node.kind} | ${node.evidence}\n${sourceSummary(node)}`}</title>
                  <rect width={positioned.width} height={positioned.height} rx={5} />
                  <rect className="node-evidence-rail" width={3} height={positioned.height - 2} x={1} y={1} rx={2} />
                  <text className="flow-node-label" x={12} y={24}>{truncateGraphLabel(node.label, 23)}</text>
                  {positioned.observed && (
                    <circle className="flow-node-observed" cx={positioned.width - 11} cy={12} r={3} />
                  )}
                </g>
              );
            })}
          </g>
        </g>
      </svg>
      <div className="visually-hidden" role="status">
        <span>{layout.nodes.length} visible / {layout.scopeNodeCount} trace nodes</span>
        <span>{layout.edges.length} visible / {layout.scopeEdgeCount} trace edges</span>
        {latestObservedNodeId && (
          <span>Live observed card: {graph.nodes.find((node) => node.id === latestObservedNodeId)?.label}</span>
        )}
        {layout.omittedVisibleEdgeCount > 0 && (
          <span>{layout.omittedVisibleEdgeCount} visible-card relationships summarized. Select a card to prioritize its edges.</span>
        )}
        {layout.hiddenNodeCount > 0 && <span>{layout.hiddenNodeCount} hidden. Search reveals any loaded node.</span>}
        {layout.hiddenGroupCount > 0 && <span>{layout.hiddenGroupCount} source groups summarized</span>}
      </div>
      <div className="visually-hidden"><ShieldCheck size={12} /> Observed marks boundary events or explicit source spans only. Internal hops keep their own evidence.</div>
      <div className="evidence-legend" aria-label="Evidence legend">
        <EvidenceBadge evidence="resolved" />
        <EvidenceBadge evidence="observed" />
        <EvidenceBadge evidence="inferred" />
      </div>
      <SelectedFlowSummary graph={graph} selectedNodeId={rootNodeId} />
    </div>
  );
}
