import { useCallback, useRef, useState, type MutableRefObject } from "react";
import { GraphQueryProjectionEnum } from "../generated/ipc/models/GraphQuery";
import type { ApiEndpointInventoryItem } from "../features/api-catalog/model";
import { queryGraph } from "../lib/bridge";
import type { GraphSnapshot } from "../types";
import {
  mergeExecutionFlow,
  queryExecutionFlow,
  type ExecutionFlowSnapshot,
} from "./graphQueries";
import { EMPTY_GRAPH, type MainView } from "./model";
import type { GraphLens } from "../types";

interface WorkspaceGraphOptions {
  generationRef: MutableRefObject<number>;
  setLens: (lens: GraphLens) => void;
  setMainView: (view: MainView) => void;
  setSelectedNodeId: (nodeId: string | null) => void;
  notify: (text: string, tone?: "info" | "success" | "warning" | "error") => void;
}

export function useWorkspaceGraphs(options: WorkspaceGraphOptions) {
  const [overview, setOverview] = useState<GraphSnapshot>(EMPTY_GRAPH);
  const [detail, setDetail] = useState<GraphSnapshot>(EMPTY_GRAPH);
  const [flow, setFlow] = useState<ExecutionFlowSnapshot>(EMPTY_GRAPH);
  const [flowRootId, setFlowRootId] = useState<string | null>(null);
  const [flowQueryRootId, setFlowQueryRootId] = useState<string | null>(null);
  const [flowLoading, setFlowLoading] = useState(false);
  const [flowError, setFlowError] = useState<string | null>(null);
  const [searchResult, setSearchResult] = useState<GraphSnapshot>(EMPTY_GRAPH);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const flowRequestRef = useRef(0);
  const searchRequestRef = useRef(0);

  const replace = useCallback((
    nextOverview: GraphSnapshot,
    nextDetail: GraphSnapshot,
    nextFlow: ExecutionFlowSnapshot,
  ) => {
    flowRequestRef.current += 1;
    setOverview(nextOverview);
    setDetail(nextDetail);
    setFlow(nextFlow);
    const rootId = nextFlow.nodes.find((node) => node.metadata.flowRoot === true)?.id
      ?? nextFlow.nodes[0]?.id
      ?? null;
    setFlowRootId(rootId);
    setFlowQueryRootId(rootId);
    setFlowError(null);
    setFlowLoading(false);
    searchRequestRef.current += 1;
    setSearchResult(EMPTY_GRAPH);
    setSearchQuery("");
    setSearchError(null);
    setSearchLoading(false);
  }, []);

  const clear = useCallback(() => replace(EMPTY_GRAPH, EMPTY_GRAPH, EMPTY_GRAPH), [replace]);

  const traceRoot = useCallback(async (
    rootId: string,
    fallbackId?: string,
    targetView: MainView = "graph",
  ): Promise<void> => {
    const generation = options.generationRef.current;
    const request = ++flowRequestRef.current;
    options.setLens("system");
    options.setMainView(targetView);
    setFlowLoading(true);
    setFlowError(null);
    try {
      let resolvedRoot = rootId;
      let next: ExecutionFlowSnapshot;
      try {
        next = await queryExecutionFlow(rootId);
      } catch (error) {
        if (!fallbackId || fallbackId === rootId) throw error;
        resolvedRoot = fallbackId;
        next = await queryExecutionFlow(fallbackId);
      }
      if (generation !== options.generationRef.current || request !== flowRequestRef.current) return;
      const displayRootId = next.nodes.some((node) => node.id === resolvedRoot)
        ? resolvedRoot
        : fallbackId && next.nodes.some((node) => node.id === fallbackId)
          ? fallbackId
          : next.nodes.find((node) => node.metadata.flowRoot === true)?.id
            ?? next.nodes[0]?.id
            ?? null;
      setFlow(next);
      setFlowRootId(displayRootId);
      setFlowQueryRootId(resolvedRoot);
      options.setSelectedNodeId(displayRootId);
      options.notify(`Loaded execution flow for ${next.nodes.find((node) => node.id === resolvedRoot)?.label ?? resolvedRoot}`, "success");
    } catch (error) {
      if (generation !== options.generationRef.current || request !== flowRequestRef.current) return;
      const message = error instanceof Error ? error.message : "Unable to load this execution flow";
      setFlowError(message);
      options.notify(message, "error");
    } finally {
      if (generation === options.generationRef.current && request === flowRequestRef.current) setFlowLoading(false);
    }
  }, [options]);

  const traceEndpoint = useCallback((endpoint: ApiEndpointInventoryItem, targetView?: MainView) => (
    traceRoot(endpoint.id, endpoint.handler?.id, targetView)
  ), [traceRoot]);

  const loadNext = useCallback(async (): Promise<void> => {
    if (!flowQueryRootId || !flow.nextCursor || flow.clientPagingLimited || flowLoading) return;
    const generation = options.generationRef.current;
    const root = flowQueryRootId;
    const request = ++flowRequestRef.current;
    setFlowLoading(true);
    setFlowError(null);
    try {
      const page = await queryExecutionFlow(root, flow.nextCursor);
      if (generation !== options.generationRef.current || request !== flowRequestRef.current) return;
      setFlow((current) => mergeExecutionFlow(current, page));
    } catch (error) {
      if (generation !== options.generationRef.current || request !== flowRequestRef.current) return;
      const message = error instanceof Error ? error.message : "Unable to load more execution-flow relationships";
      setFlowError(message);
      options.notify(message, "error");
    } finally {
      if (generation === options.generationRef.current && request === flowRequestRef.current) setFlowLoading(false);
    }
  }, [flow.clientPagingLimited, flow.nextCursor, flowLoading, flowQueryRootId, options]);

  const search = useCallback(async (rawQuery: string): Promise<void> => {
    const query = rawQuery.trim();
    const request = ++searchRequestRef.current;
    if (!query) {
      setSearchQuery("");
      setSearchResult(EMPTY_GRAPH);
      setSearchError(null);
      setSearchLoading(false);
      return;
    }
    const generation = options.generationRef.current;
    setSearchQuery(query);
    setSearchLoading(true);
    setSearchError(null);
    try {
      const result = await queryGraph({
        projection: GraphQueryProjectionEnum.Neighborhood,
        query,
        depth: 2,
        limit: 500,
      });
      if (generation !== options.generationRef.current || request !== searchRequestRef.current) return;
      setSearchResult(result);
      options.setLens("all");
      options.setMainView("graph");
      options.setSelectedNodeId(result.nodes[0]?.id ?? null);
    } catch (error) {
      if (generation !== options.generationRef.current || request !== searchRequestRef.current) return;
      const message = error instanceof Error ? error.message : "Unable to search graph relationships";
      setSearchError(message);
      options.notify(message, "error");
    } finally {
      if (generation === options.generationRef.current && request === searchRequestRef.current) {
        setSearchLoading(false);
      }
    }
  }, [options]);

  return {
    overview, detail, flow, replace, clear,
    flowRootId, flowLoading, flowError,
    traceEndpoint,
    traceNode: traceRoot,
    loadNext,
    searchResult,
    searchQuery,
    searchLoading,
    searchError,
    search,
  };
}
