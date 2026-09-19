import { describe, expect, it } from "vitest";
import {
  getGitDiff,
  getGitStatus,
  gitCommit,
  gitInitialize,
  gitStage,
  gitUnstage,
  inspectToolConfigurations,
  listToolConfigurations,
  openToolConfiguration,
} from "./devtoolsBridge";

describe("developer-tools browser demo", () => {
  it("returns a readable local Git status and bounded demo diff", async () => {
    const status = await getGitStatus();
    expect(status).toMatchObject({ isRepository: true, branch: "main", head: "browser-demo" });
    expect(status.files).toEqual(expect.arrayContaining([
      expect.objectContaining({ relativePath: "src/api/services/CheckoutService.ts", staged: false }),
      expect.objectContaining({ relativePath: "src/web/CheckoutPage.tsx", staged: true }),
    ]));

    await expect(getGitDiff({ relativePath: "src/api/services/CheckoutService.ts", staged: false }))
      .resolves.toMatchObject({ staged: false, truncated: false, content: expect.stringContaining("trace.observe") });
  });

  it("models Git mutations as explicit no-op acknowledgements", async () => {
    const actions = await Promise.all([
      gitStage({ paths: ["src/example.ts"] }),
      gitUnstage({ paths: ["src/example.ts"] }),
      gitCommit({ message: "Test commit" }),
      gitInitialize(),
    ]);
    expect(actions).toHaveLength(4);
    expect(actions.every((action) => action.ok && action.summary.includes("no Git state changed"))).toBe(true);
  });

  it("lists static tool metadata before explicit inspection", async () => {
    const tools = await listToolConfigurations();
    expect(tools.map((tool) => tool.id)).toEqual([
      "codex",
      "claude-desktop",
      "cursor",
      "gemini-cli",
      "vscode-workspace-mcp",
      "workspace-agents-md",
      "claude-workspace-instructions",
      "gemini-workspace-instructions",
      "cursor-project-rule",
      "copilot-workspace-instructions",
      "portable-project-skill",
    ]);
    expect(tools.every((tool) => tool.configurationState === "notInspected" && tool.cliState === "notInspected")).toBe(true);
    expect(tools.map((tool) => tool.kind)).toEqual(expect.arrayContaining(["mcp", "cli", "agent", "skill", "instructions", "rules"]));
  });

  it("models inspection separately from open and template creation", async () => {
    const inspected = await inspectToolConfigurations();
    expect(inspected.find((tool) => tool.id === "codex")).toMatchObject({
      configurationState: "found",
      cliState: "found",
    });
    expect(inspected.find((tool) => tool.id === "claude-desktop")).toMatchObject({
      configurationState: "notFound",
      cliState: "notFound",
    });

    await expect(openToolConfiguration({ toolId: "codex", createIfMissing: false }))
      .resolves.toEqual({ opened: true, created: false, pathHint: "~/.codex/config.toml" });
    await expect(openToolConfiguration({ toolId: "claude-desktop", createIfMissing: true }))
      .resolves.toMatchObject({ opened: true, created: true, pathHint: expect.stringContaining("Claude") });
  });
});
