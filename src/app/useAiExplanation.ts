import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { explainNode } from "../lib/bridge";
import type { AiExplanation, GraphSnapshot } from "../types";

interface AiExplanationOptions {
  selectedNodeId: string | null;
  workspaceId: string | null;
  workspaceGenerationRef: RefObject<number>;
  graph: GraphSnapshot;
}

const MAX_AI_GRAPH_NODES = 16;

/**
 * Selects a deterministic two-hop evidence corridor without asking the
 * provider to discover or authorize additional workspace facts.
 */
export function boundedEvidenceNodeIds(
  graph: GraphSnapshot,
  selectedNodeId: string,
  limit = MAX_AI_GRAPH_NODES,
): string[] {
  const available = new Set(graph.nodes.map((node) => node.id));
  if (!available.has(selectedNodeId) || limit <= 0) return [];
  const adjacency = new Map<string, Set<string>>();
  for (const edge of [...graph.edges].sort((left, right) => left.id.localeCompare(right.id))) {
    if (!available.has(edge.source) || !available.has(edge.target)) continue;
    const source = adjacency.get(edge.source) ?? new Set<string>();
    source.add(edge.target);
    adjacency.set(edge.source, source);
    const target = adjacency.get(edge.target) ?? new Set<string>();
    target.add(edge.source);
    adjacency.set(edge.target, target);
  }
  const selected = [selectedNodeId];
  const seen = new Set(selected);
  let frontier = selected;
  for (let depth = 0; depth < 2 && frontier.length > 0 && selected.length < limit; depth += 1) {
    const next = new Set<string>();
    for (const nodeId of frontier) {
      for (const relatedId of adjacency.get(nodeId) ?? []) {
        if (!seen.has(relatedId)) next.add(relatedId);
      }
    }
    frontier = [...next].sort();
    for (const nodeId of frontier) {
      if (selected.length >= limit) break;
      seen.add(nodeId);
      selected.push(nodeId);
    }
  }
  return selected;
}

export function useAiExplanation({
  selectedNodeId,
  workspaceId,
  workspaceGenerationRef,
  graph,
}: AiExplanationOptions) {
  const [explanation, setExplanation] = useState<AiExplanation | null>(null);
  const [explaining, setExplaining] = useState(false);
  const [explanationError, setExplanationError] = useState<string | null>(null);
  const requestGenerationRef = useRef(0);

  const resetExplanation = useCallback(() => {
    requestGenerationRef.current += 1;
    setExplanation(null);
    setExplanationError(null);
    setExplaining(false);
  }, []);

  useEffect(() => {
    resetExplanation();
  }, [resetExplanation, selectedNodeId, workspaceId]);

  const handleExplain = useCallback(
    async (nodeId: string) => {
      if (!workspaceId) return;
      const workspaceGeneration = workspaceGenerationRef.current;
      const requestGeneration = ++requestGenerationRef.current;
      const stale = () =>
        workspaceGeneration !== workspaceGenerationRef.current ||
        requestGeneration !== requestGenerationRef.current;
      setExplaining(true);
      setExplanationError(null);
      try {
        const corridor = boundedEvidenceNodeIds(graph, nodeId);
        // The exact selected node remains valid evidence while a newly opened
        // workspace is still loading its graph projection.
        const nodeIds = corridor.length > 0 ? corridor : [nodeId];
        const next = await explainNode({
          workspaceId,
          question:
            "Explain this component's role and how the available evidence connects it to the execution flow. Clearly separate static, observed, and inferred claims.",
          nodeIds,
          runtimeEventIds: [],
        });
        if (!stale()) setExplanation(next);
      } catch (error) {
        if (!stale()) {
          setExplanationError(error instanceof Error ? error.message : "AI explanation failed");
        }
      } finally {
        if (!stale()) setExplaining(false);
      }
    },
    [graph, workspaceGenerationRef, workspaceId],
  );

  return {
    explanation,
    explaining,
    explanationError,
    resetExplanation,
    handleExplain,
  };
}
