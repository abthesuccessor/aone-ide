import type { ProjectEnvironmentReport } from "../project-environment/model";

export type AgentSetupStepState = "awaiting-approval" | "already-applied" | "needs-engineer";

export interface AgentRunProfileSummary {
  id: string;
  name: string;
}

export interface AgentSetupPlanStep {
  id: string;
  title: string;
  summary: string;
  evidence: string[];
  state: AgentSetupStepState;
  action?: {
    kind: "select-run-profile";
    profileId: string;
    profileName: string;
  };
}

export interface AgentSetupPlan {
  id: string;
  reportId: string;
  declaredStackCount: number;
  detectedLocalToolCount: number;
  steps: AgentSetupPlanStep[];
}

function evidenceLabels(
  evidence: ProjectEnvironmentReport["recommendations"][number]["evidence"],
): string[] {
  return evidence.slice(0, 4).map((item) => `${item.relativePath} · ${item.detail}`);
}

/**
 * Turns only deterministic native recommendations into an approval plan.
 * AI prose is deliberately not accepted here: model output never becomes an
 * executable action.
 */
export function buildAgentSetupPlan(
  report: ProjectEnvironmentReport,
  runProfiles: AgentRunProfileSummary[],
  activeRunProfileId: string,
): AgentSetupPlan {
  const profiles = new Map(runProfiles.map((profile) => [profile.id, profile]));
  const steps = report.recommendations.map((recommendation): AgentSetupPlanStep => {
    const profileId = recommendation.runProfileId;
    const profile = profileId ? profiles.get(profileId) : undefined;
    if (profileId && profile) {
      const applied = activeRunProfileId === profileId;
      return {
        id: recommendation.id,
        title: recommendation.title,
        summary: recommendation.summary,
        evidence: evidenceLabels(recommendation.evidence),
        state: applied ? "already-applied" : "awaiting-approval",
        action: {
          kind: "select-run-profile",
          profileId,
          profileName: profile.name,
        },
      };
    }
    return {
      id: recommendation.id,
      title: recommendation.title,
      summary: recommendation.summary,
      evidence: evidenceLabels(recommendation.evidence),
      state: "needs-engineer",
    };
  });

  return {
    id: `agent-setup:${report.reportId}`,
    reportId: report.reportId,
    declaredStackCount: report.stacks.length,
    detectedLocalToolCount: report.tools.filter(
      (tool) => tool.status === "available"
        || (tool.status === "unverified" && Boolean(tool.canonicalPath)),
    ).length,
    steps,
  };
}

export function countApprovableAgentSteps(plan: AgentSetupPlan): number {
  return plan.steps.filter((step) => step.state === "awaiting-approval" && step.action).length;
}
