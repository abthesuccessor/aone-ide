import type { GraphNode, GraphSnapshot } from "../../types";
import { ExecutionFlowGraph } from "./ExecutionFlowGraph";

interface StructuredGraphProps {
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode) => void;
  onTraceNode?: (nodeId: string) => void;
  onLoadMore?: () => void;
  loadingMore?: boolean;
}

export function StructuredGraph(props: StructuredGraphProps) {
  return <ExecutionFlowGraph {...props} mode="trace" />;
}
