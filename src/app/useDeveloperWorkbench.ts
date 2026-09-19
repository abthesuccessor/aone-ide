import { useCallback, useEffect, useMemo, useState } from "react";
import { useEditorController } from "../features/editor/useEditorController";
import type { EditorOpenMode } from "../features/editor/model";
import { useWorkbenchSettings } from "../features/editor/settings";
import type {
  CloneGithubRepositoryRequest,
  CreateDocumentsProjectRequest,
  WorkspaceActionOutcome,
} from "../features/onboarding/model";
import type { GraphNode, SourceLocation } from "../types";
import type { GitDiffResult, GitStatusResult } from "../features/source-control/model";
import { relationshipGraphForView, type RelationshipView } from "../features/graph/relationshipView";
import type { useAppController } from "./useAppController";
import { stopMenuSubscription, subscribeToMenuActions } from "../lib/menuBridge";
import { useApplicationZoom } from "./useApplicationZoom";
import { relationshipNodeForGitPath } from "../features/source-control/relationshipSelection";
import type { ApiEndpointInventoryItem } from "../features/api-catalog/model";

export type ActivityView = "explorer" | "search" | "source-control" | "graph" | "debugger" | "agent-setup" | "tools";

export function useDeveloperWorkbench(core: ReturnType<typeof useAppController>) {
  const applicationZoom = useApplicationZoom();
  const [activityView, setActivityView] = useState<ActivityView>("explorer");
  const [primarySidebarVisible, setPrimarySidebarVisible] = useState(true);
  const [secondarySidebarVisible, setSecondarySidebarVisible] = useState(true);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [gitStatus, setGitStatus] = useState<GitStatusResult | null>(null);
  const [gitDiff, setGitDiff] = useState<GitDiffResult | null>(null);
  const [selectedGitPath, setSelectedGitPath] = useState<string | null>(null);
  const [selectedGitNodeId, setSelectedGitNodeId] = useState<string | null>(null);
  const [relationshipView, setRelationshipView] = useState<RelationshipView>("workspace");
  const [apiTraceSelection, setApiTraceSelection] = useState<{
    endpoint: ApiEndpointInventoryItem;
    workspaceId?: string;
    workspaceGeneration: number;
  } | null>(null);
  const apiTraceTarget = apiTraceSelection
    && apiTraceSelection.workspaceId === core.workspace?.id
    && apiTraceSelection.workspaceGeneration === core.workspaceGeneration
    ? apiTraceSelection.endpoint
    : null;
  const preferences = useWorkbenchSettings();
  const editor = useEditorController({
    source: core.source,
    workspaceId: core.workspace?.id,
    workspaceGeneration: core.workspaceGeneration,
    openingWorkspace: core.openingWorkspace,
    settings: preferences.settings,
    notify: core.notify,
  });
  useEffect(() => {
    if (core.activeRun?.mode !== "debug") return;
    setPrimarySidebarVisible(true);
    setActivityView("debugger");
  }, [core.activeRun?.id, core.activeRun?.mode]);
  const changedPaths = useMemo(
    () => new Set(gitStatus?.files.map((file) => file.relativePath) ?? []),
    [gitStatus],
  );
  const changedFiles = useMemo(() => gitStatus?.files ?? [], [gitStatus]);
  const focusedChangedPaths = useMemo(
    () => selectedGitPath && changedPaths.has(selectedGitPath)
      ? new Set([selectedGitPath])
      : changedPaths,
    [changedPaths, selectedGitPath],
  );
  const gitRelationshipGraph = useMemo(() => relationshipGraphForView(
    core.displayGraph,
    core.executionFlowGraph,
    "changes",
    focusedChangedPaths,
    changedFiles,
    selectedGitNodeId,
  ), [changedFiles, core.displayGraph, core.executionFlowGraph, focusedChangedPaths, selectedGitNodeId]);
  const relationshipGraph = useMemo(() => {
    if (core.graphSearchQuery) return core.graphSearchResult;
    return relationshipGraphForView(
      core.displayGraph,
      core.executionFlowGraph,
      relationshipView,
      focusedChangedPaths,
      changedFiles,
      selectedGitNodeId,
    );
  }, [
    changedFiles,
    core.displayGraph,
    core.executionFlowGraph,
    core.graphSearchQuery,
    core.graphSearchResult,
    focusedChangedPaths,
    relationshipView,
    selectedGitNodeId,
  ]);
  const selectedNode = useMemo(
    () => relationshipGraph.nodes.find((node) => node.id === core.selectedNodeId) ?? core.selectedNode,
    [core.selectedNode, core.selectedNodeId, relationshipGraph.nodes],
  );
  const focusQuickFile = useCallback(() => {
    setPrimarySidebarVisible(true);
    setActivityView("explorer");
    core.setMainView("source");
    window.requestAnimationFrame(() => {
      document.getElementById("workspace-search")?.focus();
    });
  }, [core.setMainView]);
  useEffect(() => {
    setGitStatus(null);
    setGitDiff(null);
    setSelectedGitPath(null);
    setSelectedGitNodeId(null);
    setRelationshipView("workspace");
    setApiTraceSelection(null);
  }, [core.workspaceGeneration]);
  useEffect(() => {
    if (selectedGitPath && !changedPaths.has(selectedGitPath)) {
      setSelectedGitPath(null);
      setSelectedGitNodeId(null);
    }
  }, [changedPaths, selectedGitPath]);
  useEffect(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.shiftKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        setPrimarySidebarVisible(true);
        setActivityView("search");
        window.requestAnimationFrame(() => {
          document.getElementById("workspace-content-search")?.focus();
        });
      } else if (!event.shiftKey && event.key.toLowerCase() === "p") {
        event.preventDefault();
        focusQuickFile();
      } else if (!event.shiftKey && event.key.toLowerCase() === "b") {
        event.preventDefault();
        setPrimarySidebarVisible((visible) => !visible);
      } else if (event.key === "1" || event.key === "2" || event.key === "3") {
        setPrimarySidebarVisible(true);
        setActivityView("explorer");
      }
    };
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, [focusQuickFile]);
  const canChangeWorkspace = useCallback(() => {
    if (core.aiConfiguration?.inferenceAvailable !== true && !core.codeOnlyMode) {
      core.notify("Choose an AI provider or continue in code-only mode before opening a workspace", "warning");
      return false;
    }
    return !core.openingWorkspace && editor.confirmCanLeaveWorkspace();
  }, [core.aiConfiguration?.inferenceAvailable, core.codeOnlyMode, core.notify, core.openingWorkspace, editor.confirmCanLeaveWorkspace]);
  const openProjectAgent = useCallback(() => {
    setPrimarySidebarVisible(true);
    setActivityView("agent-setup");
    core.setMainView("graph");
  }, [core.setMainView]);
  const handleOpenFolder = useCallback(async () => {
    if (!canChangeWorkspace()) return { status: "cancelled" } as const;
    const outcome = await core.handleOpenFolder();
    if (outcome?.status === "success") openProjectAgent();
    return outcome;
  }, [canChangeWorkspace, core.handleOpenFolder, openProjectAgent]);
  useEffect(() => {
    let mounted = true;
    let unsubscribe: () => void = () => undefined;
    void subscribeToMenuActions((action) => {
      if (!mounted) return;
      if (action === "openFolder") void handleOpenFolder();
      else if (action === "newFile") void core.handleNewFile();
    }).then((stop) => {
      if (mounted) unsubscribe = stop;
      else stopMenuSubscription(stop);
    }).catch((error: unknown) => {
      if (mounted) core.notify(error instanceof Error ? error.message : "Native File menu failed", "error");
    });
    return () => {
      mounted = false;
      stopMenuSubscription(unsubscribe);
    };
  }, [core.handleNewFile, core.notify, handleOpenFolder]);
  const handleOnboardingOpenFolder = useCallback(async () => {
    await handleOpenFolder();
  }, [handleOpenFolder]);
  const handleCloneRepository = useCallback(async (
    request: CloneGithubRepositoryRequest,
  ): Promise<WorkspaceActionOutcome> => {
    if (!canChangeWorkspace()) return { status: "cancelled" };
    const outcome = await core.handleCloneRepository(request);
    if (outcome.status === "success") openProjectAgent();
    return outcome;
  }, [canChangeWorkspace, core.handleCloneRepository, openProjectAgent]);
  const handleCreateProject = useCallback(async (
    request: CreateDocumentsProjectRequest,
  ): Promise<WorkspaceActionOutcome> => {
    if (!canChangeWorkspace()) return { status: "cancelled" };
    const outcome = await core.handleCreateProject(request);
    if (outcome.status === "success") openProjectAgent();
    return outcome;
  }, [canChangeWorkspace, core.handleCreateProject, openProjectAgent]);
  const openSourceLocation = useCallback((location: SourceLocation, mode: EditorOpenMode = "preview") => {
    if (mode === "pinned") editor.pinDocument(location.relativePath);
    core.openSourceAt(location);
  }, [core.openSourceAt, editor.pinDocument]);
  const openSourceLocationInGraph = useCallback((location: SourceLocation, mode: EditorOpenMode = "preview") => {
    if (mode === "pinned") editor.pinDocument(location.relativePath);
    core.openSourceAtInGraph(location);
  }, [core.openSourceAtInGraph, editor.pinDocument]);
  const openSourceLocationInDebugger = useCallback((location: SourceLocation, mode: EditorOpenMode = "preview") => {
    if (mode === "pinned") editor.pinDocument(location.relativePath);
    void core.openSource(location.relativePath, undefined, location, "debugger");
  }, [core.openSource, editor.pinDocument]);
  const openApiTrace = useCallback((endpoint: ApiEndpointInventoryItem, mode: EditorOpenMode = "preview") => {
    const location = endpoint.handler?.source ?? endpoint.source;
    if (mode === "pinned" && location) editor.pinDocument(location.relativePath);
    setApiTraceSelection({
      endpoint,
      workspaceId: core.workspace?.id,
      workspaceGeneration: core.workspaceGeneration,
    });
    setActivityView("debugger");
    setPrimarySidebarVisible(true);
    setRelationshipView("codepath");
    core.setMainView("debugger");
    if (location) void core.openSource(location.relativePath, undefined, location, "debugger");
    void core.traceEndpoint(endpoint, "debugger");
  }, [core.openSource, core.setMainView, core.traceEndpoint, core.workspace?.id, core.workspaceGeneration, editor.pinDocument]);
  const openSearchMatch = useCallback((location: SourceLocation, mode: EditorOpenMode = "preview") => {
    const dirtyDocument = editor.documents.find(
      (document) => document.relativePath === location.relativePath && document.dirty,
    );
    if (!dirtyDocument) {
      openSourceLocation(location, mode);
      return;
    }
    editor.setActivePath(dirtyDocument.relativePath);
    core.focusSourceWithoutRange();
    core.notify(
      "Search result is from the saved version. Exact selection skipped for the unsaved file.",
      "warning",
    );
  }, [core.focusSourceWithoutRange, core.notify, editor.documents, editor.setActivePath, openSourceLocation]);
  const openGitChangedFile = useCallback((relativePath: string, mode: EditorOpenMode = "preview") => {
    setSelectedGitPath(relativePath);
    setSelectedGitNodeId(null);
    setRelationshipView("changes");
    core.setMainView("graph");
    const node = relationshipNodeForGitPath(gitRelationshipGraph.nodes, relativePath);
    if (!node) core.setSelectedNodeId(null);
    if (mode === "pinned") editor.pinDocument(relativePath);
    void core.openSource(relativePath, node ?? undefined, node?.source, "graph");
  }, [core.openSource, core.setMainView, core.setSelectedNodeId, editor.pinDocument, gitRelationshipGraph.nodes]);
  const openGitDiff = useCallback((diff: GitDiffResult | null) => {
    setGitDiff(diff);
    if (!diff) return;
    setSelectedGitPath(diff.relativePath);
    setRelationshipView("changes");
    core.setMainView("graph");
    const node = relationshipNodeForGitPath(
      gitRelationshipGraph.nodes,
      diff.relativePath,
      diff.content,
    );
    setSelectedGitNodeId(node?.metadata.gitChangeMarker === true ? null : (node?.id ?? null));
    core.setSelectedNodeId(node?.id ?? null);
  }, [core.setMainView, core.setSelectedNodeId, gitRelationshipGraph.nodes]);
  const openRelationshipNode = useCallback((node: GraphNode, mode: EditorOpenMode = "preview") => {
    core.setSelectedNodeId(node.id);
    setGitDiff(null);
    if (!node.source) return;
    if (changedPaths.has(node.source.relativePath)) {
      setSelectedGitPath(node.source.relativePath);
      if (node.metadata.gitChanged === true && node.metadata.gitChangeMarker !== true) {
        setSelectedGitNodeId(node.id);
      }
    }
    if (mode === "pinned") editor.pinDocument(node.source.relativePath);
    void core.openSource(node.source.relativePath, node, node.source, "graph");
  }, [changedPaths, core.openSource, core.setSelectedNodeId, editor.pinDocument]);
  const selectRelationshipNode = useCallback((nodeId: string) => {
    const node = relationshipGraph.nodes.find((candidate) => candidate.id === nodeId);
    if (node) openRelationshipNode(node);
    else core.setSelectedNodeId(nodeId);
  }, [core.setSelectedNodeId, openRelationshipNode, relationshipGraph.nodes]);

  return {
    activityView,
    setActivityView,
    primarySidebarVisible,
    setPrimarySidebarVisible,
    secondarySidebarVisible,
    setSecondarySidebarVisible,
    focusQuickFile,
    settingsOpen,
    setSettingsOpen,
    gitStatus,
    setGitStatus,
    gitDiff,
    setGitDiff,
    selectedGitPath,
    openGitChangedFile,
    openGitDiff,
    openSourceLocation,
    openSourceLocationInGraph,
    openSourceLocationInDebugger,
    apiTraceTarget,
    openApiTrace,
    openRelationshipNode,
    selectRelationshipNode,
    relationshipView,
    setRelationshipView,
    relationshipGraph,
    selectedNode,
    changedPaths,
    applicationZoom,
    ...preferences,
    editor,
    handleOpenFolder,
    handleOnboardingOpenFolder,
    handleCloneRepository,
    handleCreateProject,
    openSearchMatch,
  };
}
