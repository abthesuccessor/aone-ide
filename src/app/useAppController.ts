import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ToastMessage, ToastTone } from "../components/Toast";
import {
  detectRunProfiles,
  getAiConfigurationStatus,
  getAppSnapshot,
  getWorkspaceFiles,
  isTauriRuntime,
  listRuntimeEvents,
  openWorkspace,
  openWorkspaceFile,
  pickAndLoadRunEnvFile,
  readWorkspaceFile,
  rescanWorkspace,
  sendApiRequest,
} from "../lib/bridge";
import { withRuntimeProjection } from "../lib/runtimeGraph";
import { failureMessage } from "../lib/errorMessage";
import { useWebSocketController } from "../features/realtime/useWebSocketController";
import type { CloneGithubRepositoryRequest, CreateDocumentsProjectRequest,
  OnboardingWorkspaceResult, WorkspaceActionOutcome } from "../features/onboarding/model";
import { runWorkspaceMutation } from "../features/onboarding/workspaceMutation";
import { cloneGithubRepository, createDocumentsProject } from "../lib/onboardingBridge";
import type { ApiRequest, ApiResponse, EnvLoadResult, GraphLens, GraphNode,
  RunProfile, RuntimeEvent, ScanProgress, SourceFile, SourceLocation,
  WorkspaceFile, WorkspaceSummary } from "../types";
import type { ActiveRun, AppPhase, MainView } from "./model";
import { loadGraphSnapshots } from "./graphQueries";
import { createBrowserHttpEvent } from "./runtimeEvents";
import { useAppShortcuts } from "./useAppShortcuts";
import { useAiExplanation } from "./useAiExplanation";
import { useRuntimeSubscriptions } from "./useRuntimeSubscriptions";
import { useWorkspaceGraphs } from "./useWorkspaceGraphs";
import { useExecutionDebuggerSession } from "../features/execution-debugger/useExecutionDebuggerSession";
import { useRunActions } from "./useRunActions";
import { useAiConfiguration } from "./useAiConfiguration";
import { useNewWorkspaceFile } from "./useNewWorkspaceFile";

function indexingCompleteMessage(summary: WorkspaceSummary): string { return `Indexing complete: ${summary.name} (${summary.fileCount} ${summary.fileCount === 1 ? "file" : "files"})`; }
export function useAppController() {
  const [phase, setPhase] = useState<AppPhase>("loading");
  const [appError, setAppError] = useState<string | null>(null);
  const [codeOnlyMode, setCodeOnlyMode] = useState(false);
  const [workspace, setWorkspace] = useState<WorkspaceSummary | null>(null);
  const [files, setFiles] = useState<WorkspaceFile[]>([]);
  const [filesLoading, setFilesLoading] = useState(false);
  const [graphLoading, setGraphLoading] = useState(false);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [runProfiles, setRunProfiles] = useState<RunProfile[]>([]);
  const [activeProfileId, setActiveProfileId] = useState("");
  const [activeRun, setActiveRun] = useState<ActiveRun | null>(null);
  const activeRunRef = useRef<ActiveRun | null>(null);
  activeRunRef.current = activeRun;
  const [runtimeEvents, setRuntimeEvents] = useState<RuntimeEvent[]>([]);
  const [runEnv, setRunEnv] = useState<EnvLoadResult | null>(null);
  const [envOpen, setEnvOpen] = useState(false);
  const [runEnvLoading, setRunEnvLoading] = useState(false);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [mainView, setMainView] = useState<MainView>("graph");
  const [lens, setLens] = useState<GraphLens>("system");
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [source, setSource] = useState<SourceFile | null>(null);
  const [sourceLocation, setSourceLocation] = useState<SourceLocation | null>(null);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [sourceError, setSourceError] = useState<string | null>(null);
  const [consoleCollapsed, setConsoleCollapsed] = useState(false);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const notify = useCallback((text: string, tone: ToastTone = "info") => setToast({ id: Date.now(), text, tone }), []);
  const [openingWorkspace, setOpeningWorkspace] = useState(false);
  const [scanningWorkspace, setScanningWorkspace] = useState(false);
  const [workspaceGeneration, setWorkspaceGeneration] = useState(0);
  const [catalogGeneration, setCatalogGeneration] = useState(0);
  const openingWorkspaceRef = useRef(false);
  const workspaceOperationRef = useRef(false);
  const aiConfigurationController = useAiConfiguration({
    operationBlockedRef: openingWorkspaceRef,
    notify,
  });
  const {
    configuration: aiConfiguration,
    setConfiguration: setAiConfiguration,
    error: aiConfigurationError,
    setError: setAiConfigurationError,
    env: aiEnv,
    loading: aiEnvLoading,
  } = aiConfigurationController;
  const workspaceGenerationRef = useRef(0);
  const sourceRequestGenerationRef = useRef(0);
  const initializingRef = useRef(false);
  const terminalRunIdsRef = useRef(new Set<string>());
  const {
    overview: systemGraph, detail: neighborhoodGraph, flow: executionFlowGraph,
    replace: replaceGraphs, clear: clearGraphs, flowRootId, flowLoading, flowError,
    traceEndpoint, traceNode, loadNext,
    searchResult: graphSearchResult,
    searchQuery: graphSearchQuery,
    searchLoading: graphSearchLoading,
    searchError: graphSearchError,
    search: searchGraph,
  } = useWorkspaceGraphs({ generationRef: workspaceGenerationRef,
    setLens, setMainView, setSelectedNodeId, notify });
  const webSocket = useWebSocketController();
  const broadGraph = neighborhoodGraph.nodes.length > 0 ? neighborhoodGraph : systemGraph;
  const activeGraph = lens === "architecture" ? systemGraph
    : lens === "runtime" ? executionFlowGraph : broadGraph;
  const displayGraph = useMemo(() => withRuntimeProjection(activeGraph, runtimeEvents),
    [activeGraph, runtimeEvents]);
  const selectedNode = useMemo(() =>
    displayGraph.nodes.find((node) => node.id === selectedNodeId) ?? null,
  [displayGraph.nodes, selectedNodeId]);
  const aiEvidenceGraph = graphSearchQuery ? graphSearchResult : displayGraph;
  const { explanation, explaining, explanationError, resetExplanation, handleExplain } =
    useAiExplanation({ selectedNodeId, workspaceId: workspace?.id ?? null,
      workspaceGenerationRef, graph: aiEvidenceGraph });
  const selectedSource = selectedNode?.source;
  const loadFilesAndGraph = useCallback(async () => {
    const generation = workspaceGenerationRef.current;
    setFilesLoading(true);
    setGraphLoading(true);
    setGraphError(null);
    try {
      const [nextFiles, [nextSystemGraph, nextNeighborhoodGraph, nextExecutionFlow], profiles] = await Promise.all([
        getWorkspaceFiles(undefined, 5_000),
        loadGraphSnapshots(),
        detectRunProfiles(),
      ]);
      if (generation !== workspaceGenerationRef.current) return;
      setFiles(nextFiles);
      replaceGraphs(nextSystemGraph, nextNeighborhoodGraph, nextExecutionFlow);
      setRunProfiles(profiles);
      setActiveProfileId((current) =>
        profiles.some((profile) => profile.id === current) ? current : (profiles[0]?.id ?? ""),
      );
      setSelectedNodeId((current) => current && (
        nextSystemGraph.nodes.some((node) => node.id === current)
        || nextNeighborhoodGraph.nodes.some((node) => node.id === current)
        || nextExecutionFlow.nodes.some((node) => node.id === current)
      ) ? current : null);
    } catch (error) {
      if (generation === workspaceGenerationRef.current) {
        setGraphError(error instanceof Error ? error.message : "Unable to query the graph");
      }
    } finally {
      if (generation === workspaceGenerationRef.current) {
        setFilesLoading(false);
        setGraphLoading(false);
        setCatalogGeneration((current) => current + 1);
      }
    }
  }, [replaceGraphs]);

  const initialize = useCallback(async () => {
    if (workspaceOperationRef.current || initializingRef.current) return;
    initializingRef.current = true;
    const generation = workspaceGenerationRef.current + 1;
    workspaceGenerationRef.current = generation;
    setPhase("loading");
    setAppError(null);
    try {
      const [snapshot, configuration] = await Promise.all([
        getAppSnapshot(),
        getAiConfigurationStatus(),
      ]);
      if (generation !== workspaceGenerationRef.current || workspaceOperationRef.current) return;
      sourceRequestGenerationRef.current += 1;
      resetExplanation();
      setWorkspaceGeneration((current) => current + 1);
      setAiConfiguration(configuration);
      setAiConfigurationError(null);
      setWorkspace(snapshot.workspace ?? null);
      if (snapshot.workspace) {
        const [nextFiles, [nextSystemGraph, nextNeighborhoodGraph, nextExecutionFlow], profiles, events] = await Promise.all([
          getWorkspaceFiles(undefined, 5_000),
          loadGraphSnapshots(),
          detectRunProfiles(),
          listRuntimeEvents(500),
        ]);
        if (generation !== workspaceGenerationRef.current || workspaceOperationRef.current) return;
        setFiles(nextFiles);
        replaceGraphs(nextSystemGraph, nextNeighborhoodGraph, nextExecutionFlow);
        setRunProfiles(profiles);
        setActiveProfileId(profiles[0]?.id ?? "");
        setRuntimeEvents(events);
        setSelectedNodeId(null);
        setCatalogGeneration((current) => current + 1);
      } else {
        setFiles([]);
        clearGraphs();
        setRunProfiles([]);
        setRuntimeEvents([]);
      }
      setPhase("ready");
    } catch (error) {
      if (generation !== workspaceGenerationRef.current || workspaceOperationRef.current) return;
      setAppError(error instanceof Error ? error.message : "Aone could not initialize");
      setPhase("error");
    } finally {
      initializingRef.current = false;
    }
  }, [clearGraphs, replaceGraphs, resetExplanation]);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const { cancelRuntimeEventFlush, cancelWorkspaceReload, eventSubscriptionReady } = useRuntimeSubscriptions({
    loadFilesAndGraph,
    notify,
    setScanProgress,
    setRuntimeEvents,
    setActiveRun,
    activeRunRef,
    scanOperationActiveRef: workspaceOperationRef,
    terminalRunIdsRef,
    appendWebSocketEvent: webSocket.appendWebSocketEvent,
  });
  const debuggerSession = useExecutionDebuggerSession({
    workspaceGeneration,
    events: runtimeEvents,
  });

  const openSource = useCallback(
    async (relativePath: string, node?: GraphNode, location?: SourceLocation, targetView: MainView = "source") => {
      if (openingWorkspaceRef.current) return;
      if (node) setSelectedNodeId(node.id);
      if (source?.relativePath === relativePath) {
        sourceRequestGenerationRef.current += 1;
        setSource({ ...source });
        setSourceLocation(location ?? node?.source ?? null);
        setSourceLoading(false);
        setSourceError(null);
        setMainView(targetView);
        return;
      }
      const workspaceGeneration = workspaceGenerationRef.current;
      const sourceGeneration = ++sourceRequestGenerationRef.current;
      const stale = () => workspaceGeneration !== workspaceGenerationRef.current || sourceGeneration !== sourceRequestGenerationRef.current;
      setSourceLoading(true);
      setSourceError(null);
      try {
        const nextSource = await readWorkspaceFile(relativePath);
        if (stale()) return;
        setSource(nextSource);
        setSourceLocation(location ?? node?.source ?? null);
        if (!node) {
          const matching = [...executionFlowGraph.nodes, ...systemGraph.nodes, ...neighborhoodGraph.nodes].find(
            (candidate) => candidate.source?.relativePath === relativePath,
          );
          if (matching) setSelectedNodeId(matching.id);
        }
        setMainView(targetView);
      } catch (error) {
        setSourceError(error instanceof Error ? error.message : "Unable to read source file");
        setMainView(targetView);
      } finally {
        if (!stale()) setSourceLoading(false);
      }
    },
    [executionFlowGraph.nodes, neighborhoodGraph.nodes, source, systemGraph.nodes],
  );

  const openSourceForNode = useCallback(
    (node: GraphNode) => {
      if (!node.source) {
        notify("This graph node has no source location", "warning");
        return;
      }
      void openSource(node.source.relativePath, node, node.source);
    },
    [notify, openSource],
  );

  const openSourceAt = useCallback((location: SourceLocation) => {
    void openSource(location.relativePath, undefined, location);
  }, [openSource]);
  const openSourceAtInGraph = useCallback((location: SourceLocation) => {
    void openSource(location.relativePath, undefined, location, "graph");
  }, [openSource]);
  const openSourceForNodeInGraph = useCallback((node: GraphNode) => {
    if (!node.source) {
      notify("This graph node has no source location", "warning");
      return;
    }
    void openSource(node.source.relativePath, node, node.source, "graph");
  }, [notify, openSource]);
  const focusSourceWithoutRange = useCallback(() => {
    sourceRequestGenerationRef.current += 1;
    setSourceLocation(null);
    setSourceLoading(false);
    setSourceError(null);
    setSelectedNodeId(null);
    setMainView("source");
  }, []);

  const adoptWorkspace = useCallback(async (result: { workspace: WorkspaceSummary }) => {
      const next = result.workspace;
      sourceRequestGenerationRef.current += 1;
      resetExplanation();
      setWorkspaceGeneration((current) => current + 1);
      cancelWorkspaceReload();
      cancelRuntimeEventFlush();
      setWorkspace(next);
      setFiles([]);
      clearGraphs();
      setGraphError(null);
      setRunProfiles([]);
      setActiveProfileId("");
      setActiveRun(null);
      debuggerSession.clear();
      terminalRunIdsRef.current.clear();
      setRuntimeEvents([]);
      setRunEnv(null);
      setSource(null);
      setSourceLocation(null);
      setSourceLoading(false);
      setSourceError(null);
      setSelectedNodeId(null);
      await loadFilesAndGraph();
      notify(indexingCompleteMessage(next), "success");
  }, [cancelRuntimeEventFlush, cancelWorkspaceReload, clearGraphs, debuggerSession.clear, loadFilesAndGraph, notify, resetExplanation]);

  const runWorkspaceAction = useCallback(<T extends { workspace: WorkspaceSummary }>(
    request: () => Promise<T | null>, fallback: string,
  ) => runWorkspaceMutation({
    phase, initializingRef, openingRef: openingWorkspaceRef,
    operationRef: workspaceOperationRef, generationRef: workspaceGenerationRef,
    setOpening: setOpeningWorkspace, clearProgress: () => setScanProgress(null),
    adopt: adoptWorkspace, notify,
  }, request, fallback), [adoptWorkspace, notify, phase]);

  const handleOpenFolder = useCallback(async () => {
    return runWorkspaceAction(async () => {
      const workspace = await openWorkspace();
      return workspace ? { workspace } : null;
    }, "Unable to open workspace");
  }, [runWorkspaceAction]);

  const handleOpenFile = useCallback(async () => {
    const outcome = await runWorkspaceAction(
      () => openWorkspaceFile(),
      "Unable to open file",
    );
    if (outcome.status === "success") {
      await openSource(outcome.result.relativePath);
    }
  }, [openSource, runWorkspaceAction]);
  const handleNewFile = useNewWorkspaceFile({
    workspace, openingWorkspaceRef, loadFilesAndGraph, notify,
    setSource, setSourceLocation, setSourceError, setSourceLoading, setMainView,
  });
  const handleCloneRepository = useCallback((request: CloneGithubRepositoryRequest): Promise<WorkspaceActionOutcome> =>
    runWorkspaceAction<OnboardingWorkspaceResult>(
      () => cloneGithubRepository(request), "Unable to clone repository",
    ), [runWorkspaceAction]);

  const handleCreateProject = useCallback((request: CreateDocumentsProjectRequest): Promise<WorkspaceActionOutcome> =>
    runWorkspaceAction<OnboardingWorkspaceResult>(
      () => createDocumentsProject(request), "Unable to create project",
    ), [runWorkspaceAction]);

  const handleScan = useCallback(async () => {
    if (!workspace || workspaceOperationRef.current) return;
    workspaceOperationRef.current = true;
    setScanningWorkspace(true);
    const timers: number[] = [];
    if (!isTauriRuntime()) {
      const updates: ScanProgress[] = [
        { phase: "Discover", completed: 9, total: 42, currentPath: "src/web/CheckoutPage.tsx" },
        {
          phase: "Parse",
          completed: 24,
          total: 42,
          currentPath: "src/api/services/CheckoutService.ts",
        },
        {
          phase: "Resolve",
          completed: 37,
          total: 42,
          currentPath: "migrations/004_create_orders.sql",
        },
        { phase: "committing", completed: 0, total: 0 },
      ];
      updates.forEach((progress, index) =>
        timers.push(window.setTimeout(() => setScanProgress(progress), index * 220)),
      );
    } else {
      setScanProgress({
        phase: "Starting",
        completed: 0,
        total: Math.max(workspace.fileCount, 1),
      });
    }

    try {
      const summary = await rescanWorkspace();
      setWorkspace(summary);
      await loadFilesAndGraph();
      notify(indexingCompleteMessage(summary), "success");
    } catch (error) {
      notify(failureMessage(error, "Workspace scan failed"), "error");
    } finally {
      timers.forEach((timer) => window.clearTimeout(timer));
      workspaceOperationRef.current = false;
      setScanningWorkspace(false);
      setScanProgress(null);
    }
  }, [loadFilesAndGraph, notify, workspace]);

  const handlePickRunEnv = useCallback(async () => {
    if (openingWorkspaceRef.current) return;
    setRunEnvLoading(true);
    try {
      const next = await pickAndLoadRunEnvFile();
      if (next) {
        setRunEnv(next);
        notify(`Loaded ${next.names.length} run key names`, "success");
      }
    } catch (error) {
      notify(error instanceof Error ? error.message : "Could not load run environment keys", "error");
    } finally {
      setRunEnvLoading(false);
    }
  }, [notify]);

  const { handleStart, handleStop, handleDebugControl } = useRunActions({
    activeRun, activeProfileId, runProfiles, runEnv,
    openingWorkspaceRef, terminalRunIdsRef, debuggerSession,
    setActiveRun, setConsoleCollapsed, setEnvOpen, setMainView, setRuntimeEvents, notify,
  });

  const handleApiRequest = useCallback(
    async (request: ApiRequest): Promise<ApiResponse> => {
      const response = await sendApiRequest(request);
      if (!isTauriRuntime()) {
        const observedEvent = createBrowserHttpEvent(request, response);
        setRuntimeEvents((current) => [...current, observedEvent].slice(-500));
      }
      notify(
        `${request.method} completed with ${response.status} in ${response.durationMs} ms`,
        response.status < 400 ? "success" : "warning",
      );
      return response;
    },
    [notify],
  );
  const refreshRuntimeEvents = () => listRuntimeEvents().then(setRuntimeEvents);

  useAppShortcuts({
    activeRun,
    selectedNode,
    handleStop,
    openSourceForNode,
    setMainView,
    setEnvOpen,
  });

  return {
    phase, appError,
    workspace, openingWorkspace, scanningWorkspace, workspaceGeneration, catalogGeneration,
    files, filesLoading,
    graph: activeGraph, executionFlowGraph, flowRootId, graphLoading, flowLoading,
    graphError: graphError ?? graphSearchError ?? flowError,
    graphSearchResult, graphSearchQuery, graphSearchLoading, searchGraph,
    runProfiles, activeProfileId, setActiveProfileId, activeRun,
    runtimeEvents, debuggerSession,
    aiConfiguration, aiConfigurationError, codeOnlyMode, setCodeOnlyMode,
    aiEnv, runEnv, envOpen, setEnvOpen, aiEnvLoading, runEnvLoading,
    scanProgress,
    mainView, setMainView, lens, setLens,
    selectedNodeId, setSelectedNodeId,
    source, sourceLocation, sourceLoading, sourceError,
    consoleCollapsed, setConsoleCollapsed,
    explanation, explaining, explanationError,
    toast, setToast,
    displayGraph, selectedNode, selectedSource,
    notify, initialize,
    openSource, openSourceForNode, openSourceAt, openSourceAtInGraph,
    openSourceForNodeInGraph, focusSourceWithoutRange,
    handleOpenFile, handleNewFile, handleOpenFolder, handleCloneRepository, handleCreateProject, handleScan,
    handlePickAiEnv: aiConfigurationController.chooseEnv,
    configureHostedAi: aiConfigurationController.configureHosted,
    configureOllama: aiConfigurationController.configureLocal,
    cliAdapters: aiConfigurationController.cliAdapters,
    refreshCliAdapters: aiConfigurationController.refreshCliAdapters,
    configureCli: aiConfigurationController.configureCli,
    aiConfigurationAction: aiConfigurationController.action,
    handlePickRunEnv,
    handleStart, handleStop, handleDebugControl, handleApiRequest, handleExplain,
    refreshRuntimeEvents,
    traceEndpoint, traceNode, loadNextExecutionFlow: loadNext,
    webSocketEvents: webSocket.webSocketEvents,
    webSocketEventSubscriptionReady: eventSubscriptionReady,
    handleWebSocketConnect: webSocket.handleWebSocketConnect,
    handleWebSocketSend: webSocket.handleWebSocketSend,
    handleWebSocketDisconnect: webSocket.handleWebSocketDisconnect,
  };
}
