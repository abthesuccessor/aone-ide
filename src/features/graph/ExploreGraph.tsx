import type { GraphNode, GraphSnapshot } from "../../types";
import { ExecutionFlowGraph } from "./ExecutionFlowGraph";

interface ExploreGraphProps {
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode) => void;
  onTraceNode?: (nodeId: string) => void;
  onLoadMore?: () => void;
  loadingMore?: boolean;
}

/** The workspace map is deterministic and never starts a physics loop. */
export function ExploreGraph(props: ExploreGraphProps) {
  return <ExecutionFlowGraph {...props} mode="overview" />;
}
