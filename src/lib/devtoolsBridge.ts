import type { GitDiffResult, GitMutationResult, GitStatusResult } from "../features/source-control/model";
import type { ToolConfiguration, ToolConfigurationOpenResult } from "../features/tool-config/model";

function desktopRuntime() {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

const demoStatus: GitStatusResult = {
  isRepository: true,
  branch: "main",
  head: "browser-demo",
  files: [
    { relativePath: "src/api/services/CheckoutService.ts", indexStatus: " ", workingTreeStatus: "M", staged: false, conflicted: false },
    { relativePath: "src/web/CheckoutPage.tsx", indexStatus: "A", workingTreeStatus: " ", staged: true, conflicted: false },
  ],
};

export async function getGitStatus(): Promise<GitStatusResult> {
  return desktopRuntime() ? invokeDesktop("get_git_status") : demoStatus;
}

export async function getGitDiff(request: { relativePath: string; staged: boolean }): Promise<GitDiffResult> {
  if (desktopRuntime()) return invokeDesktop("get_git_diff", { request });
  return {
    ...request,
    content: "@@ -12,2 +12,3 @@\n const trace = startTrace();\n+trace.observe(\"checkout\");",
    truncated: false,
    originalContent: "const trace = startTrace();\n",
    modifiedContent: "const trace = startTrace();\ntrace.observe(\"checkout\");\n",
    comparisonTruncated: false,
  };
}

function mutation(command: string, request?: Record<string, unknown>): Promise<GitMutationResult> {
  return desktopRuntime() ? invokeDesktop(command, request ? { request } : undefined) : Promise.resolve({ ok: true, summary: "Browser demo: no Git state changed" });
}

export const gitStage = (request: { paths: string[] }) => mutation("git_stage", request);
export const gitUnstage = (request: { paths: string[] }) => mutation("git_unstage", request);
export const gitCommit = (request: { message: string }) => mutation("git_commit", request);
export const gitInitialize = () => mutation("git_initialize");

const demoTools: ToolConfiguration[] = [
  { id: "codex", label: "Codex CLI and MCP", company: "OpenAI", kind: "cli", scope: "home", pathHint: "~/.codex/config.toml", configurationState: "notInspected", cliState: "notInspected" },
  { id: "claude-desktop", label: "Claude Desktop MCP", company: "Anthropic", kind: "mcp", scope: "home", pathHint: "~/Library/Application Support/Claude/claude_desktop_config.json", configurationState: "notInspected", cliState: "notInspected" },
  { id: "cursor", label: "Cursor MCP", company: "Anysphere", kind: "mcp", scope: "home", pathHint: "~/.cursor/mcp.json", configurationState: "notInspected", cliState: "notInspected" },
  { id: "gemini-cli", label: "Gemini CLI and MCP", company: "Google", kind: "cli", scope: "home", pathHint: "~/.gemini/settings.json", configurationState: "notInspected", cliState: "notInspected" },
  { id: "vscode-workspace-mcp", label: "VS Code workspace MCP", company: "Microsoft", kind: "mcp", scope: "workspace", pathHint: ".vscode/mcp.json", configurationState: "notInspected", cliState: "notInspected" },
  { id: "workspace-agents-md", label: "Codex workspace instructions", company: "OpenAI", kind: "agent", scope: "workspace", pathHint: "AGENTS.md", configurationState: "notInspected", cliState: "notInspected" },
  { id: "claude-workspace-instructions", label: "Claude project instructions", company: "Anthropic", kind: "instructions", scope: "workspace", pathHint: "CLAUDE.md", configurationState: "notInspected", cliState: "notInspected" },
  { id: "gemini-workspace-instructions", label: "Gemini project instructions", company: "Google", kind: "instructions", scope: "workspace", pathHint: "GEMINI.md", configurationState: "notInspected", cliState: "notInspected" },
  { id: "cursor-project-rule", label: "Cursor project rule", company: "Anysphere", kind: "rules", scope: "workspace", pathHint: ".cursor/rules/aone-project.mdc", configurationState: "notInspected", cliState: "notInspected" },
  { id: "copilot-workspace-instructions", label: "GitHub Copilot instructions", company: "GitHub", kind: "instructions", scope: "workspace", pathHint: ".github/copilot-instructions.md", configurationState: "notInspected", cliState: "notInspected" },
  { id: "portable-project-skill", label: "Portable project skill", company: "Agent Skills", kind: "skill", scope: "workspace", pathHint: ".agents/skills/aone-project/SKILL.md", configurationState: "notInspected", cliState: "notInspected" },
];

export async function listToolConfigurations(): Promise<ToolConfiguration[]> {
  return desktopRuntime()
    ? invokeDesktop("list_tool_configurations")
    : demoTools.map((tool) => ({ ...tool }));
}

export async function inspectToolConfigurations(): Promise<ToolConfiguration[]> {
  if (desktopRuntime()) return invokeDesktop("inspect_tool_configurations");
  const configured = new Set(["codex", "cursor", "workspace-agents-md"]);
  const cliFound = new Set(["codex", "cursor", "gemini-cli", "workspace-agents-md", "gemini-workspace-instructions", "cursor-project-rule"]);
  return demoTools.map((tool) => ({
    ...tool,
    configurationState: configured.has(tool.id) ? "found" : "notFound",
    cliState: cliFound.has(tool.id) ? "found" : "notFound",
  }));
}

export async function openToolConfiguration(request: { toolId: string; createIfMissing: boolean }): Promise<ToolConfigurationOpenResult> {
  if (desktopRuntime()) return invokeDesktop("open_tool_configuration", { request });
  return { opened: true, created: request.createIfMissing, pathHint: demoTools.find((tool) => tool.id === request.toolId)?.pathHint ?? "browser demo" };
}
