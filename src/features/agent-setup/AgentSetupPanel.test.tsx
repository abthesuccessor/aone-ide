import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AiConfigurationStatus } from "../../types";
import type { ProjectEnvironmentReport } from "../project-environment/model";
import { AgentSetupPanel } from "./AgentSetupPanel";

const bridge = vi.hoisted(() => ({
  askProjectAgent: vi.fn(),
  getProjectEnvironmentReport: vi.fn(),
  inspectProjectEnvironment: vi.fn(),
}));

vi.mock("../../lib/projectEnvironmentBridge", () => bridge);

const aiStatus: AiConfigurationStatus = {
  configured: true,
  inferenceAvailable: true,
  provider: "codex",
  model: "gpt-5.6-luna",
  transport: "cli",
};

const report: ProjectEnvironmentReport = {
  reportId: "report-a",
  workspaceId: "workspace-a",
  inspectedAt: "2026-09-05T00:00:00.000Z",
  versionProbeApproved: true,
  stacks: [{ id: "python", label: "Python", confidence: "confirmed", evidence: [] }],
  tools: [{
    id: "python",
    label: "Python",
    category: "Python",
    required: true,
    status: "available",
    alternateCanonicalPaths: [],
    versionArgs: ["--version"],
    usedBy: ["python"],
  }],
  recommendations: [
    {
      id: "api-profile",
      title: "Use API development profile",
      summary: "A registered profile matches this service.",
      kind: "run-profile",
      severity: "recommended",
      evidence: [{ relativePath: "pyproject.toml", detail: "registered command" }],
      runProfileId: "api-dev",
    },
    {
      id: "database",
      title: "Check PostgreSQL",
      summary: "Confirm the database separately.",
      kind: "environment",
      severity: "warning",
      evidence: [{ relativePath: "src/settings.py", detail: "database hint" }],
    },
  ],
};

const baseProps = {
  workspaceId: "workspace-a",
  workspaceGeneration: 1,
  aiStatus,
  runProfiles: [{ id: "api-dev", name: "API development" }],
  activeRunProfileId: "",
  onUseRunProfile: vi.fn(),
  onConfigureAi: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  bridge.getProjectEnvironmentReport.mockResolvedValue(null);
  bridge.inspectProjectEnvironment.mockResolvedValue(report);
  bridge.askProjectAgent.mockResolvedValue({
    status: "completed",
    answer: "Use a run-only .env and keep provider credentials separate.",
    evidence: [{ kind: "projectEnvironment", id: "report-a", label: "Saved project inspection", evidence: "resolved" }],
    model: "codex-cli",
  });
});

describe("Project Agent", () => {
  it("checks for an approved report without opening another inspection", async () => {
    render(<AgentSetupPanel {...baseProps} />);

    expect(screen.getByText("Checking this session")).toBeVisible();
    expect(await screen.findByRole("button", { name: "Inspect project environment" })).toBeEnabled();
    expect(bridge.getProjectEnvironmentReport).toHaveBeenCalledWith("workspace-a");
    expect(bridge.inspectProjectEnvironment).not.toHaveBeenCalled();
    expect(screen.getByText(/Closing and reopening Project Agent reuses that report/i)).toBeVisible();
  });

  it("restores the approved report when the panel is reopened", async () => {
    bridge.getProjectEnvironmentReport.mockResolvedValue(report);
    render(<AgentSetupPanel {...baseProps} />);

    expect(await screen.findByText("Use API development profile")).toBeVisible();
    expect(screen.getByText("Saved this session")).toBeVisible();
    expect(screen.getByText(/Reused the approved inspection.*no new permission was requested/i)).toBeVisible();
    expect(bridge.inspectProjectEnvironment).not.toHaveBeenCalled();
  });

  it("builds a deterministic plan and requires approval before selecting a profile", async () => {
    const onUseRunProfile = vi.fn();
    render(<AgentSetupPanel {...baseProps} onUseRunProfile={onUseRunProfile} />);
    fireEvent.click(await screen.findByRole("button", { name: "Inspect project environment" }));

    expect(await screen.findByText("Use API development profile")).toBeVisible();
    expect(screen.getByText("Check PostgreSQL")).toBeVisible();
    expect(screen.getByText("Engineer action")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Review IDE change" }));

    const approval = screen.getByRole("group", { name: "Approve Use API development profile" });
    expect(within(approval).getByText(/No process, installer, service, or container starts/i)).toBeVisible();
    fireEvent.click(within(approval).getByRole("button", { name: "Approve profile" }));
    expect(onUseRunProfile).toHaveBeenCalledWith("api-dev");
    expect(screen.getByText(/Run or Debug is still your explicit start decision/i)).toBeVisible();
  });

  it("keeps the last approved report visible while refresh waits or is cancelled", async () => {
    bridge.getProjectEnvironmentReport.mockResolvedValue(report);
    let resolveRefresh: ((value: ProjectEnvironmentReport | null) => void) | undefined;
    bridge.inspectProjectEnvironment.mockImplementationOnce(() => new Promise((resolve) => { resolveRefresh = resolve; }));
    render(<AgentSetupPanel {...baseProps} />);
    expect(await screen.findByText("Use API development profile")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Inspect again" }));
    expect(screen.getByText("Use API development profile")).toBeVisible();
    expect(screen.getByRole("button", { name: "Waiting for permission…" })).toBeDisabled();

    await act(async () => resolveRefresh?.(null));
    expect(screen.getByText("Use API development profile")).toBeVisible();
    expect(screen.getByText(/Re-inspection cancelled; the previous approved report remains available/i)).toBeVisible();
  });

  it("invalidates an in-flight chat when re-inspection begins", async () => {
    bridge.getProjectEnvironmentReport.mockResolvedValue(report);
    bridge.askProjectAgent.mockImplementationOnce(() => new Promise(() => undefined));
    bridge.inspectProjectEnvironment.mockImplementationOnce(() => new Promise(() => undefined));
    render(<AgentSetupPanel {...baseProps} />);
    expect(await screen.findByText("Use API development profile")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "What must run before I can test this service?" }));
    expect(await screen.findByText("Project Agent is thinking")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Inspect again" }));

    expect(screen.queryByText("Project Agent is thinking")).not.toBeInTheDocument();
    expect(screen.getByText("Use API development profile")).toBeVisible();
  });

  it("provides an explicit, evidence-labelled project chat", async () => {
    bridge.getProjectEnvironmentReport.mockResolvedValue(report);
    render(<AgentSetupPanel {...baseProps} />);
    expect(await screen.findByText("Use API development profile")).toBeVisible();

    fireEvent.change(screen.getByLabelText("Question for Project Agent"), {
      target: { value: "How should I configure .env?" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Ask" }));

    expect(await screen.findByText(/keep provider credentials separate/i)).toBeVisible();
    expect(bridge.askProjectAgent).toHaveBeenCalledWith({
      workspaceId: "workspace-a",
      reportId: "report-a",
      question: "How should I configure .env?",
    });
    expect(screen.getByText("Inferred · codex-cli")).toBeVisible();
    expect(screen.getByText("Saved project inspection · resolved")).toBeVisible();
    expect(baseProps.onUseRunProfile).not.toHaveBeenCalled();
  });

  it("shows a recoverable chat error without losing the engineer's question", async () => {
    bridge.getProjectEnvironmentReport.mockResolvedValue(report);
    bridge.askProjectAgent.mockResolvedValueOnce({
      status: "error",
      code: "providerFailed",
      message: "Provider timed out.",
      retryable: true,
    });
    render(<AgentSetupPanel {...baseProps} />);
    expect(await screen.findByText("Use API development profile")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "What must run before I can test this service?" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Provider timed out.");
    expect(within(screen.getByRole("list", { name: "Project Agent conversation" })).getByText("What must run before I can test this service?")).toBeVisible();
    expect(screen.getByRole("button", { name: "Retry" })).toBeEnabled();
  });

  it("routes code-only users to real AI configuration", async () => {
    const onConfigureAi = vi.fn();
    render(<AgentSetupPanel {...baseProps} aiStatus={null} onConfigureAi={onConfigureAi} />);

    fireEvent.click(screen.getByRole("button", { name: "Configure AI" }));
    expect(onConfigureAi).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("button", { name: "Inspect project environment" })).toBeEnabled();
  });

  it("ignores saved evidence from a previous workspace", async () => {
    let resolveOld: ((value: ProjectEnvironmentReport | null) => void) | undefined;
    bridge.getProjectEnvironmentReport.mockImplementationOnce(() => new Promise((resolve) => { resolveOld = resolve; }));
    const { rerender } = render(<AgentSetupPanel {...baseProps} />);

    rerender(<AgentSetupPanel {...baseProps} workspaceId="workspace-b" workspaceGeneration={2} />);
    await act(async () => resolveOld?.(report));

    expect(screen.queryByText("Use API development profile")).not.toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Inspect project environment" })).toBeEnabled();
  });

  it("never makes an unregistered profile approvable", async () => {
    render(<AgentSetupPanel {...baseProps} runProfiles={[]} />);
    fireEvent.click(await screen.findByRole("button", { name: "Inspect project environment" }));

    await waitFor(() => expect(screen.getAllByText("Engineer action")).toHaveLength(2));
    expect(screen.queryByRole("button", { name: "Review IDE change" })).not.toBeInTheDocument();
  });
});
