import { act, fireEvent, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GraphNode, SourceFile } from "../types";
import type { useAppController } from "./useAppController";
import { useDeveloperWorkbench } from "./useDeveloperWorkbench";

const bridge = vi.hoisted(() => ({
  getFormatterCapabilities: vi.fn(),
}));

vi.mock("../lib/editorBridge", () => ({
  formatDocument: vi.fn(),
  getFormatterCapabilities: bridge.getFormatterCapabilities,
  writeWorkspaceFile: vi.fn(),
}));

function core(
  source: SourceFile | null,
  openFolder: () => Promise<unknown>,
  openingWorkspace = false,
  cloneRepository = vi.fn().mockResolvedValue({ status: "cancelled" }),
) {
  return {
    source,
    workspace: { id: "workspace-a" },
    displayGraph: { nodes: [], edges: [], truncated: false },
    executionFlowGraph: { nodes: [], edges: [], truncated: false },
    aiConfiguration: { inferenceAvailable: true },
    workspaceGeneration: 1,
    openingWorkspace,
    notify: vi.fn(),
    openSource: vi.fn().mockResolvedValue(undefined),
    openSourceAt: vi.fn(),
    focusSourceWithoutRange: vi.fn(),
    setMainView: vi.fn(),
    setSelectedNodeId: vi.fn(),
    handleOpenFolder: openFolder,
    handleCloneRepository: cloneRepository,
    handleCreateProject: vi.fn().mockResolvedValue({ status: "cancelled" }),
  } as unknown as ReturnType<typeof useAppController>;
}

beforeEach(() => {
  window.localStorage.clear();
  bridge.getFormatterCapabilities.mockResolvedValue([]);
});

afterEach(() => vi.restoreAllMocks());

describe("useDeveloperWorkbench workspace opening", () => {
  it("opens a workspace in explicitly selected code-only mode", async () => {
    const openFolder = vi.fn().mockResolvedValue(undefined);
    const controller = core(null, openFolder) as ReturnType<typeof useAppController> & {
      codeOnlyMode: boolean;
    };
    controller.aiConfiguration = {
      configured: false,
      provider: null,
      model: null,
      transport: null,
      inferenceAvailable: false,
    };
    controller.codeOnlyMode = true;
    const { result } = renderHook(() => useDeveloperWorkbench(controller));

    await act(async () => result.current.handleOpenFolder());

    expect(openFolder).toHaveBeenCalledTimes(1);
  });

  it("selects Search with the VS Code Shift-Command-F shortcut", () => {
    const openFolder = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useDeveloperWorkbench(core(null, openFolder)));

    fireEvent.keyDown(window, { key: "f", metaKey: true, shiftKey: true });

    expect(result.current.activityView).toBe("search");
  });

  it("activates the Run and Debug sidebar when a debug process starts", () => {
    const controller = core(null, vi.fn().mockResolvedValue(undefined));
    const { result, rerender } = renderHook(
      ({ active }) => useDeveloperWorkbench({
        ...controller,
        activeRun: active ? { id: "run-debug", mode: "debug", stopping: false } : null,
      } as ReturnType<typeof useAppController>),
      { initialProps: { active: false } },
    );
    act(() => result.current.setPrimarySidebarVisible(false));

    rerender({ active: true });

    expect(result.current.activityView).toBe("debugger");
    expect(result.current.primarySidebarVisible).toBe(true);
  });

  it("does not call the native folder picker when dirty-workspace consent is cancelled", async () => {
    const openFolder = vi.fn().mockResolvedValue(undefined);
    const selected: SourceFile = {
      relativePath: "src/dirty.ts",
      language: "TypeScript",
      content: "saved",
      contentHash: "hash-a",
    };
    const { result, rerender } = renderHook(
      ({ selectedSource }) => useDeveloperWorkbench(core(selectedSource, openFolder)),
      { initialProps: { selectedSource: null as SourceFile | null } },
    );
    rerender({ selectedSource: selected });
    await waitFor(() => expect(result.current.editor.activeDocument).not.toBeNull());
    act(() => result.current.editor.updateContent("unsaved"));
    vi.spyOn(window, "confirm").mockReturnValue(false);

    await act(async () => result.current.handleOpenFolder());

    expect(window.confirm).toHaveBeenCalledTimes(1);
    expect(openFolder).not.toHaveBeenCalled();
    expect(result.current.editor.activeDocument).toMatchObject({ content: "unsaved", dirty: true });
  });

  it("calls the shared folder-opening path after explicit dirty-workspace consent", async () => {
    const openFolder = vi.fn().mockResolvedValue(undefined);
    const selected: SourceFile = {
      relativePath: "src/dirty.ts",
      language: "TypeScript",
      content: "saved",
      contentHash: "hash-a",
    };
    const { result } = renderHook(() => useDeveloperWorkbench(core(selected, openFolder)));
    await waitFor(() => expect(result.current.editor.activeDocument).not.toBeNull());
    act(() => result.current.editor.updateContent("unsaved"));
    vi.spyOn(window, "confirm").mockReturnValue(true);

    await act(async () => result.current.handleOpenFolder());

    expect(openFolder).toHaveBeenCalledTimes(1);
  });

  it("opens the single Project Agent surface after a folder is opened successfully", async () => {
    const openFolder = vi.fn().mockResolvedValue({ status: "success", result: { workspace: { id: "workspace-b" } } });
    const target = core(null, openFolder);
    const { result } = renderHook(() => useDeveloperWorkbench(target));

    await act(async () => result.current.handleOpenFolder());

    expect(result.current.activityView).toBe("agent-setup");
    expect(result.current.primarySidebarVisible).toBe(true);
    expect(target.setMainView).toHaveBeenCalledWith("graph");
  });

  it("uses the same dirty-workspace consent before a repository clone", async () => {
    const cloneRepository = vi.fn().mockResolvedValue({ status: "success" });
    const selected: SourceFile = {
      relativePath: "src/dirty.ts",
      language: "TypeScript",
      content: "saved",
      contentHash: "hash-a",
    };
    const { result } = renderHook(() => useDeveloperWorkbench(
      core(selected, vi.fn().mockResolvedValue(undefined), false, cloneRepository),
    ));
    await waitFor(() => expect(result.current.editor.activeDocument).not.toBeNull());
    act(() => result.current.editor.updateContent("unsaved"));
    vi.spyOn(window, "confirm").mockReturnValue(false);

    let outcome: unknown;
    await act(async () => {
      outcome = await result.current.handleCloneRepository({
        repositoryUrl: "https://github.com/example/repository",
        destinationName: "repository",
      });
    });
    expect(outcome).toEqual({ status: "cancelled" });
    expect(cloneRepository).not.toHaveBeenCalled();
  });

  it("does not prompt or invoke another picker while workspace opening is active", async () => {
    const openFolder = vi.fn().mockResolvedValue(undefined);
    const confirm = vi.spyOn(window, "confirm");
    const { result } = renderHook(() => useDeveloperWorkbench(core(null, openFolder, true)));

    await act(async () => result.current.handleOpenFolder());

    expect(confirm).not.toHaveBeenCalled();
    expect(openFolder).not.toHaveBeenCalled();
  });

  it("preserves a shifted dirty buffer and skips the saved-index range", async () => {
    const dirtySource: SourceFile = {
      relativePath: "src/dirty.ts",
      language: "TypeScript",
      content: "first\ncheckout();",
      contentHash: "hash-a",
    };
    const otherSource: SourceFile = {
      relativePath: "src/other.ts",
      language: "TypeScript",
      content: "const other = true;",
      contentHash: "hash-other",
    };
    const controller = core(null, vi.fn().mockResolvedValue(undefined));
    const { result, rerender } = renderHook(
      ({ selectedSource }) => useDeveloperWorkbench({
        ...controller,
        source: selectedSource,
      } as ReturnType<typeof useAppController>),
      { initialProps: { selectedSource: dirtySource } },
    );
    await waitFor(() => expect(result.current.editor.activePath).toBe(dirtySource.relativePath));
    act(() => result.current.editor.updateContent("inserted\nfirst\ncheckout();"));
    rerender({ selectedSource: otherSource });
    await waitFor(() => expect(result.current.editor.activePath).toBe(otherSource.relativePath));

    act(() => result.current.openSearchMatch({
      relativePath: dirtySource.relativePath,
      startLine: 2,
      startColumn: 1,
      endLine: 2,
      endColumn: 9,
    }));

    expect(result.current.editor.activePath).toBe(dirtySource.relativePath);
    expect(result.current.editor.activeDocument).toMatchObject({
      content: "inserted\nfirst\ncheckout();",
      dirty: true,
    });
    expect(controller.openSourceAt).not.toHaveBeenCalled();
    expect(controller.focusSourceWithoutRange).toHaveBeenCalledTimes(1);
    expect(controller.notify).toHaveBeenCalledWith(
      "Search result is from the saved version. Exact selection skipped for the unsaved file.",
      "warning",
    );
  });

  it("opens an indexed range normally when the target document is clean", async () => {
    const selected: SourceFile = {
      relativePath: "src/clean.ts",
      language: "TypeScript",
      content: "checkout();",
      contentHash: "hash-clean",
    };
    const controller = core(selected, vi.fn().mockResolvedValue(undefined));
    const location = {
      relativePath: selected.relativePath,
      startLine: 1,
      startColumn: 1,
      endLine: 1,
      endColumn: 9,
    };
    const { result } = renderHook(() => useDeveloperWorkbench(controller));
    await waitFor(() => expect(result.current.editor.activeDocument).not.toBeNull());

    act(() => result.current.openSearchMatch(location));

    expect(controller.openSourceAt).toHaveBeenCalledWith(location);
    expect(controller.focusSourceWithoutRange).not.toHaveBeenCalled();
    expect(controller.notify).not.toHaveBeenCalled();
  });

  it("opens a changed file in Relationships and refines selection from its diff", () => {
    const at = (id: string, startLine: number, endLine: number): GraphNode => ({
      id,
      kind: "function",
      label: id,
      source: {
        relativePath: "src/changed.ts",
        startLine,
        startColumn: 1,
        endLine,
        endColumn: 1,
      },
      evidence: "resolved",
      metadata: {},
    });
    const fileScope = at("file-scope", 1, 80);
    const changedCall = at("changed-call", 24, 25);
    const api: GraphNode = {
      ...at("api", 1, 4),
      kind: "api",
      source: {
        relativePath: "src/api.ts",
        startLine: 1,
        startColumn: 1,
        endLine: 4,
        endColumn: 1,
      },
    };
    const controller = core(null, vi.fn().mockResolvedValue(undefined));
    Object.assign(controller, {
      displayGraph: {
        nodes: [api, fileScope, changedCall],
        edges: [{
          id: "api-changed-call",
          source: "api",
          target: "changed-call",
          kind: "calls",
          evidence: "resolved",
          metadata: {},
        }],
        truncated: false,
      },
    });
    const { result } = renderHook(() => useDeveloperWorkbench(controller));

    act(() => result.current.setGitStatus({
      isRepository: true,
      files: [{
        relativePath: "src/changed.ts",
        indexStatus: " ",
        workingTreeStatus: "M",
        staged: false,
        conflicted: false,
      }, {
        relativePath: "README.md",
        indexStatus: " ",
        workingTreeStatus: "M",
        staged: false,
        conflicted: false,
      }],
    }));
    act(() => result.current.openGitChangedFile("src/changed.ts"));

    expect(result.current.relationshipView).toBe("changes");
    expect(controller.setMainView).toHaveBeenCalledWith("graph");
    expect(controller.openSource).toHaveBeenCalledWith(
      "src/changed.ts",
      expect.objectContaining({ id: "git-change:src/changed.ts" }),
      expect.objectContaining({ relativePath: "src/changed.ts" }),
      "graph",
    );
    expect(result.current.selectedGitPath).toBe("src/changed.ts");
    expect(result.current.relationshipGraph.nodes.map((node) => node.id)).toEqual(expect.arrayContaining([
      "api",
      "changed-call",
      "git-change:src/changed.ts",
    ]));
    expect(result.current.relationshipGraph.nodes.some((node) => node.id === "git-change:README.md")).toBe(false);

    act(() => result.current.openGitDiff({
      relativePath: "src/changed.ts",
      staged: false,
      content: "@@ -23,1 +23,2 @@\n unchanged();\n+changedCall();",
      truncated: false,
      comparisonTruncated: false,
    }));

    expect(controller.setSelectedNodeId).toHaveBeenLastCalledWith("changed-call");

    act(() => result.current.selectRelationshipNode("api"));

    expect(result.current.gitDiff).toBeNull();
    expect(result.current.selectedGitPath).toBe("src/changed.ts");
    expect(controller.openSource).toHaveBeenLastCalledWith(
      "src/api.ts",
      expect.objectContaining({ id: "api" }),
      expect.objectContaining({ relativePath: "src/api.ts" }),
      "graph",
    );
  });
});
