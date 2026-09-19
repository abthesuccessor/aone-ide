import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SourceFile, SourceLocation } from "../types";
import { useAppController } from "./useAppController";

const bridge = vi.hoisted(() => ({
  getAiConfigurationStatus: vi.fn(),
  getAppSnapshot: vi.fn(),
  readWorkspaceFile: vi.fn(),
}));

vi.mock("../lib/bridge", () => ({
  detectRunProfiles: vi.fn(),
  explainNode: vi.fn(),
  getAiConfigurationStatus: bridge.getAiConfigurationStatus,
  getAppSnapshot: bridge.getAppSnapshot,
  getWorkspaceFiles: vi.fn(),
  isTauriRuntime: () => false,
  listRuntimeEvents: vi.fn(),
  openWorkspace: vi.fn(),
  openWorkspaceFile: vi.fn(),
  pickAndLoadEnvFile: vi.fn(),
  pickAndLoadRunEnvFile: vi.fn(),
  queryGraph: vi.fn(),
  readWorkspaceFile: bridge.readWorkspaceFile,
  rescanWorkspace: vi.fn(),
  sendApiRequest: vi.fn(),
  startRun: vi.fn(),
  stopRun: vi.fn(),
}));

vi.mock("./useRuntimeSubscriptions", () => ({
  useRuntimeSubscriptions: () => ({ cancelWorkspaceReload: vi.fn(), eventSubscriptionReady: true }),
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

beforeEach(() => {
  vi.clearAllMocks();
  bridge.getAiConfigurationStatus.mockResolvedValue({
    configured: true, provider: "openai", model: "test-model", transport: "api", inferenceAvailable: true,
  });
  bridge.getAppSnapshot.mockResolvedValue({ workspace: null });
});

describe("useAppController source ordering", () => {
  it("retains the exact search range for the existing source editor", async () => {
    const selected: SourceFile = {
      relativePath: "src/routes.ts",
      language: "TypeScript",
      content: "router.post('/checkout')",
      contentHash: "hash-search",
    };
    const location: SourceLocation = {
      relativePath: selected.relativePath,
      startLine: 17,
      startColumn: 4,
      endLine: 17,
      endColumn: 12,
    };
    bridge.readWorkspaceFile.mockResolvedValue(selected);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));

    await act(async () => result.current.openSourceAt(location));

    expect(result.current.source).toEqual(selected);
    expect(result.current.sourceLocation).toEqual(location);
    expect(result.current.mainView).toBe("source");
  });

  it("clears disk-derived ranges when focusing an already open dirty source", async () => {
    const selected: SourceFile = {
      relativePath: "src/routes.ts",
      language: "TypeScript",
      content: "router.post('/checkout')",
      contentHash: "hash-search",
    };
    const location: SourceLocation = {
      relativePath: selected.relativePath,
      startLine: 17,
      startColumn: 4,
      endLine: 17,
      endColumn: 12,
    };
    bridge.readWorkspaceFile.mockResolvedValue(selected);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));
    await act(async () => result.current.openSourceAt(location));

    act(() => result.current.focusSourceWithoutRange());

    expect(result.current.sourceLocation).toBeNull();
    expect(result.current.selectedNodeId).toBeNull();
    expect(result.current.mainView).toBe("source");
  });

  it("reuses an already loaded file when graph selection changes its source range", async () => {
    const selected: SourceFile = {
      relativePath: "src/routes.ts",
      language: "TypeScript",
      content: "router.post('/checkout')",
      contentHash: "hash-search",
    };
    bridge.readWorkspaceFile.mockResolvedValue(selected);
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));

    await act(async () => result.current.openSourceAt({
      relativePath: selected.relativePath,
      startLine: 17,
      startColumn: 1,
      endLine: 17,
      endColumn: 8,
    }));
    await act(async () => result.current.openSourceAtInGraph({
      relativePath: selected.relativePath,
      startLine: 42,
      startColumn: 1,
      endLine: 44,
      endColumn: 2,
    }));

    expect(bridge.readWorkspaceFile).toHaveBeenCalledTimes(1);
    expect(result.current.sourceLocation?.startLine).toBe(42);
    expect(result.current.mainView).toBe("graph");
  });

  it("keeps source B when source A resolves after the newer request", async () => {
    let resolveA: ((source: SourceFile) => void) | undefined;
    let resolveB: ((source: SourceFile) => void) | undefined;
    bridge.readWorkspaceFile
      .mockImplementationOnce(() => new Promise((resolve) => { resolveA = resolve; }))
      .mockImplementationOnce(() => new Promise((resolve) => { resolveB = resolve; }));
    const sourceA: SourceFile = {
      relativePath: "src/a.ts",
      language: "TypeScript",
      content: "A",
      contentHash: "hash-a",
    };
    const sourceB: SourceFile = {
      relativePath: "src/b.ts",
      language: "TypeScript",
      content: "B",
      contentHash: "hash-b",
    };
    const { result } = renderHook(() => useAppController());
    await waitFor(() => expect(result.current.phase).toBe("ready"));

    let requestA: Promise<void> = Promise.resolve();
    let requestB: Promise<void> = Promise.resolve();
    act(() => { requestA = result.current.openSource(sourceA.relativePath); });
    act(() => { requestB = result.current.openSource(sourceB.relativePath); });
    await act(async () => {
      resolveB?.(sourceB);
      await requestB;
    });
    expect(result.current.source).toEqual(sourceB);
    expect(result.current.sourceLoading).toBe(false);

    await act(async () => {
      resolveA?.(sourceA);
      await requestA;
    });
    expect(result.current.source).toEqual(sourceB);
    expect(result.current.sourceError).toBeNull();
    expect(result.current.sourceLoading).toBe(false);
  });
});
