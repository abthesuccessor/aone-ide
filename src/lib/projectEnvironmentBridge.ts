import type { ProjectEnvironmentReport } from "../features/project-environment/model";
import type { AiExplanation, EvidenceReference } from "../types";

export interface ProjectAgentQuestion {
  workspaceId: string;
  reportId: string;
  question: string;
}

export type ProjectAgentResponse = {
  status: "completed";
  answer: string;
  evidence: EvidenceReference[];
  model: string;
} | {
  status: "error";
  code: "cancelled" | "notConfigured" | "invalidRequest" | "workspaceChanged" | "providerFailed";
  message: string;
  retryable: boolean;
};

function desktopRuntime() {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

const demoReport: ProjectEnvironmentReport = {
  reportId: "environment:browser-demo",
  workspaceId: "workspace:demo-checkout",
  inspectedAt: "2026-08-17T08:30:00.000Z",
  versionProbeApproved: true,
  stacks: [
    {
      id: "typescript-node",
      label: "TypeScript + Node.js",
      confidence: "confirmed",
      evidence: [
        { relativePath: "package.json", detail: "Node scripts and TypeScript dependencies" },
        { relativePath: "src/web/CheckoutPage.tsx", detail: "TypeScript React source" },
      ],
    },
    {
      id: "docker-compose",
      label: "Docker Compose",
      confidence: "confirmed",
      evidence: [{ relativePath: "docker-compose.yml", detail: "API and PostgreSQL services" }],
    },
  ],
  tools: [
    {
      id: "node", label: "Node.js", category: "JavaScript", required: true,
      status: "available", canonicalPath: "/opt/homebrew/bin/node",
      alternateCanonicalPaths: [], version: "v24.5.0", versionArgs: ["--version"],
      usedBy: ["typescript-node"],
    },
    {
      id: "npm", label: "npm", category: "JavaScript", required: true,
      status: "available", canonicalPath: "/opt/homebrew/bin/npm",
      alternateCanonicalPaths: [], version: "11.5.1", versionArgs: ["--version"],
      usedBy: ["typescript-node"],
    },
    {
      id: "docker", label: "Docker", category: "Containers", required: false,
      status: "missing", alternateCanonicalPaths: [], versionArgs: ["--version"],
      usedBy: ["docker-compose"],
    },
  ],
  recommendations: [
    {
      id: "run-web-dev", title: "Use Web · dev", kind: "run-profile",
      severity: "recommended", runProfileId: "web-dev",
      summary: "The existing run profile matches the detected Node.js workspace.",
      evidence: [{ relativePath: "package.json", detail: "dev:web script" }],
    },
    {
      id: "install-docker", title: "Docker is optional", kind: "tool",
      severity: "optional", summary: "Install Docker only when you need the composed PostgreSQL service.",
      evidence: [{ relativePath: "docker-compose.yml", detail: "Local service definition" }],
    },
  ],
};

const demoReports = new Map<string, ProjectEnvironmentReport>();

export async function getProjectEnvironmentReport(
  workspaceId: string,
): Promise<ProjectEnvironmentReport | null> {
  if (desktopRuntime()) {
    return invokeDesktop("get_project_environment_report", { request: { workspaceId } });
  }
  return demoReports.get(workspaceId) ?? null;
}

export async function inspectProjectEnvironment(
  workspaceId: string,
): Promise<ProjectEnvironmentReport | null> {
  if (desktopRuntime()) {
    return invokeDesktop("inspect_project_environment", { request: { workspaceId } });
  }
  await new Promise((resolve) => window.setTimeout(resolve, 120));
  const report = { ...demoReport, workspaceId, inspectedAt: new Date().toISOString() };
  demoReports.set(workspaceId, report);
  return report;
}

export async function explainProjectEnvironment(
  workspaceId: string,
  reportId: string,
): Promise<AiExplanation> {
  if (desktopRuntime()) {
    return invokeDesktop("ai_explain_project_environment", {
      request: { workspaceId, reportId },
    });
  }
  await new Promise((resolve) => window.setTimeout(resolve, 120));
  return {
    answer: "This project uses a confirmed TypeScript and Node.js stack. The Web · dev profile is the closest registered local run path. Docker is optional unless you want to start the composed PostgreSQL service.",
    evidence: [
      { kind: "projectStack", id: "typescript-node", label: "TypeScript + Node.js", evidence: "declared" },
      { kind: "runProfile", id: "web-dev", label: "Web · dev", evidence: "resolved" },
    ],
    model: "browser-demo",
  };
}

export async function askProjectAgent(
  request: ProjectAgentQuestion,
): Promise<ProjectAgentResponse> {
  if (desktopRuntime()) {
    return invokeDesktop("ai_ask_project_agent", { request });
  }
  await new Promise((resolve) => window.setTimeout(resolve, 180));
  const normalized = request.question.toLowerCase();
  const answer = normalized.includes("env")
    ? "Keep AI-provider keys separate from run-time service variables. Use Env in the title bar to load a run .env file by name; Aone keeps values in native memory and never adds them to model evidence."
    : normalized.includes("docker")
      ? "Docker is optional in the inspected browser project. I can explain the declared Compose topology, but starting containers still requires a dedicated reviewed action."
      : "Start with the registered Web · dev profile. Aone can select that IDE profile after approval; you decide when Run or Debug may start it and whether external services should be prepared.";
  return {
    status: "completed",
    answer,
    evidence: [
      { kind: "projectEnvironment", id: request.reportId, label: "Saved project inspection", evidence: "resolved" },
    ],
    model: "browser-demo",
  };
}
