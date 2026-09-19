import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SourceFile } from "../../types";
import { DEFAULT_WORKBENCH_SETTINGS } from "./settings";
import { useEditorController } from "./useEditorController";

const bridge = vi.hoisted(() => ({
  formatDocument: vi.fn(),
  getFormatterCapabilities: vi.fn(),
  writeWorkspaceFile: vi.fn(),
}));

vi.mock("../../lib/editorBridge", () => bridge);

function source(relativePath: string, content = "const value = 1;"): SourceFile {
  return { relativePath, content, language: "TypeScript", contentHash: `hash:${relativePath}` };
}

beforeEach(() => {
  vi.clearAllMocks();
  bridge.getFormatterCapabilities.mockResolvedValue([
    { language: "TypeScript", formatter: "Prettier", available: true, external: true },
  ]);
  bridge.formatDocument.mockResolvedValue({
    content: "const value = 2;\n",
    formatter: "Prettier",
    changed: true,
    usedExternalTool: true,
  });
  vi.spyOn(window, "confirm").mockReturnValue(true);
});

afterEach(() => vi.restoreAllMocks());

describe("useEditorController", () => {
  it("opens multiple files, tracks dirty state, and chooses the previous tab on close", async () => {
    const notify = vi.fn();
    const first = source("src/first.ts");
    const second = source("src/second.ts");
    const { result, rerender } = renderHook(
      ({ selected }) => useEditorController({
        source: selected,
        workspaceId: "workspace-1",
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify,
      }),
      { initialProps: { selected: first as SourceFile | null } },
    );

    await waitFor(() => expect(result.current.activePath).toBe(first.relativePath));
    expect(result.current.activeDocument?.preview).toBe(true);
    act(() => result.current.updateContent("const value = 3;"));
    expect(result.current.activeDocument?.dirty).toBe(true);
    expect(result.current.activeDocument?.preview).toBe(false);

    rerender({ selected: second });
    await waitFor(() => expect(result.current.documents).toHaveLength(2));
    act(() => result.current.closeDocument(second.relativePath));
    expect(result.current.activePath).toBe(first.relativePath);
    expect(result.current.documents.map((document) => document.relativePath)).toEqual([first.relativePath]);
  });

  it("replaces one clean preview while preserving pinned and edited documents", async () => {
    const first = source("src/first.ts");
    const second = source("src/second.ts");
    const third = source("src/third.ts");
    const { result, rerender } = renderHook(
      ({ selected }) => useEditorController({
        source: selected,
        workspaceId: "workspace-1",
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify: vi.fn(),
      }),
      { initialProps: { selected: first as SourceFile | null } },
    );

    await waitFor(() => expect(result.current.activePath).toBe(first.relativePath));
    rerender({ selected: second });
    await waitFor(() => expect(result.current.activePath).toBe(second.relativePath));
    expect(result.current.documents).toMatchObject([
      { relativePath: second.relativePath, preview: true, dirty: false },
    ]);

    act(() => result.current.pinDocument(second.relativePath));
    expect(result.current.activeDocument?.preview).toBe(false);
    rerender({ selected: third });
    await waitFor(() => expect(result.current.activePath).toBe(third.relativePath));
    expect(result.current.documents).toMatchObject([
      { relativePath: second.relativePath, preview: false },
      { relativePath: third.relativePath, preview: true },
    ]);

    act(() => result.current.updateContent(third.content));
    expect(result.current.activeDocument).toMatchObject({ dirty: false, preview: false });
    rerender({ selected: first });
    await waitFor(() => expect(result.current.activePath).toBe(first.relativePath));
    expect(result.current.documents.map((document) => document.relativePath)).toEqual([
      second.relativePath,
      third.relativePath,
      first.relativePath,
    ]);
  });

  it("honors a pin requested before the source read completes", async () => {
    const selected = source("src/pending.ts");
    const { result, rerender } = renderHook(
      ({ value }) => useEditorController({
        source: value,
        workspaceId: "workspace-1",
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify: vi.fn(),
      }),
      { initialProps: { value: null as SourceFile | null } },
    );

    act(() => result.current.pinDocument(selected.relativePath));
    rerender({ value: selected });
    await waitFor(() => expect(result.current.activeDocument).toMatchObject({
      relativePath: selected.relativePath,
      preview: false,
    }));
  });

  it("keeps a dirty tab open when close confirmation is declined", async () => {
    vi.mocked(window.confirm).mockReturnValue(false);
    const selected = source("src/guarded.ts");
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-1",
      settings: DEFAULT_WORKBENCH_SETTINGS,
      notify: vi.fn(),
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("changed"));
    let closed = true;
    act(() => { closed = result.current.closeDocument(selected.relativePath); });

    expect(window.confirm).toHaveBeenCalledWith("Close src/guarded.ts without saving?");
    expect(closed).toBe(false);
    expect(result.current.documents).toHaveLength(1);
  });

  it("formats on save and replaces the optimistic hash only after a successful write", async () => {
    const notify = vi.fn();
    const selected = source("src/save.ts");
    bridge.writeWorkspaceFile.mockResolvedValue({
      ...selected,
      content: "const value = 2;\n",
      contentHash: "hash:saved",
    });
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-1",
      settings: { ...DEFAULT_WORKBENCH_SETTINGS, formatOnSave: true, wordWrapColumn: 88 },
      notify,
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("const value = 2;   "));

    await act(async () => { await result.current.saveDocument(); });

    expect(bridge.formatDocument).toHaveBeenCalledWith(expect.objectContaining({
      workspaceId: "workspace-1",
      relativePath: selected.relativePath,
      expectedContentHash: selected.contentHash,
      printWidth: 88,
    }));
    expect(bridge.writeWorkspaceFile).toHaveBeenCalledWith({
      workspaceId: "workspace-1",
      relativePath: selected.relativePath,
      content: "const value = 2;\n",
      expectedContentHash: selected.contentHash,
    });
    expect(result.current.activeDocument).toMatchObject({
      content: "const value = 2;\n",
      contentHash: "hash:saved",
      dirty: false,
    });
    expect(notify).toHaveBeenCalledWith("Saved src/save.ts", "success");
  });

  it("advances the saved baseline without overwriting typing completed during save", async () => {
    let finishWrite: ((value: SourceFile) => void) | undefined;
    bridge.writeWorkspaceFile.mockImplementation(() => new Promise((resolve) => { finishWrite = resolve; }));
    const selected = source("src/racing-save.ts");
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-1",
      settings: DEFAULT_WORKBENCH_SETTINGS,
      notify: vi.fn(),
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("submitted content"));
    let saving: Promise<unknown>;
    act(() => { saving = result.current.saveDocument(); });
    act(() => result.current.updateContent("newer typing"));
    await act(async () => {
      finishWrite?.({ ...selected, content: "submitted content", contentHash: "hash:new-baseline" });
      await saving;
    });

    expect(result.current.activeDocument).toMatchObject({
      content: "newer typing",
      savedContent: "submitted content",
      contentHash: "hash:new-baseline",
      dirty: true,
    });
  });

  it.each(["save", "format"] as const)("preserves actionable native %s failures and unsaved content", async (operation) => {
    const notify = vi.fn();
    const selected = source("src/conflicted.ts");
    const message = "invalid request: file changed on disk; reload it before formatting or saving";
    if (operation === "save") bridge.writeWorkspaceFile.mockRejectedValueOnce(message);
    else bridge.formatDocument.mockRejectedValueOnce(message);
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-1",
      settings: DEFAULT_WORKBENCH_SETTINGS,
      notify,
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("unsaved changes"));

    await act(async () => {
      if (operation === "save") await result.current.saveDocument();
      else await result.current.formatActiveDocument();
    });

    expect(notify).toHaveBeenCalledWith(message, "error");
    expect(result.current.activeDocument).toMatchObject({
      content: "unsaved changes", contentHash: selected.contentHash, dirty: true,
    });
    expect(result.current.savingPath).toBeNull();
    expect(result.current.formattingPath).toBeNull();
  });

  it("does not apply a formatter result over typing completed during formatting", async () => {
    let finishFormat: ((value: {
      content: string;
      formatter: string;
      changed: boolean;
      usedExternalTool: boolean;
    }) => void) | undefined;
    bridge.formatDocument.mockImplementation(() => new Promise((resolve) => { finishFormat = resolve; }));
    const selected = source("src/racing-format.ts", "unformatted   ");
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-1",
      settings: DEFAULT_WORKBENCH_SETTINGS,
      notify: vi.fn(),
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    let formatting: Promise<void>;
    act(() => { formatting = result.current.formatActiveDocument(); });
    act(() => result.current.updateContent("newer typing"));
    await act(async () => {
      finishFormat?.({ content: "formatted\n", formatter: "Prettier", changed: true, usedExternalTool: true });
      await formatting;
    });

    expect(result.current.activeDocument).toMatchObject({ content: "newer typing", dirty: true });
  });

  it("synchronously refuses leaving a dirty workspace with a bounded prompt", async () => {
    vi.mocked(window.confirm).mockReturnValue(false);
    const selected = source(`src/${"very-long-path-".repeat(20)}.ts`);
    const { result } = renderHook(() => useEditorController({
      source: selected,
      workspaceId: "workspace-a",
      workspaceGeneration: 1,
      settings: DEFAULT_WORKBENCH_SETTINGS,
      notify: vi.fn(),
    }));
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("unsaved"));
    let allowed = true;
    act(() => { allowed = result.current.confirmCanLeaveWorkspace(); });

    expect(allowed).toBe(false);
    const message = vi.mocked(window.confirm).mock.calls[0]?.[0];
    expect(message).toContain("discard unsaved changes in 1 file");
    expect(String(message).length).toBeLessThan(260);
    expect(result.current.activeDocument).toMatchObject({ content: "unsaved", dirty: true });
  });

  it("does not let a save from workspace A mutate the same path in workspace B", async () => {
    let finishWrite: ((value: SourceFile) => void) | undefined;
    bridge.writeWorkspaceFile.mockImplementation(() => new Promise((resolve) => { finishWrite = resolve; }));
    const notify = vi.fn();
    const fromA = source("src/shared.ts", "workspace A");
    const fromB = { ...source("src/shared.ts", "workspace B"), contentHash: "hash:workspace-b" };
    const { result, rerender } = renderHook(
      ({ selected, workspaceId, generation }) => useEditorController({
        source: selected,
        workspaceId,
        workspaceGeneration: generation,
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify,
      }),
      { initialProps: { selected: fromA, workspaceId: "workspace-a", generation: 1 } },
    );
    await waitFor(() => expect(result.current.activeDocument?.content).toBe("workspace A"));
    act(() => result.current.updateContent("save from A"));
    let saving: Promise<unknown> = Promise.resolve();
    act(() => { saving = result.current.saveDocument(); });

    rerender({ selected: fromB, workspaceId: "workspace-b", generation: 2 });
    await waitFor(() => expect(result.current.activeDocument).toMatchObject({
      content: "workspace B",
      contentHash: "hash:workspace-b",
      dirty: false,
    }));
    await act(async () => {
      finishWrite?.({ ...fromA, content: "save from A", contentHash: "hash:saved-a" });
      await saving;
    });

    expect(result.current.activeDocument).toMatchObject({
      content: "workspace B",
      contentHash: "hash:workspace-b",
      dirty: false,
    });
    expect(notify).not.toHaveBeenCalledWith("Saved src/shared.ts", "success");
  });

  it("does not let a format from workspace A mutate the same path in workspace B", async () => {
    let finishFormat: ((value: {
      content: string;
      formatter: string;
      changed: boolean;
      usedExternalTool: boolean;
    }) => void) | undefined;
    bridge.formatDocument.mockImplementation(() => new Promise((resolve) => { finishFormat = resolve; }));
    const notify = vi.fn();
    const fromA = source("src/shared.ts", "workspace A   ");
    const fromB = { ...source("src/shared.ts", "workspace B"), contentHash: "hash:workspace-b" };
    const { result, rerender } = renderHook(
      ({ selected, workspaceId, generation }) => useEditorController({
        source: selected,
        workspaceId,
        workspaceGeneration: generation,
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify,
      }),
      { initialProps: { selected: fromA, workspaceId: "workspace-a", generation: 4 } },
    );
    await waitFor(() => expect(result.current.activeDocument?.content).toBe("workspace A   "));
    let formatting: Promise<void> = Promise.resolve();
    act(() => { formatting = result.current.formatActiveDocument(); });

    rerender({ selected: fromB, workspaceId: "workspace-b", generation: 5 });
    await waitFor(() => expect(result.current.activeDocument?.content).toBe("workspace B"));
    await act(async () => {
      finishFormat?.({ content: "formatted A\n", formatter: "Prettier", changed: true, usedExternalTool: true });
      await formatting;
    });

    expect(result.current.activeDocument).toMatchObject({
      content: "workspace B",
      contentHash: "hash:workspace-b",
      dirty: false,
    });
    expect(notify).not.toHaveBeenCalled();
  });

  it("does not write after format-on-save crosses a workspace generation", async () => {
    let finishFormat: ((value: {
      content: string;
      formatter: string;
      changed: boolean;
      usedExternalTool: boolean;
    }) => void) | undefined;
    bridge.formatDocument.mockImplementation(() => new Promise((resolve) => { finishFormat = resolve; }));
    const fromA = source("src/shared.ts", "workspace A");
    const fromB = source("src/shared.ts", "workspace B");
    const { result, rerender } = renderHook(
      ({ selected, workspaceId, generation }) => useEditorController({
        source: selected,
        workspaceId,
        workspaceGeneration: generation,
        settings: { ...DEFAULT_WORKBENCH_SETTINGS, formatOnSave: true },
        notify: vi.fn(),
      }),
      { initialProps: { selected: fromA, workspaceId: "same-workspace-id", generation: 7 } },
    );
    await waitFor(() => expect(result.current.activeDocument).not.toBeNull());
    act(() => result.current.updateContent("dirty A"));
    let saving: Promise<unknown> = Promise.resolve();
    act(() => { saving = result.current.saveDocument(); });
    rerender({ selected: fromB, workspaceId: "same-workspace-id", generation: 8 });
    await act(async () => {
      finishFormat?.({ content: "formatted A\n", formatter: "Prettier", changed: true, usedExternalTool: true });
      await saving;
    });

    expect(bridge.writeWorkspaceFile).not.toHaveBeenCalled();
    expect(result.current.activeDocument?.content).toBe("workspace B");
  });

  it("locks edits and document mutations until workspace opening unwinds", async () => {
    const fromA = source("src/locked.ts", "workspace A");
    const fromB = { ...source("src/locked.ts", "workspace B"), contentHash: "hash:b" };
    const { result, rerender } = renderHook(
      ({ selected, workspaceId, generation, opening }) => useEditorController({
        source: selected,
        workspaceId,
        workspaceGeneration: generation,
        openingWorkspace: opening,
        settings: DEFAULT_WORKBENCH_SETTINGS,
        notify: vi.fn(),
      }),
      { initialProps: { selected: fromA, workspaceId: "workspace-a", generation: 1, opening: false } },
    );
    await waitFor(() => expect(result.current.activeDocument?.content).toBe("workspace A"));

    rerender({ selected: fromA, workspaceId: "workspace-a", generation: 1, opening: true });
    act(() => result.current.updateContent("post-consent edit"));
    let closed = true;
    act(() => { closed = result.current.closeDocument(fromA.relativePath); });
    await act(async () => {
      await result.current.saveDocument();
      await result.current.formatActiveDocument();
    });

    expect(closed).toBe(false);
    expect(result.current.activeDocument).toMatchObject({ content: "workspace A", dirty: false });
    expect(bridge.writeWorkspaceFile).not.toHaveBeenCalled();
    expect(bridge.formatDocument).not.toHaveBeenCalled();

    rerender({ selected: fromA, workspaceId: "workspace-a", generation: 1, opening: false });
    act(() => result.current.updateContent("edit after cancel"));
    expect(result.current.activeDocument).toMatchObject({ content: "edit after cancel", dirty: true });

    rerender({ selected: fromB, workspaceId: "workspace-b", generation: 2, opening: true });
    await waitFor(() => expect(result.current.activeDocument).toBeNull());
    rerender({ selected: fromB, workspaceId: "workspace-b", generation: 2, opening: false });
    await waitFor(() => expect(result.current.activeDocument).toMatchObject({
      content: "workspace B",
      dirty: false,
    }));
  });
});
