import { demoWorkspace } from "../data/demo";
import type {
  CloneGithubRepositoryRequest,
  CreateDocumentsProjectRequest,
  GitOnboardingReport,
  OnboardingWorkspaceResult,
} from "../features/onboarding/model";
import { isTauriRuntime } from "./bridge";

const DEMO_DELAY_MS = 90;

async function invokeDesktop<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

function waitForDemo(): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, DEMO_DELAY_MS));
}

function demoResult(
  action: OnboardingWorkspaceResult["action"],
  name: string,
  repositoryUrl?: string,
): OnboardingWorkspaceResult {
  return {
    action,
    destinationHint: `Documents/${name}`,
    repositoryUrl,
    workspace: {
      ...demoWorkspace,
      id: `browser:${action}:${name}`,
      name,
      rootPath: `Documents/${name}`,
      lastScannedAt: "2026-08-17T00:00:00.000Z",
    },
  };
}

export async function cloneGithubRepository(
  request: CloneGithubRepositoryRequest,
): Promise<OnboardingWorkspaceResult> {
  if (isTauriRuntime()) {
    return invokeDesktop("clone_github_repository", { request });
  }
  await waitForDemo();
  return demoResult("clone", request.destinationName, request.repositoryUrl);
}

export async function createDocumentsProject(
  request: CreateDocumentsProjectRequest,
): Promise<OnboardingWorkspaceResult> {
  if (isTauriRuntime()) {
    return invokeDesktop("create_documents_project", { request });
  }
  await waitForDemo();
  return demoResult("create", request.projectName);
}

export async function inspectGitOnboarding(
  request: { workspaceId: string },
): Promise<GitOnboardingReport | null> {
  if (isTauriRuntime()) {
    return invokeDesktop("inspect_git_onboarding", { request });
  }
  await waitForDemo();
  return {
    workspaceId: request.workspaceId,
    inspectedAt: "2026-08-17T00:00:00.000Z",
    isRepository: true,
    branch: "main",
    identity: { source: "global" },
    remotes: [{
      name: "origin",
      displayUrl: "https://github.com/aone-labs/browser-demo.git",
      host: "github.com",
      ownerRepo: "aone-labs/browser-demo",
      transport: "https",
    }],
    publicKeys: [],
    privateKeysRead: false,
  };
}
