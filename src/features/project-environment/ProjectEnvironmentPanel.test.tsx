import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectEnvironmentReport } from "./model";
import { ProjectEnvironmentPanel } from "./ProjectEnvironmentPanel";

const bridge = vi.hoisted(() => ({
  explainProjectEnvironment: vi.fn(),
  inspectProjectEnvironment: vi.fn(),
}));

vi.mock("../../lib/projectEnvironmentBridge", () => bridge);

const report: ProjectEnvironmentReport = {
  reportId: "report-a",
  workspaceId: "workspace-a",
  inspectedAt: "2026-08-17T08:30:00.000Z",
  versionProbeApproved: false,
  stacks: [{
    id: "node", label: "TypeScript + Node.js", confidence: "confirmed",
    evidence: [{ relativePath: "package.json", detail: "Node scripts" }],
  }],
  tools: [{
    id: "node", label: "Node.js", category: "JavaScript", required: true,
    status: "unverified", canonicalPath: "/opt/homebrew/bin/node",
    alternateCanonicalPaths: [], versionArgs: ["--version"], usedBy: ["node"],
  }],
  recommendations: [
    {
      id: "run", title: "Use Web · dev", summary: "Matches the detected workspace.",
      kind: "run-profile", severity: "recommended",
      evidence: [{ relativePath: "package.json", detail: "dev script" }],
      runProfileId: "web-dev",
    },
    {
      id: "environment:inferred-names", title: "Inferred environment names (hint only)",
      summary: "Hint only — API_PORT may be understood. No values were collected, and this is not proof it is required.",
      kind: "environment", severity: "optional",
      evidence: [{ relativePath: "src/config.ts", detail: "Names-only hint" }],
    },
  ],
};

const baseProps = {
  workspaceId: "workspace-a",
  workspaceGeneration: 1,
  runProfileIds: ["web-dev"],
  activeRunProfileId: "",
  onUseRunProfile: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  bridge.inspectProjectEnvironment.mockResolvedValue(report);
  bridge.explainProjectEnvironment.mockResolvedValue({
    answer: "Use the registered Node development profile.",
    evidence: [{ kind: "runProfile", id: "web-dev", label: "Web · dev", evidence: "resolved" }],
    model: "browser-demo",
  });
});

describe("ProjectEnvironmentPanel", () => {
  it("does not inspect on mount and discloses both native permission steps", () => {
    render(<ProjectEnvironmentPanel {...baseProps} />);

    expect(bridge.inspectProjectEnvironment).not.toHaveBeenCalled();
    expect(screen.getByText(/first native dialog asks before Aone reads bounded build\/config sources and executable paths/i)).toBeVisible();
    expect(screen.getByText(/second dialog asks before fixed, read-only version probes/i)).toBeVisible();
    expect(screen.getByText(/names-only environment and dependency hints · never values or proven requirements/i)).toBeVisible();
    expect(screen.getByRole("button", { name: "Inspect project environment" })).toBeEnabled();
  });

  it("renders the permission result and selects only a registered run profile", async () => {
    const onUseRunProfile = vi.fn();
    render(<ProjectEnvironmentPanel {...baseProps} onUseRunProfile={onUseRunProfile} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));

    expect(await screen.findByText("TypeScript + Node.js")).toBeVisible();
    expect(bridge.inspectProjectEnvironment).toHaveBeenCalledWith("workspace-a");
    expect(screen.getByText("Version probes skipped by user")).toBeVisible();
    expect(screen.getByText("Path found · version not run")).toBeVisible();
    expect(screen.getByTitle("/opt/homebrew/bin/node")).toBeVisible();
    expect(screen.getByText("Inferred environment names (hint only)")).toBeVisible();
    expect(screen.getByText(/No values were collected, and this is not proof it is required/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Use run profile" }));
    expect(onUseRunProfile).toHaveBeenCalledWith("web-dev");
  });

  it("requests an AI explanation only after a report and labels it inferred", async () => {
    render(<ProjectEnvironmentPanel {...baseProps} />);
    expect(screen.queryByRole("button", { name: "Ask AI to explain setup" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));
    await screen.findByText("TypeScript + Node.js");
    fireEvent.click(screen.getByRole("button", { name: "Ask AI to explain setup" }));

    expect(await screen.findByText(/Use the registered Node development profile/)).toBeVisible();
    expect(bridge.explainProjectEnvironment).toHaveBeenCalledWith("workspace-a", "report-a");
    expect(screen.getByText(/AI output is inferred · browser-demo/)).toBeVisible();
    expect(screen.getByText("Web · dev · resolved")).toBeVisible();
  });

  it("clears and ignores an inspection that resolves after the workspace changes", async () => {
    let resolveOld: ((value: ProjectEnvironmentReport | null) => void) | undefined;
    bridge.inspectProjectEnvironment.mockImplementationOnce(
      () => new Promise((resolve) => { resolveOld = resolve; }),
    );
    const { rerender } = render(<ProjectEnvironmentPanel {...baseProps} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));

    rerender(<ProjectEnvironmentPanel {...baseProps} workspaceId="workspace-b" workspaceGeneration={2} />);
    expect(screen.getByRole("button", { name: "Inspect project environment" })).toBeEnabled();
    await act(async () => resolveOld?.(report));

    expect(screen.queryByText("TypeScript + Node.js")).not.toBeInTheDocument();
    expect(screen.getByText("Inspection only starts when you ask")).toBeVisible();
  });

  it("keeps inspection disabled while a workspace is opening", () => {
    render(<ProjectEnvironmentPanel {...baseProps} disabled />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));
    expect(bridge.inspectProjectEnvironment).not.toHaveBeenCalled();
  });

  it("clears a retained report as soon as folder opening begins", async () => {
    const { rerender } = render(<ProjectEnvironmentPanel {...baseProps} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));
    expect(await screen.findByText("TypeScript + Node.js")).toBeVisible();

    rerender(<ProjectEnvironmentPanel {...baseProps} disabled />);

    expect(screen.queryByText("TypeScript + Node.js")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Inspect project environment" })).toBeDisabled();
  });

  it("reports first-dialog cancellation without fabricating environment data", async () => {
    bridge.inspectProjectEnvironment.mockResolvedValueOnce(null);
    render(<ProjectEnvironmentPanel {...baseProps} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect project environment" }));

    expect(await screen.findByText("Inspection cancelled. No report was retained.")).toBeVisible();
    expect(screen.queryByText("TypeScript + Node.js")).not.toBeInTheDocument();
  });
});
