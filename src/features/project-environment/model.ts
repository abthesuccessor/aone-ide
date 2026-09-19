export type ProjectStackConfidence = "confirmed" | "inferred";
export type ProjectToolStatus = "available" | "missing" | "unverified";
export type ProjectEnvironmentRecommendationKind = "run-profile" | "tool" | "environment";
export type ProjectEnvironmentRecommendationSeverity = "recommended" | "optional" | "warning";

export interface ProjectEnvironmentEvidence {
  relativePath: string;
  detail: string;
}

export interface ProjectStack {
  id: string;
  label: string;
  confidence: ProjectStackConfidence;
  evidence: ProjectEnvironmentEvidence[];
}

export interface ProjectTool {
  id: string;
  label: string;
  category: string;
  required: boolean;
  status: ProjectToolStatus;
  canonicalPath?: string;
  alternateCanonicalPaths: string[];
  version?: string;
  versionArgs: string[];
  probeError?: string;
  usedBy: string[];
}

export interface ProjectEnvironmentRecommendation {
  id: string;
  title: string;
  summary: string;
  kind: ProjectEnvironmentRecommendationKind;
  severity: ProjectEnvironmentRecommendationSeverity;
  evidence: ProjectEnvironmentEvidence[];
  runProfileId?: string;
}

export interface ProjectEnvironmentReport {
  reportId: string;
  workspaceId: string;
  inspectedAt: string;
  versionProbeApproved: boolean;
  stacks: ProjectStack[];
  tools: ProjectTool[];
  recommendations: ProjectEnvironmentRecommendation[];
}
