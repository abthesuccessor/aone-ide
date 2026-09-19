import type { WorkspaceSummary } from "../../types";

export type OnboardingSection = "workspace" | "ai" | "tools" | "git";
export type AiProvider = "openai" | "anthropic";

export interface CloneGithubRepositoryRequest {
  repositoryUrl: string;
  destinationName: string;
}

export interface CreateDocumentsProjectRequest {
  projectName: string;
  initializeGit: boolean;
}

export interface OnboardingWorkspaceResult {
  action: "clone" | "create";
  workspace: WorkspaceSummary;
  destinationHint: string;
  repositoryUrl?: string;
}

export interface GitRemoteSummary {
  name: string;
  displayUrl: string;
  host?: string;
  ownerRepo?: string;
  transport: "https" | "ssh" | "other" | "local";
}

export interface GitPublicKeySummary {
  fileName: string;
  keyType: string;
  fingerprint: string;
}

export interface GitOnboardingReport {
  workspaceId: string;
  inspectedAt: string;
  isRepository: boolean;
  branch?: string;
  identity?: {
    name?: string;
    email?: string;
    source: "local" | "global";
  };
  remotes: GitRemoteSummary[];
  publicKeys: GitPublicKeySummary[];
  privateKeysRead: false;
}

export type WorkspaceActionOutcome =
  | { status: "success"; result: OnboardingWorkspaceResult }
  | { status: "cancelled" | "stale" }
  | { status: "error"; message: string };

export const AI_ENVIRONMENTS: Record<AiProvider, {
  label: string;
  fileName: string;
  names: readonly string[];
}> = {
  openai: {
    label: "OpenAI",
    fileName: ".env.aone.openai",
    names: ["AONE_AI_PROVIDER", "OPENAI_API_KEY", "OPENAI_MODEL"],
  },
  anthropic: {
    label: "Anthropic",
    fileName: ".env.aone.anthropic",
    names: ["AONE_AI_PROVIDER", "ANTHROPIC_API_KEY", "ANTHROPIC_MODEL"],
  },
};

export function githubDestination(raw: string): { repositoryUrl: string; destinationName: string } {
  const value = raw.trim();
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error("Enter a public GitHub URL such as https://github.com/owner/repository");
  }
  if (parsed.protocol !== "https:" || parsed.hostname.toLowerCase() !== "github.com"
    || parsed.username || parsed.password || parsed.search || parsed.hash) {
    throw new Error("Use an HTTPS github.com URL without credentials, query text, or a fragment");
  }
  const segments = parsed.pathname.split("/").filter(Boolean);
  if (segments.length !== 2) {
    throw new Error("Enter a repository URL with an owner and repository name");
  }
  const destinationName = segments[1]!.replace(/\.git$/i, "");
  if (!/^[A-Za-z0-9._-]{1,100}$/.test(destinationName) || destinationName === "." || destinationName === "..") {
    throw new Error("The repository name cannot be used as a local folder name");
  }
  return {
    repositoryUrl: `https://github.com/${segments[0]}/${destinationName}`,
    destinationName,
  };
}

export function validProjectName(raw: string): string {
  const value = raw.trim();
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,99}$/.test(value) || value.endsWith(".")) {
    throw new Error("Use 1 to 100 ASCII letters, numbers, dots, dashes, or underscores. Do not start or end with a dot");
  }
  return value;
}
