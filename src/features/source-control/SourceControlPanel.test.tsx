import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SourceControlPanel } from "./SourceControlPanel";

const git = vi.hoisted(() => ({
  getGitDiff: vi.fn(),
  getGitStatus: vi.fn(),
  gitCommit: vi.fn(),
  gitInitialize: vi.fn(),
  gitStage: vi.fn(),
  gitUnstage: vi.fn(),
}));

vi.mock("../../lib/devtoolsBridge", () => git);

beforeEach(() => {
  vi.clearAllMocks();
  git.getGitStatus.mockResolvedValue({
    isRepository: true,
    branch: "feature/graph",
    files: [
      { relativePath: "src/changed.ts", indexStatus: " ", workingTreeStatus: "M", staged: false, conflicted: false },
      { relativePath: "src/staged.ts", indexStatus: "A", workingTreeStatus: " ", staged: true, conflicted: false },
    ],
  });
  git.getGitDiff.mockResolvedValue({
    relativePath: "src/changed.ts",
    staged: false,
    content: "+const observed = true;",
    truncated: false,
    originalContent: "const observed = false;",
    modifiedContent: "const observed = true;",
    comparisonTruncated: false,
  });
  git.gitStage.mockResolvedValue({ ok: true, summary: "staged" });
  git.gitCommit.mockResolvedValue({ ok: true, summary: "committed" });
});

describe("SourceControlPanel", () => {
  it("opens a file with its diff and stages it through the typed bridge", async () => {
    const onOpenFile = vi.fn();
    const onOpenDiff = vi.fn();
    render(<SourceControlPanel workspaceId="workspace-1" onOpenFile={onOpenFile} onOpenDiff={onOpenDiff} />);
    const file = await screen.findByRole("button", { name: "Open working tree diff for src/changed.ts" });

    fireEvent.click(file);
    expect(onOpenFile).toHaveBeenCalledWith("src/changed.ts", "preview");
    expect(file).toHaveAttribute("aria-current", "true");
    expect(file.closest(".git-file")).toHaveClass("is-active");
    await waitFor(() => expect(onOpenDiff).toHaveBeenCalledWith(expect.objectContaining({
      relativePath: "src/changed.ts",
      modifiedContent: "const observed = true;",
    })));
    expect(git.getGitDiff).toHaveBeenCalledWith({ relativePath: "src/changed.ts", staged: false });

    fireEvent.click(screen.getByRole("button", { name: "Stage src/changed.ts" }));
    await waitFor(() => expect(git.gitStage).toHaveBeenCalledWith({ paths: ["src/changed.ts"] }));
  });

  it("follows an externally selected relationship node back to its changed file", async () => {
    const { rerender } = render(
      <SourceControlPanel workspaceId="workspace-1" selectedPath="src/changed.ts" onOpenFile={vi.fn()} />,
    );
    const changed = await screen.findByRole("button", { name: "Open working tree diff for src/changed.ts" });
    const staged = screen.getByRole("button", { name: "Open staged diff for src/staged.ts" });

    expect(changed.closest(".git-file")).toHaveClass("is-active");
    expect(staged.closest(".git-file")).not.toHaveClass("is-active");

    rerender(
      <SourceControlPanel workspaceId="workspace-1" selectedPath="src/staged.ts" onOpenFile={vi.fn()} />,
    );
    expect(staged.closest(".git-file")).toHaveClass("is-active");
    expect(changed.closest(".git-file")).not.toHaveClass("is-active");
  });

  it("commits only staged changes and clears the successful message", async () => {
    render(<SourceControlPanel workspaceId="workspace-1" onOpenFile={vi.fn()} />);
    const input = await screen.findByRole("textbox", { name: "Commit message" });
    fireEvent.change(input, { target: { value: "Explain graph flow" } });
    fireEvent.click(screen.getByRole("button", { name: "Commit staged" }));

    await waitFor(() => expect(git.gitCommit).toHaveBeenCalledWith({ message: "Explain graph flow" }));
    expect(input).toHaveValue("");
  });

  it("renders independent staged and working-tree actions for a partially staged file", async () => {
    git.getGitStatus.mockResolvedValue({
      isRepository: true,
      branch: "feature/partial",
      files: [
        { relativePath: "src/partial.ts", indexStatus: "M", workingTreeStatus: "M", staged: true, conflicted: false },
      ],
    });
    render(<SourceControlPanel workspaceId="workspace-1" onOpenFile={vi.fn()} />);

    expect(await screen.findByRole("button", { name: "Open staged diff for src/partial.ts" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Open working tree diff for src/partial.ts" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Unstage src/partial.ts" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Stage src/partial.ts" })).toBeVisible();
  });

  it("ignores status from an older workspace after the selection changes", async () => {
    let resolveOld: ((value: { isRepository: boolean; branch: string; files: never[] }) => void) | undefined;
    git.getGitStatus
      .mockReset()
      .mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce({ isRepository: true, branch: "new-workspace", files: [] });
    const { rerender } = render(<SourceControlPanel workspaceId="old" onOpenFile={vi.fn()} />);
    rerender(<SourceControlPanel workspaceId="new" onOpenFile={vi.fn()} />);

    expect(await screen.findByText("new-workspace")).toBeVisible();
    await act(async () => resolveOld?.({ isRepository: true, branch: "old-workspace", files: [] }));
    expect(screen.queryByText("old-workspace")).not.toBeInTheDocument();
    expect(screen.getByText("new-workspace")).toBeVisible();
  });

  it("keeps cached changes visible while a watcher refresh is pending", async () => {
    let resolveRefresh: ((value: {
      isRepository: boolean;
      branch: string;
      files: Array<{
        relativePath: string;
        indexStatus: string;
        workingTreeStatus: string;
        staged: boolean;
        conflicted: boolean;
      }>;
    }) => void) | undefined;
    git.getGitStatus.mockReset().mockImplementation(
      () => new Promise((resolve) => { resolveRefresh = resolve; }),
    );
    const cached = {
      isRepository: true,
      branch: "cached-branch",
      files: [
        { relativePath: "src/cached.ts", indexStatus: " ", workingTreeStatus: "M", staged: false, conflicted: false },
      ],
    };
    const { rerender } = render(
      <SourceControlPanel workspaceId="workspace-1" initialStatus={cached} refreshToken={0} onOpenFile={vi.fn()} />,
    );

    expect(screen.getByRole("button", { name: "Open working tree diff for src/cached.ts" })).toBeVisible();
    rerender(
      <SourceControlPanel workspaceId="workspace-1" initialStatus={cached} refreshToken={1} onOpenFile={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Open working tree diff for src/cached.ts" })).toBeVisible();

    await act(async () => resolveRefresh?.({
      isRepository: true,
      branch: "refreshed-branch",
      files: [
        { relativePath: "src/refreshed.ts", indexStatus: " ", workingTreeStatus: "M", staged: false, conflicted: false },
      ],
    }));
    expect(await screen.findByRole("button", { name: "Open working tree diff for src/refreshed.ts" })).toBeVisible();
  });

  it("keeps the newest selected diff when an older request resolves last", async () => {
    let resolveOld: ((value: {
      relativePath: string;
      staged: boolean;
      content: string;
      truncated: boolean;
      comparisonTruncated: boolean;
    }) => void) | undefined;
    git.getGitDiff
      .mockReset()
      .mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }))
      .mockResolvedValueOnce({
        relativePath: "src/staged.ts",
        staged: true,
        content: "newest diff",
        truncated: false,
        comparisonTruncated: false,
      });
    const onOpenDiff = vi.fn();
    render(<SourceControlPanel workspaceId="workspace-1" onOpenFile={vi.fn()} onOpenDiff={onOpenDiff} />);
    fireEvent.click(await screen.findByRole("button", { name: "Open working tree diff for src/changed.ts" }));
    fireEvent.click(screen.getByRole("button", { name: "Open staged diff for src/staged.ts" }));

    await waitFor(() => expect(onOpenDiff).toHaveBeenCalledWith(expect.objectContaining({ content: "newest diff" })));
    await act(async () => resolveOld?.({
      relativePath: "src/changed.ts",
      staged: false,
      content: "stale diff",
      truncated: false,
      comparisonTruncated: false,
    }));
    expect(onOpenDiff).not.toHaveBeenCalledWith(expect.objectContaining({ content: "stale diff" }));
    expect(onOpenDiff).toHaveBeenLastCalledWith(expect.objectContaining({ content: "newest diff" }));
  });
});
