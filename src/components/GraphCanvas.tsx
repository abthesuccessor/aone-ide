import { ArrowsOut, Warning } from "@phosphor-icons/react";
import { useMemo } from "react";
import { ForceRelationshipGraph } from "../features/graph/ForceRelationshipGraph";
import { graphForLens } from "../features/graph/model";
import type { GraphLens, GraphNode, GraphSnapshot } from "../types";
import type { EditorOpenMode } from "../features/editor/model";

interface GraphCanvasProps {
  graph: GraphSnapshot;
  lens: GraphLens;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode, mode?: EditorOpenMode) => void;
  onTraceNode?: (nodeId: string) => void;
  onLoadMore?: () => void;
  loadingMore?: boolean;
  loading?: boolean;
  error?: string | null;
  applicationZoom?: number;
}

function GraphState({ error }: { error?: string | null }) {
  if (error) {
    return (
      <div className="graph-state graph-error" role="alert">
        <Warning size={28} weight="duotone" />
        <strong>Graph unavailable</strong>
        <span>{error}</span>
      </div>
    );
  }
  return (
    <div className="graph-state" aria-live="polite">
      <div className="graph-loader" />
      <strong>Building the workspace graph</strong>
      <span>Resolving entry points, interfaces, data, and external boundaries...</span>
    </div>
  );
}

function EmptyGraph() {
  return (
    <div className="graph-canvas-wrap">
      <div className="graph-state">
        <ArrowsOut size={28} weight="thin" />
        <strong>No relationships in this lens</strong>
        <span>Scan the workspace again to rebuild indexed relationships.</span>
      </div>
    </div>
  );
}

export function GraphCanvas({
  graph,
  lens,
  selectedNodeId,
  onSelectNode,
  onOpenSource,
  onTraceNode,
  onLoadMore,
  loadingMore,
  loading = false,
  error = null,
  applicationZoom = 1,
}: GraphCanvasProps) {
  const visibleGraph = useMemo(() => graphForLens(graph, lens), [graph, lens]);

  if ((loading || error) && visibleGraph.nodes.length === 0) return <GraphState error={error} />;
  if (visibleGraph.nodes.length === 0) return <EmptyGraph />;

  return (
    <div className="graph-mode-shell">
      <ForceRelationshipGraph
        graph={visibleGraph}
        selectedNodeId={selectedNodeId}
        onSelectNode={onSelectNode}
        onOpenSource={onOpenSource}
        onTraceNode={onTraceNode}
        onLoadMore={onLoadMore}
        loadingMore={loadingMore}
        applicationZoom={applicationZoom}
      />
      {(loading || error) && (
        <div className={`graph-refresh-status${error ? " is-error" : ""}`} role={error ? "alert" : "status"}>
          {error ?? "Refreshing indexed relationships..."}
        </div>
      )}
    </div>
  );
}
