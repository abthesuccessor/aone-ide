import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GitOnboarding } from "./GitOnboarding";
import type { GitOnboardingReport } from "./model";

const bridge = vi.hoisted(() => ({ inspectGitOnboarding: vi.fn() }));
vi.mock("../../lib/onboardingBridge", () => ({
  inspectGitOnboarding: bridge.inspectGitOnboarding,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

function report(workspaceId: string): GitOnboardingReport {
  return {
    workspaceId,
    inspectedAt: "2026-08-17T00:00:00Z",
    isRepository: true,
    branch: "feature/onboarding",
    identity: { name: "Amra", source: "local" },
    remotes: [{
      name: "origin",
      displayUrl: "git@github.com:example/project.git",
      host: "github.com",
      ownerRepo: "example/project",
      transport: "ssh",
    }],
    publicKeys: [{ fileName: "id_ed25519.pub", keyType: "ssh-ed25519", fingerprint: "SHA256:public-fingerprint" }],
    privateKeysRead: false,
  };
}

beforeEach(() => bridge.inspectGitOnboarding.mockReset());

describe("Git onboarding", () => {
  it("does not inspect on mount and always states the private-key boundary", () => {
    render(<GitOnboarding workspaceId="workspace:a" workspaceGeneration={1} />);
    expect(bridge.inspectGitOnboarding).not.toHaveBeenCalled();
    expect(screen.getByText("Private key contents: never read")).toBeInTheDocument();
    expect(screen.getByText("Aone has not executed these actions.")).toBeVisible();
  });

  it("cancels a pending inspection and ignores its late report", async () => {
    const pending = deferred<GitOnboardingReport | null>();
    bridge.inspectGitOnboarding.mockReturnValueOnce(pending.promise);
    render(<GitOnboarding workspaceId="workspace:a" workspaceGeneration={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Git setup" }));
    expect(bridge.inspectGitOnboarding).toHaveBeenCalledWith({ workspaceId: "workspace:a" });
    fireEvent.click(screen.getByRole("button", { name: "Cancel inspection" }));
    expect(screen.getByText("Inspection cancelled. No report was retained.")).toBeVisible();
    pending.resolve(report("workspace:a"));
    await waitFor(() => expect(screen.queryByLabelText("Sanitized Git setup report")).not.toBeInTheDocument());
    await waitFor(() => expect(screen.getByRole("button", { name: "Inspect Git setup" })).toHaveFocus());
  });

  it("ignores a report after the workspace generation changes", async () => {
    const pending = deferred<GitOnboardingReport | null>();
    bridge.inspectGitOnboarding.mockReturnValueOnce(pending.promise);
    const { rerender } = render(<GitOnboarding workspaceId="workspace:a" workspaceGeneration={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Git setup" }));
    rerender(<GitOnboarding workspaceId="workspace:b" workspaceGeneration={2} />);
    pending.resolve(report("workspace:a"));
    await waitFor(() => expect(screen.queryByLabelText("Sanitized Git setup report")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Inspect Git setup" })).toBeEnabled();
  });

  it("renders only sanitized report fields after explicit inspection", async () => {
    bridge.inspectGitOnboarding.mockResolvedValueOnce(report("workspace:a"));
    render(<GitOnboarding workspaceId="workspace:a" workspaceGeneration={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Inspect Git setup" }));
    expect(await screen.findByLabelText("Sanitized Git setup report")).toHaveTextContent("example/project");
    expect(screen.getByLabelText("Sanitized Git setup report")).toHaveTextContent("SHA256:public-fingerprint");
    expect(screen.getAllByText("Private key contents: never read")).toHaveLength(2);
    expect(document.body).not.toHaveTextContent("PRIVATE KEY");
  });
});
