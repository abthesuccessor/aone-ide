import { describe, expect, it } from "vitest";
import type { ProjectEnvironmentReport } from "../project-environment/model";
import { buildAgentSetupPlan, countApprovableAgentSteps } from "./model";

const report: ProjectEnvironmentReport = {
  reportId: "report-1",
  workspaceId: "workspace-1",
  inspectedAt: "2026-09-05T00:00:00.000Z",
  versionProbeApproved: true,
  stacks: [{
    id: "python",
    label: "Python",
    confidence: "confirmed",
    evidence: [{ relativePath: "pyproject.toml", detail: "Python project manifest" }],
  }],
  tools: [
    {
      id: "python", label: "Python", category: "Python", required: true,
      status: "available", canonicalPath: "/usr/bin/python3",
      alternateCanonicalPaths: [], versionArgs: ["--version"], usedBy: ["python"],
    },
    {
      id: "docker", label: "Docker", category: "Containers", required: false,
      status: "missing", alternateCanonicalPaths: [], versionArgs: ["--version"], usedBy: [],
    },
    {
      id: "uv", label: "uv", category: "Python", required: true,
      status: "unverified", canonicalPath: "/opt/homebrew/bin/uv",
      alternateCanonicalPaths: [], versionArgs: ["--version"], usedBy: ["python"],
    },
  ],
  recommendations: [
    {
      id: "profile-api",
      title: "Use API profile",
      summary: "A registered profile matches the service.",
      kind: "run-profile",
      severity: "recommended",
      evidence: [{ relativePath: "pyproject.toml", detail: "Registered dev task" }],
      runProfileId: "api-dev",
    },
    {
      id: "postgres",
      title: "Check PostgreSQL",
      summary: "The service may need PostgreSQL.",
      kind: "environment",
      severity: "warning",
      evidence: [{ relativePath: "src/settings.py", detail: "PostgreSQL dependency hint" }],
    },
  ],
};

describe("buildAgentSetupPlan", () => {
  it("permits only an exact registered run-profile selection", () => {
    const plan = buildAgentSetupPlan(report, [{ id: "api-dev", name: "API development" }], "");

    expect(plan.steps[0]).toMatchObject({
      state: "awaiting-approval",
      action: { kind: "select-run-profile", profileId: "api-dev", profileName: "API development" },
    });
    expect(plan.steps[1]).toMatchObject({ state: "needs-engineer" });
    expect(plan.steps[1]?.action).toBeUndefined();
    expect(plan.detectedLocalToolCount).toBe(2);
    expect(countApprovableAgentSteps(plan)).toBe(1);
  });

  it("does not turn an unknown profile from project evidence into an action", () => {
    const plan = buildAgentSetupPlan(report, [{ id: "another-profile", name: "Other" }], "");

    expect(plan.steps[0]).toMatchObject({ state: "needs-engineer" });
    expect(plan.steps[0]?.action).toBeUndefined();
    expect(countApprovableAgentSteps(plan)).toBe(0);
  });

  it("marks an exact active profile as already applied", () => {
    const plan = buildAgentSetupPlan(report, [{ id: "api-dev", name: "API development" }], "api-dev");

    expect(plan.steps[0]).toMatchObject({ state: "already-applied" });
    expect(countApprovableAgentSteps(plan)).toBe(0);
  });
});
