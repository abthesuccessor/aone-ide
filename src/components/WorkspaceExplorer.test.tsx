import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { getWorkspaceFiles } from "../lib/bridge";
import type { WorkspaceFile, WorkspaceSummary } from "../types";
import { buildTree, WorkspaceExplorer } from "./WorkspaceExplorer";

vi.mock("../lib/bridge", () => ({ getWorkspaceFiles: vi.fn() }));

const getWorkspaceFilesMock = vi.mocked(getWorkspaceFiles);

function file(relativePath: string): WorkspaceFile {
  return {
    id: `file:${relativePath}`,
    relativePath,
    language: "typescript",
    capability: "semantic",
    sizeBytes: 10,
    contentHash: `hash:${relativePath}`,
    modifiedAt: "2026-08-17T00:00:00Z",
    parseErrors: false,
  };
}

const workspace: WorkspaceSummary = {
  id: "workspace:test",
  name: "large-workspace",
  rootPath: "/workspace",
  fileCount: 6_001,
  nodeCount: 0,
  edgeCount: 0,
  languages: [],
  lastScannedAt: "2026-08-17T00:00:00Z",
};

function renderExplorer(onOpenFile = vi.fn()) {
  render(
    <WorkspaceExplorer
      workspace={workspace}
      files={[file("src/first.ts"), file("src/second.ts")]}
      onOpenFile={onOpenFile}
      onOpenFolder={vi.fn()}
    />,
  );
  return onOpenFile;
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("WorkspaceExplorer indexed-file search", () => {
  it("builds a flat 5,000-file inventory through indexed path lookup", () => {
    const inventory = Array.from({ length: 5_000 }, (_, index) => file(`file-${String(index).padStart(4, "0")}.ts`));
    const startedAt = performance.now();
    const tree = buildTree(inventory);
    const durationMs = performance.now() - startedAt;

    expect(tree).toHaveLength(5_000);
    expect(tree[0]?.path).toBe("file-0000.ts");
    expect(tree.at(-1)?.path).toBe("file-4999.ts");
    expect(durationMs).toBeLessThan(150);
  }, 1_000);

  it("searches the complete backend index and opens a result outside the loaded tree", async () => {
    const hidden = file("zzzz/outside-initial-tree/uniquely-searchable.ts");
    getWorkspaceFilesMock.mockResolvedValue([hidden]);
    const onOpenFile = renderExplorer();

    expect(screen.getByText("Showing 2 of 6,001 indexed files")).toBeVisible();
    fireEvent.change(screen.getByPlaceholderText("Filter files · ⌘K"), {
      target: { value: "uniquely-searchable" },
    });
    expect(screen.getByText(/No files match the loaded list/)).toBeVisible();

    const result = await screen.findByTitle(hidden.relativePath);
    expect(getWorkspaceFilesMock).toHaveBeenCalledWith("uniquely-searchable", 200);
    expect(screen.getByText("Showing 1 matches · searched all 6,001 indexed")).toBeVisible();
    fireEvent.click(result);
    expect(onOpenFile).toHaveBeenCalledWith(hidden, "preview");
    fireEvent.doubleClick(result);
    expect(onOpenFile).toHaveBeenLastCalledWith(hidden, "pinned");
    expect(result).toHaveAttribute("aria-keyshortcuts", "Meta+Enter Control+Enter");
  });

  it("ignores an older query response after a newer search completes", async () => {
    let resolveFirst!: (files: WorkspaceFile[]) => void;
    let resolveSecond!: (files: WorkspaceFile[]) => void;
    getWorkspaceFilesMock.mockImplementation((query) => new Promise((resolve) => {
      if (query === "first-query") resolveFirst = resolve;
      else resolveSecond = resolve;
    }));
    renderExplorer();
    const input = screen.getByPlaceholderText("Filter files · ⌘K");

    fireEvent.change(input, { target: { value: "first-query" } });
    await waitFor(() => expect(getWorkspaceFilesMock).toHaveBeenCalledTimes(1));
    fireEvent.change(input, { target: { value: "second-query" } });
    await waitFor(() => expect(getWorkspaceFilesMock).toHaveBeenCalledTimes(2));

    resolveSecond([file("deep/current-second-query.ts")]);
    expect(await screen.findByTitle("deep/current-second-query.ts")).toBeVisible();
    await act(async () => {
      resolveFirst([file("stale/first-query.ts")]);
      await Promise.resolve();
    });

    expect(screen.queryByTitle("stale/first-query.ts")).not.toBeInTheDocument();
    expect(screen.getByTitle("deep/current-second-query.ts")).toBeVisible();
  });

  it("does not apply a search response from the previously open workspace", async () => {
    let invocation = 0;
    let resolvePrevious!: (files: WorkspaceFile[]) => void;
    let resolveCurrent!: (files: WorkspaceFile[]) => void;
    getWorkspaceFilesMock.mockImplementation(() => new Promise((resolve) => {
      invocation += 1;
      if (invocation === 1) resolvePrevious = resolve;
      else resolveCurrent = resolve;
    }));
    const loaded = [file("src/loaded.ts")];
    const props = {
      files: loaded,
      onOpenFile: vi.fn(),
      onOpenFolder: vi.fn(),
    };
    const { rerender } = render(<WorkspaceExplorer workspace={workspace} {...props} />);
    fireEvent.change(screen.getByPlaceholderText("Filter files · ⌘K"), {
      target: { value: "shared-query" },
    });
    await waitFor(() => expect(getWorkspaceFilesMock).toHaveBeenCalledTimes(1));

    rerender(
      <WorkspaceExplorer
        workspace={{ ...workspace, id: "workspace:current", name: "current" }}
        {...props}
      />,
    );
    await waitFor(() => expect(getWorkspaceFilesMock).toHaveBeenCalledTimes(2));
    resolveCurrent([file("current/shared-query.ts")]);
    expect(await screen.findByTitle("current/shared-query.ts")).toBeVisible();
    await act(async () => {
      resolvePrevious([file("previous/shared-query.ts")]);
      await Promise.resolve();
    });

    expect(screen.queryByTitle("previous/shared-query.ts")).not.toBeInTheDocument();
    expect(screen.getByTitle("current/shared-query.ts")).toBeVisible();
  });

  it("removes controls and caps renderer search input before invoking the backend", () => {
    getWorkspaceFilesMock.mockResolvedValue([]);
    renderExplorer();
    const input = screen.getByPlaceholderText("Filter files · ⌘K") as HTMLInputElement;
    fireEvent.change(input, { target: { value: `safe\n${"x".repeat(300)}` } });

    expect(input.value).not.toContain("\n");
    expect(Array.from(input.value)).toHaveLength(256);
  });

  it("uses one tree tab stop and supports arrow, Home, and End navigation", () => {
    getWorkspaceFilesMock.mockResolvedValue([]);
    renderExplorer();
    const initialItems = screen.getAllByRole("treeitem");
    expect(initialItems.filter((item) => item.tabIndex === 0)).toHaveLength(1);
    const root = screen.getByTitle("src");
    expect(root).toHaveAttribute("tabindex", "0");

    root.focus();
    fireEvent.keyDown(root, { key: "ArrowRight" });
    const firstFile = screen.getByTitle("src/first.ts");
    expect(firstFile).toHaveFocus();
    expect(screen.getAllByRole("treeitem").filter((item) => item.tabIndex === 0)).toEqual([firstFile]);

    fireEvent.keyDown(firstFile, { key: "ArrowLeft" });
    expect(root).toHaveFocus();
    fireEvent.keyDown(root, { key: "End" });
    expect(screen.getByTitle("src/second.ts")).toHaveFocus();
    fireEvent.keyDown(screen.getByTitle("src/second.ts"), { key: "Home" });
    expect(root).toHaveFocus();
  });
});
