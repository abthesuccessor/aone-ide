import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ToolConfiguration } from "./model";
import { ToolConfigPanel } from "./ToolConfigPanel";

const bridge = vi.hoisted(() => ({
  inspectToolConfigurations: vi.fn(),
  listToolConfigurations: vi.fn(),
  openToolConfiguration: vi.fn(),
}));

vi.mock("../../lib/devtoolsBridge", () => bridge);

const staticCatalog: ToolConfiguration[] = [
  { id: "codex", label: "Codex CLI and MCP", company: "OpenAI", kind: "cli", scope: "home", pathHint: "~/.codex/config.toml", configurationState: "notInspected", cliState: "notInspected" },
  { id: "claude-desktop", label: "Claude Desktop MCP", company: "Anthropic", kind: "mcp", scope: "home", pathHint: "~/Library/Application Support/Claude/claude_desktop_config.json", configurationState: "notInspected", cliState: "notInspected" },
  { id: "portable-project-skill", label: "Portable project skill", company: "Agent Skills", kind: "skill", scope: "workspace", pathHint: ".agents/skills/aone-project/SKILL.md", configurationState: "notInspected", cliState: "notInspected" },
];

const inspectedCatalog: ToolConfiguration[] = staticCatalog.map((tool) => ({
  ...tool,
  configurationState: tool.id === "codex" ? "found" : "notFound",
  cliState: tool.id === "codex" ? "found" : "notFound",
}));

beforeEach(() => {
  vi.clearAllMocks();
  bridge.listToolConfigurations.mockResolvedValue(staticCatalog);
  bridge.inspectToolConfigurations.mockResolvedValue(inspectedCatalog);
  bridge.openToolConfiguration.mockResolvedValue({ opened: true, created: false, pathHint: "demo" });
});

async function startInspection() {
  const button = await screen.findByRole("button", { name: "Inspect AI tools" });
  await waitFor(() => expect(button).toBeEnabled());
  fireEvent.click(button);
}

describe("ToolConfigPanel", () => {
  it("loads static categories without inspecting local files or PATH", async () => {
    render(<ToolConfigPanel workspaceId="workspace" />);

    expect(await screen.findByText("Codex CLI and MCP")).toBeVisible();
    expect(screen.getByText("Portable project skill")).toBeVisible();
    expect(screen.getByText("skill")).toBeVisible();
    expect(screen.getAllByText("File not inspected")).toHaveLength(3);
    expect(bridge.listToolConfigurations).toHaveBeenCalledTimes(1);
    expect(bridge.inspectToolConfigurations).not.toHaveBeenCalled();
  });

  it("requires explicit inspection before opening an existing configuration", async () => {
    render(<ToolConfigPanel workspaceId="workspace" />);
    const inspectButton = await screen.findByRole("button", { name: "Inspect AI tools" });
    await waitFor(() => expect(inspectButton).toBeEnabled());
    expect(screen.getByRole("button", { name: "Inspect before configuring Codex CLI and MCP" })).toBeDisabled();

    fireEvent.click(inspectButton);
    await waitFor(() => expect(bridge.inspectToolConfigurations).toHaveBeenCalledTimes(1));
    const openButton = await screen.findByRole("button", { name: "Open config for Codex CLI and MCP" });
    fireEvent.click(openButton);

    await waitFor(() => expect(bridge.openToolConfiguration).toHaveBeenCalledWith({
      toolId: "codex",
      createIfMissing: false,
    }));
  });

  it("creates only the compiled template for an inspected missing artifact", async () => {
    bridge.openToolConfiguration.mockResolvedValueOnce({ opened: true, created: true, pathHint: "demo" });
    render(<ToolConfigPanel workspaceId="workspace" />);
    await startInspection();

    const create = await screen.findByRole("button", { name: "Create template for Claude Desktop MCP" });
    fireEvent.click(create);
    await waitFor(() => expect(bridge.openToolConfiguration).toHaveBeenCalledWith({
      toolId: "claude-desktop",
      createIfMissing: true,
    }));
    expect(await screen.findByRole("button", { name: "Open config for Claude Desktop MCP" })).toBeEnabled();
    expect(bridge.inspectToolConfigurations).toHaveBeenCalledTimes(1);
  });

  it("resets inspection state and ignores a late result after workspace change", async () => {
    let resolveOld: ((value: ToolConfiguration[]) => void) | undefined;
    bridge.inspectToolConfigurations.mockImplementationOnce(() => new Promise((resolve) => {
      resolveOld = resolve;
    }));
    const { rerender } = render(<ToolConfigPanel workspaceId="old" />);
    await startInspection();
    rerender(<ToolConfigPanel workspaceId="new" />);

    await waitFor(() => expect(bridge.listToolConfigurations).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("button", { name: "Awaiting permission…" })).toBeDisabled();
    resolveOld?.([{ ...inspectedCatalog[0]!, label: "Old workspace result" }]);
    await waitFor(() => expect(screen.queryByText("Old workspace result")).not.toBeInTheDocument());
    expect(screen.getByText("Codex CLI and MCP")).toBeVisible();
    expect(screen.getAllByText("File not inspected")).toHaveLength(3);
  });

  it("does not offer workspace artifact creation without an open folder", async () => {
    render(<ToolConfigPanel />);
    await startInspection();
    expect(await screen.findByRole("button", { name: "Open a folder before configuring Portable project skill" })).toBeDisabled();
  });
});
