import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WorkspaceSummary } from "../../types";
import { OnboardingPanel } from "./OnboardingPanel";

const workspace: WorkspaceSummary = {
  id: "workspace:a",
  name: "alpha-service",
  rootPath: "/Users/example/Documents/alpha-service",
  fileCount: 12,
  nodeCount: 30,
  edgeCount: 21,
  languages: [],
  lastScannedAt: "2026-08-17T00:00:00Z",
};

function props(activeWorkspace: WorkspaceSummary | null = null) {
  return {
    workspace: activeWorkspace,
    workspaceGeneration: 1,
    workspaceBusy: false,
    aiEnv: null,
    aiEnvLoading: false,
    onOpenFolder: vi.fn().mockResolvedValue(undefined),
    onClone: vi.fn().mockResolvedValue({
      status: "success",
      result: { action: "clone", workspace, destinationHint: "~/Documents/terax-ai" },
    }),
    onCreate: vi.fn().mockResolvedValue({
      status: "success",
      result: { action: "create", workspace, destinationHint: "~/Documents/notes" },
    }),
    onPickAiEnv: vi.fn().mockResolvedValue(undefined),
    onOpenTools: vi.fn(),
    onOpenProjectAgent: vi.fn(),
    onExit: vi.fn(),
  };
}

describe("Getting Started onboarding", () => {
  it("focuses the heading and makes no native-style action on mount", async () => {
    const handlers = props();
    render(<OnboardingPanel {...handlers} />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "Set up a local Aone workspace" })).toHaveFocus());
    expect(handlers.onOpenFolder).not.toHaveBeenCalled();
    expect(handlers.onClone).not.toHaveBeenCalled();
    expect(handlers.onCreate).not.toHaveBeenCalled();
    expect(handlers.onPickAiEnv).not.toHaveBeenCalled();
  });

  it("derives the clone destination and submits the frozen request payload", async () => {
    const handlers = props();
    render(<OnboardingPanel {...handlers} />);
    fireEvent.click(screen.getByRole("button", { name: /Clone public GitHub repository/ }));
    fireEvent.change(screen.getByLabelText("Public GitHub repository URL"), {
      target: { value: "https://github.com/crynta/terax-ai.git" },
    });
    expect(screen.getByText("Documents/terax-ai")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Clone to Documents" }));
    await waitFor(() => expect(handlers.onClone).toHaveBeenCalledWith({
      repositoryUrl: "https://github.com/crynta/terax-ai",
      destinationName: "terax-ai",
    }));
    expect(await screen.findByText("Repository cloned and opened")).toBeVisible();
  });

  it("creates a Documents project with Git initialization selected by default", async () => {
    const handlers = props();
    render(<OnboardingPanel {...handlers} />);
    fireEvent.click(screen.getByRole("button", { name: /Create new project/ }));
    expect(screen.getByRole("checkbox", { name: /Initialize Git repository/ })).toBeChecked();
    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "notes" } });
    fireEvent.click(screen.getByRole("button", { name: "Create in Documents" }));
    await waitFor(() => expect(handlers.onCreate).toHaveBeenCalledWith({
      projectName: "notes",
      initializeGit: true,
    }));
  });

  it("shows only env names, has no secret input, and keeps send consent separate", () => {
    render(<OnboardingPanel {...props()} />);
    fireEvent.click(screen.getByRole("tab", { name: "AI Provider" }));
    expect(screen.getByText(/AONE_AI_PROVIDER=openai/)).toBeVisible();
    expect(screen.getByText(/OPENAI_API_KEY=/)).toBeVisible();
    expect(screen.getByText(/Provider network consent remains a separate action/)).toBeVisible();
    expect(document.querySelector('input[type="password"]')).not.toBeInTheDocument();
    expect(document.querySelector('input[name*="KEY" i]')).not.toBeInTheDocument();
  });

  it("routes to existing tool and project panels without inspecting", () => {
    const handlers = props(workspace);
    render(<OnboardingPanel {...handlers} />);
    fireEvent.click(screen.getByRole("tab", { name: "AI Tools" }));
    fireEvent.click(screen.getByRole("button", { name: "Open MCP and CLI configuration" }));
    expect(handlers.onOpenTools).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Open Project Agent" }));
    expect(handlers.onOpenProjectAgent).toHaveBeenCalledTimes(1);
  });

  it("supports roving tab keys, Escape form return, and Escape close with focus restoration delegated", async () => {
    const handlers = props(workspace);
    render(<OnboardingPanel {...handlers} />);
    const workspaceTab = screen.getByRole("tab", { name: /Workspace/ });
    workspaceTab.focus();
    fireEvent.keyDown(workspaceTab, { key: "ArrowDown" });
    await waitFor(() => expect(screen.getByRole("tab", { name: "AI Provider" })).toHaveFocus());
    fireEvent.click(workspaceTab);
    fireEvent.click(screen.getByRole("button", { name: /Clone public GitHub repository/ }));
    fireEvent.keyDown(screen.getByLabelText("Public GitHub repository URL"), { key: "Escape" });
    await waitFor(() => expect(screen.getByRole("button", { name: /Clone public GitHub repository/ })).toHaveFocus());
    fireEvent.keyDown(screen.getByRole("tabpanel"), { key: "Escape" });
    expect(handlers.onExit).toHaveBeenCalledTimes(1);
  });
});
