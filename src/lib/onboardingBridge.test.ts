import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceSummary } from "../types";

const runtime = vi.hoisted(() => ({ desktop: true, invoke: vi.fn() }));
vi.mock("./bridge", () => ({ isTauriRuntime: () => runtime.desktop }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: runtime.invoke }));

import {
  cloneGithubRepository,
  createDocumentsProject,
  inspectGitOnboarding,
} from "./onboardingBridge";

const workspace: WorkspaceSummary = {
  id: "workspace:new",
  name: "new",
  rootPath: "/Users/example/Documents/new",
  fileCount: 0,
  nodeCount: 0,
  edgeCount: 0,
  languages: [],
  lastScannedAt: "2026-08-17T00:00:00Z",
};

beforeEach(() => {
  runtime.desktop = true;
  runtime.invoke.mockReset();
});

describe("onboarding native bridge", () => {
  it("sends exact nested clone and create payloads", async () => {
    runtime.invoke
      .mockResolvedValueOnce({ action: "clone", workspace, destinationHint: "~/Documents/new" })
      .mockResolvedValueOnce({ action: "create", workspace, destinationHint: "~/Documents/new" });

    await cloneGithubRepository({
      repositoryUrl: "https://github.com/example/new",
      destinationName: "new",
    });
    await createDocumentsProject({ projectName: "new", initializeGit: true });

    expect(runtime.invoke).toHaveBeenNthCalledWith(1, "clone_github_repository", {
      request: { repositoryUrl: "https://github.com/example/new", destinationName: "new" },
    });
    expect(runtime.invoke).toHaveBeenNthCalledWith(2, "create_documents_project", {
      request: { projectName: "new", initializeGit: true },
    });
  });

  it("passes only the workspace identity into Git inspection", async () => {
    runtime.invoke.mockResolvedValue(null);
    await inspectGitOnboarding({ workspaceId: "workspace:new" });
    expect(runtime.invoke).toHaveBeenCalledWith("inspect_git_onboarding", {
      request: { workspaceId: "workspace:new" },
    });
  });

  it("uses a deterministic browser demo without invoking the host", async () => {
    runtime.desktop = false;
    const result = await cloneGithubRepository({
      repositoryUrl: "https://github.com/example/browser-project",
      destinationName: "browser-project",
    });
    expect(result).toMatchObject({
      action: "clone",
      destinationHint: "Documents/browser-project",
      workspace: { id: "browser:clone:browser-project" },
    });
    expect(runtime.invoke).not.toHaveBeenCalled();
  });
});
