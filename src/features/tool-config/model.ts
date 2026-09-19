export type ToolConfigurationKind = "mcp" | "cli" | "agent" | "skill" | "instructions" | "rules";
export type ToolInspectionState = "notInspected" | "found" | "notFound";
export type ToolConfigurationScope = "home" | "workspace";

export interface ToolConfiguration {
  id: string;
  label: string;
  company: string;
  kind: ToolConfigurationKind;
  scope: ToolConfigurationScope;
  pathHint: string;
  configurationState: ToolInspectionState;
  cliState: ToolInspectionState;
}

export interface ToolConfigurationOpenResult {
  opened: boolean;
  created: boolean;
  pathHint: string;
}
