import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AiExplanation, ScanProgress, SourceFile, WorkspaceSummary } from "../types";
import { useAppController } from "./useAppController";
import { useDeveloperWorkbench } from "./useDeveloperWorkbench";

const bridge = vi.hoisted(() => ({
  detectRunProfiles: vi.fn(), explainNode: vi.fn(),
  getAiConfigurationStatus: vi.fn(),
  getAppSnapshot: vi.fn(), getWorkspaceFiles: vi.fn(),
  listRuntimeEvents: vi.fn(), openWorkspace: vi.fn(), openWorkspaceFile: vi.fn(), queryGraph: vi.fn(),
  readWorkspaceFile: vi.fn(), rescanWorkspace: vi.fn(),
}));
const editorBridge = vi.hoisted(() => ({
  formatDocument: vi.fn(),
  getFormatterCapabilities: vi.fn(),
  writeWorkspaceFile: vi.fn(),
}));
const onboardingBridge = vi.hoisted(() => ({
  cloneGithubRepository: vi.fn(),
  createDocumentsProject: vi.fn(),
}));
const subscriptions = vi.hoisted(() => ({
  setScanProgress: undefined as ((value: ScanProgress | null) => void) | undefined,
  loadFilesAndGraph: undefined as (() => Promise<void>) | undefined,
}));

vi.mock("../lib/bridge", () => ({
  detectRunProfiles: bridge.detectRunProfiles,
  explainNode: bridge.explainNode,
  getAiConfigurationStatus: bridge.getAiConfigurationStatus,
  getAppSnapshot: bridge.getAppSnapshot,
  getWorkspaceFiles: bridge.getWorkspaceFiles,
  isTauriRuntime: () => false,
  listRuntimeEvents: bridge.listRuntimeEvents,
  openWorkspace: bridge.openWorkspace,
  openWorkspaceFile: bridge.openWorkspaceFile,
  pickAndLoadEnvFile: vi.fn(),
  pickAndLoadRunEnvFile: vi.fn(),
  queryGraph: bridge.queryGraph,
  readWorkspaceFile: bridge.readWorkspaceFile,
  rescanWorkspace: bridge.rescanWorkspace,
  sendApiRequest: vi.fn(),
  startRun: vi.fn(),
  stopRun: vi.fn(),
}));

vi.mock("../lib/editorBridge", () => ({
  formatDocument: editorBridge.formatDocument,
  getFormatterCapabilities: editorBridge.getFormatterCapabilities,
  writeWorkspaceFile: editorBridge.writeWorkspaceFile,
}));

vi.mock("../lib/onboardingBridge", () => ({
  cloneGithubRepository: onboardingBridge.cloneGithubRepository,
  createDocumentsProject: onboardingBridge.createDocumentsProject,
}));

vi.mock("./useRuntimeSubscriptions", () => ({
  useRuntimeSubscriptions: (options: {
    setScanProgress: (value: ScanProgress | null) => void;
    loadFilesAndGraph: () => Promise<void>;
  }) => {
    subscriptions.setScanProgress = options.setScanProgress;
    subscriptions.loadFilesAndGraph = options.loadFilesAndGraph;
    return {
      cancelRuntimeEventFlush: vi.fn(),
      cancelWorkspaceReload: vi.fn(),
      eventSubscriptionReady: true,
    };
  },
}));

vi.mock("../features/realtime/useWebSocketController", () => ({
  useWebSocketController: () => ({
    webSocketEvents: [],
    appendWebSocketEvent: vi.fn(),
    handleWebSocketConnect: vi.fn(),
    handleWebSocketSend: vi.fn(),
    handleWebSocketDisconnect: vi.fn(),
  }),
}));

vi.mock("./useAppShortcuts", () => ({ useAppShortcuts: vi.fn() }));

function workspace(id: string): WorkspaceSummary {
  return {
    id,
    name: id,
    rootPath: `/tmp/${id}`,
    fileCount: 1,
    nodeCount: 0,
    edgeCount: 0,
    languages: [],
    lastScannedAt: "2026-08-17T00:00:00Z",
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((next, fail) => {
    resolve = next;
    reject = fail;
  });
  return { promise, resolve, reject };
}

function useHarness() {
  const core = useAppController();
  return { ...core, ...useDeveloperWorkbench(core) };
}

beforeEach(() => {
  vi.clearAllMocks();
  subscriptions.setScanProgress = undefined;
  subscriptions.loadFilesAndGraph = undefined;
  window.localStorage.clear();
  bridge.getAiConfigurationStatus.mockResolvedValue({
    configured: true, provider: "openai", model: "test-model", transport: "api", inferenceAvailable: true,
  });
  bridge.getAppSnapshot.mockResolvedValue({ workspace: workspace("workspace-a") });
  bridge.getWorkspaceFiles.mockResolvedValue([]);
  bridge.queryGraph.mockResolvedValue({ nodes: [], edges: [], truncated: false });
  bridge.detectRunProfiles.mockResolvedValue([]);
  bridge.listRuntimeEvents.mockResolvedValue([]);
  bridge.openWorkspaceFile.mockResolvedValue(null);
  bridge.rescanWorkspace.mockResolvedValue(workspace("workspace-a"));
  editorBridge.getFormatterCapabilities.mockResolvedValue([]);
  onboardingBridge.cloneGithubRepository.mockResolvedValue({
    action: "clone", workspace: workspace("workspace-clone"), destinationHint: "~/Documents/workspace-clone",
  });
  onboardingBridge.createDocumentsProject.mockResolvedValue({
    action: "create", workspace: workspace("workspace-create"), destinationHint: "~/Documents/workspace-create",
  });
});

afterEach(() => vi.restoreAllMocks());

describe("post-consent workspace opening", () => {
  it("opens a native-selected file after adopting its containing workspace", async () => {
    const selected: SourceFile = {
      relativePath: "src/selected.ts",
      language: "TypeScript",
      content: "export const selected = true;",
      contentHash: "selected-hash",
    };
    bridge.openWorkspaceFile.mockResolvedValueOnce({
      workspace: workspace("workspace-file"),
      relativePath: selected.relativePath,
    });
    bridge.readWorkspaceFile.mockResolvedValueOnce(selected);
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    await act(async () => result.current.handleOpenFile());
    expect(bridge.openWorkspaceFile).toHaveBeenCalledTimes(1);
    expect(result.current.workspace?.id).toBe("workspace-file");
    expect(result.current.source?.relativePath).toBe(selected.relativePath);
    expect(result.current.mainView).toBe("source");
  });

  it("blocks Open Folder until a pending startup result is safely committed", async () => {
    const startup = deferred<{ workspace: WorkspaceSummary }>();
    bridge.getAppSnapshot.mockReturnValueOnce(startup.promise);
    const { result } = renderHook(() => useHarness());
    expect(result.current.phase).toBe("loading");
    await act(async () => result.current.handleOpenFolder());
    expect(bridge.openWorkspace).not.toHaveBeenCalled();
    await act(async () => startup.resolve({ workspace: workspace("startup-workspace") }));
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    expect(result.current.workspace?.id).toBe("startup-workspace");
  });

  it("loads the bounded structured overview without preselecting a node", async () => {
    bridge.queryGraph.mockImplementation((query: { projection?: string }) => Promise.resolve(
      query.projection === "systemOverview"
        ? {
          nodes: [{ id: "node-1", kind: "function", label: "main", evidence: "declared", metadata: {} }],
          edges: [],
          truncated: false,
        }
        : { nodes: [], edges: [], truncated: false },
    ));
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    expect(bridge.queryGraph).toHaveBeenCalledWith({ projection: "systemOverview", depth: 2, limit: 60 });
    expect(bridge.queryGraph).toHaveBeenCalledWith({
      projection: "neighborhood",
      rootIds: ["node-1"],
      depth: 4,
      limit: 500,
    });
    expect(bridge.queryGraph).toHaveBeenCalledWith(expect.objectContaining(
      { projection: "executionFlow", rootIds: ["node-1"], depth: 4, limit: 500 },
    ));
    expect(result.current.lens).toBe("system");
    expect(result.current.selectedNodeId).toBeNull();
  });
  it("keeps the neighborhood snapshot available for dependency lenses", async () => {
    bridge.queryGraph.mockImplementation((query: { projection?: string }) => Promise.resolve(
      query.projection === "systemOverview"
        ? {
          nodes: [{ id: "overview", kind: "service", label: "Overview", evidence: "declared", metadata: { systemLayer: "application" } }],
          edges: [],
          truncated: false,
        }
        : query.projection === "executionFlow"
          ? {
            nodes: [{ id: "flow-root", kind: "function", label: "flowRoot", evidence: "resolved", metadata: { flowRoot: true } }],
            edges: [],
            truncated: false,
          }
        : {
          nodes: [
            { id: "caller", kind: "function", label: "caller", evidence: "declared", metadata: {} },
            { id: "callee", kind: "function", label: "callee", evidence: "resolved", metadata: {} },
          ],
          edges: [{ id: "call", source: "caller", target: "callee", kind: "calls", evidence: "resolved", metadata: {} }],
          truncated: false,
        },
    ));
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    expect(result.current.displayGraph.nodes.map((node) => node.id)).toEqual(["caller", "callee"]);
    act(() => result.current.setLens("architecture"));
    expect(result.current.displayGraph.nodes.map((node) => node.id)).toEqual(["overview"]);
    act(() => result.current.setLens("dependencies"));
    expect(result.current.displayGraph.edges.map((edge) => edge.kind)).toEqual(["calls"]);
    expect(result.current.displayGraph.nodes.map((node) => node.id)).toEqual(["caller", "callee"]);
    act(() => result.current.setLens("runtime"));
    expect(result.current.displayGraph.nodes.map((node) => node.id)).toEqual(["flow-root"]);
  });
  it("refuses folder opening while a workspace rescan is active", async () => {
    const rescan = deferred<WorkspaceSummary>();
    bridge.rescanWorkspace.mockReturnValueOnce(rescan.promise);
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    let scanning: Promise<void> = Promise.resolve();
    act(() => { scanning = result.current.handleScan(); });
    await waitFor(() => expect(result.current.scanningWorkspace).toBe(true));
    await act(async () => result.current.handleOpenFolder());
    expect(bridge.openWorkspace).not.toHaveBeenCalled();
    await act(async () => {
      rescan.resolve(workspace("workspace-a"));
      await scanning;
    });
    expect(result.current.scanningWorkspace).toBe(false);
  });

  it("refreshes the API catalog epoch after rescans and watcher reloads", async () => {
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    const initial = result.current.catalogGeneration;
    await act(async () => result.current.handleScan());
    expect(result.current.catalogGeneration).toBeGreaterThan(initial);
    const afterScan = result.current.catalogGeneration;
    await act(async () => subscriptions.loadFilesAndGraph?.());
    expect(result.current.catalogGeneration).toBeGreaterThan(afterScan);
  });

  it("locks mutations while pending, restores them on cancel, and safely clears on resolve", async () => {
    const cancelOpen = deferred<WorkspaceSummary | null>();
    const resolveOpen = deferred<WorkspaceSummary | null>();
    bridge.openWorkspace
      .mockImplementationOnce(() => cancelOpen.promise)
      .mockImplementationOnce(() => resolveOpen.promise);
    const selected: SourceFile = {
      relativePath: "src/current.ts",
      language: "TypeScript",
      content: "saved",
      contentHash: "hash:a",
    };
    bridge.readWorkspaceFile.mockResolvedValue(selected);
    const pendingSave = deferred<SourceFile>();
    editorBridge.writeWorkspaceFile.mockImplementationOnce(() => pendingSave.promise);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    await act(async () => result.current.openSource(selected.relativePath));
    await waitFor(() => expect(result.current.editor.activeDocument?.content).toBe("saved"));
    act(() => result.current.editor.updateContent("dirty before consent"));

    let saving: Promise<unknown> = Promise.resolve();
    act(() => { saving = result.current.editor.saveDocument(); });
    await act(async () => result.current.handleOpenFolder());
    expect(bridge.openWorkspace).not.toHaveBeenCalled();
    expect(confirm).not.toHaveBeenCalled();
    expect(result.current.openingWorkspace).toBe(false);
    await act(async () => {
      pendingSave.resolve({
        ...selected,
        content: "dirty before consent",
        contentHash: "hash:saved-before-picker",
      });
      await saving;
    });
    expect(result.current.editor.activeDocument).toMatchObject({
      contentHash: "hash:saved-before-picker",
      dirty: false,
    });
    act(() => result.current.editor.updateContent("dirty before cancel consent"));

    let cancelledOpening: Promise<unknown> = Promise.resolve();
    act(() => { cancelledOpening = result.current.handleOpenFolder(); });
    await waitFor(() => expect(result.current.openingWorkspace).toBe(true));
    act(() => subscriptions.setScanProgress?.({
      phase: "Complete",
      completed: 1,
      total: 1,
    }));
    expect(result.current.scanProgress?.phase).toBe("Complete");
    expect(confirm.mock.invocationCallOrder[0]!).toBeLessThan(bridge.openWorkspace.mock.invocationCallOrder[0]!);
    act(() => result.current.editor.updateContent("post-consent edit"));
    expect(result.current.editor.closeDocument(selected.relativePath)).toBe(false);
    expect(result.current.editor.activeDocument).toMatchObject({
      content: "dirty before cancel consent",
      dirty: true,
    });

    await act(async () => {
      cancelOpen.resolve(null);
      await cancelledOpening;
    });
    expect(result.current.openingWorkspace).toBe(false);
    expect(result.current.scanProgress).toBeNull();
    expect(result.current.workspace?.id).toBe("workspace-a");
    act(() => result.current.editor.updateContent("edit after cancel"));
    expect(result.current.editor.activeDocument?.content).toBe("edit after cancel");

    let resolvedOpening: Promise<unknown> = Promise.resolve();
    act(() => { resolvedOpening = result.current.handleOpenFolder(); });
    await waitFor(() => expect(result.current.openingWorkspace).toBe(true));
    act(() => result.current.editor.updateContent("discarded post-consent edit"));
    expect(result.current.editor.activeDocument?.content).toBe("edit after cancel");
    await act(async () => {
      resolveOpen.resolve(workspace("workspace-b"));
      await resolvedOpening;
    });

    expect(result.current.openingWorkspace).toBe(false);
    expect(result.current.scanProgress).toBeNull();
    expect(result.current.workspace?.id).toBe("workspace-b");
    expect(result.current.toast).toMatchObject({
      text: "Indexing complete: workspace-b (1 file)",
      tone: "success",
    });
    expect(result.current.editor.documents).toEqual([]);
  });

  it("adopts clone and create results through the shared workspace reset pipeline", async () => {
    const { result } = renderHook(() => useHarness());
    await waitFor(() => expect(result.current.phase).toBe("ready"));

    await act(async () => result.current.handleCloneRepository({
      repositoryUrl: "https://github.com/example/workspace-clone",
      destinationName: "workspace-clone",
    }));
    expect(onboardingBridge.cloneGithubRepository).toHaveBeenCalledWith({
      repositoryUrl: "https://github.com/example/workspace-clone",
      destinationName: "workspace-clone",
    });
    expect(result.current.workspace?.id).toBe("workspace-clone");
    expect(result.current.activityView).toBe("agent-setup");

    await act(async () => result.current.handleCreateProject({
      projectName: "workspace-create",
      initializeGit: true,
    }));
    expect(onboardingBridge.createDocumentsProject).toHaveBeenCalledWith({
      projectName: "workspace-create",
      initializeGit: true,
    });
    expect(result.current.workspace?.id).toBe("workspace-create");
    expect(result.current.activityView).toBe("agent-setup");
    expect(result.current.runEnv).toBeNull();
    expect(result.current.editor.documents).toEqual([]);
  });

  it("clears terminal indexing progress when workspace opening fails", async () => {
    const failedOpen = deferred<WorkspaceSummary | null>();
    bridge.openWorkspace.mockImplementationOnce(() => failedOpen.promise);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    let opening: Promise<unknown> = Promise.resolve();
    act(() => { opening = result.current.handleOpenFolder(); });
    await waitFor(() => expect(result.current.openingWorkspace).toBe(true));
    act(() => subscriptions.setScanProgress?.({
      phase: "Complete",
      completed: 1,
      total: 1,
    }));
    expect(result.current.scanProgress?.phase).toBe("Complete");
    await act(async () => {
      failedOpen.reject(new Error("Unable to index selected folder"));
      await opening;
    });
    expect(result.current.openingWorkspace).toBe(false);
    expect(result.current.scanProgress).toBeNull();
    expect(result.current.toast).toMatchObject({
      text: "Unable to index selected folder",
      tone: "error",
    });
  });

  it("surfaces a native string rejection, preserves the workspace, and permits a retry", async () => {
    bridge.openWorkspace
      .mockRejectedValueOnce("database error: UNIQUE constraint failed: graph_nodes.id")
      .mockResolvedValueOnce(workspace("workspace-b"));
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    await act(async () => result.current.handleOpenFolder());
    expect(result.current.workspace?.id).toBe("workspace-a");
    expect(result.current.openingWorkspace).toBe(false);
    expect(result.current.toast).toEqual(expect.objectContaining({
      text: "database error: UNIQUE constraint failed: graph_nodes.id",
      tone: "error",
    }));

    await act(async () => result.current.handleOpenFolder());

    expect(bridge.openWorkspace).toHaveBeenCalledTimes(2);
    expect(result.current.workspace?.id).toBe("workspace-b");
    expect(result.current.openingWorkspace).toBe(false);
    expect(result.current.toast).toEqual(expect.objectContaining({
      text: "Indexing complete: workspace-b (1 file)",
      tone: "success",
    }));
  });

  it("omits ambient runtime events and ignores stale AI results and completion", async () => {
    bridge.listRuntimeEvents.mockResolvedValue([{
      id: "runtime:private-response",
      kind: "http.response",
      timestamp: "2026-08-17T00:00:00Z",
      label: "private response",
      evidence: "observed",
      metadata: { body: "patient@example.test" },
    }]);
    bridge.openWorkspace.mockResolvedValue(workspace("workspace-b"));
    const first = deferred<AiExplanation>();
    const second = deferred<AiExplanation>();
    bridge.explainNode
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));

    let firstRequest: Promise<void> = Promise.resolve();
    act(() => { firstRequest = result.current.handleExplain("node-a"); });
    await waitFor(() => expect(bridge.explainNode).toHaveBeenCalledTimes(1));
    expect(bridge.explainNode).toHaveBeenNthCalledWith(1, expect.objectContaining({
      workspaceId: "workspace-a",
      nodeIds: ["node-a"],
      runtimeEventIds: [],
    }));

    await act(async () => result.current.handleOpenFolder());
    let secondRequest: Promise<void> = Promise.resolve();
    act(() => { secondRequest = result.current.handleExplain("node-b"); });
    await waitFor(() => expect(bridge.explainNode).toHaveBeenCalledTimes(2));
    expect(bridge.explainNode).toHaveBeenNthCalledWith(2, expect.objectContaining({
      workspaceId: "workspace-b",
      nodeIds: ["node-b"],
      runtimeEventIds: [],
    }));
    await act(async () => {
      first.resolve({ answer: "stale", evidence: [], model: "gpt-test" });
      await firstRequest;
    });
    expect(result.current.explanation).toBeNull();
    expect(result.current.explanationError).toBeNull();
    expect(result.current.explaining).toBe(true);
    const current = { answer: "current", evidence: [], model: "gpt-test" };
    await act(async () => {
      second.resolve(current);
      await secondRequest;
    });
    expect(result.current.explanation).toEqual(current);
    expect(result.current.explaining).toBe(false);
  });
  it("ignores an AI error returned after the workspace changes", async () => {
    bridge.openWorkspace.mockResolvedValue(workspace("workspace-b"));
    const first = deferred<AiExplanation>();
    bridge.explainNode.mockImplementationOnce(() => first.promise);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    let request: Promise<void> = Promise.resolve();
    act(() => { request = result.current.handleExplain("node-a"); });
    await waitFor(() => expect(result.current.explaining).toBe(true));
    await act(async () => result.current.handleOpenFolder());
    await act(async () => {
      first.reject(new Error("stale provider failure"));
      await request;
    });
    expect(result.current.workspace?.id).toBe("workspace-b");
    expect(result.current.explanationError).toBeNull();
    expect(result.current.explaining).toBe(false);
  });
});
