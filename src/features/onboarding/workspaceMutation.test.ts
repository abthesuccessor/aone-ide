import { describe, expect, it, vi } from "vitest";
import type { WorkspaceSummary } from "../../types";
import { runWorkspaceMutation } from "./workspaceMutation";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

const workspace: WorkspaceSummary = {
  id: "workspace:late",
  name: "late",
  rootPath: "/tmp/late",
  fileCount: 0,
  nodeCount: 0,
  edgeCount: 0,
  languages: [],
  lastScannedAt: "2026-08-17T00:00:00Z",
};

describe("shared onboarding workspace mutation", () => {
  it("does not adopt a workspace result after its generation becomes stale", async () => {
    const pending = deferred<{ workspace: WorkspaceSummary } | null>();
    const generationRef = { current: 4 };
    const adopt = vi.fn().mockResolvedValue(undefined);
    const action = runWorkspaceMutation({
      phase: "ready",
      initializingRef: { current: false },
      openingRef: { current: false },
      operationRef: { current: false },
      generationRef,
      setOpening: vi.fn(),
      clearProgress: vi.fn(),
      adopt,
      notify: vi.fn(),
    }, () => pending.promise, "Could not change workspace");

    generationRef.current += 1;
    pending.resolve({ workspace });

    await expect(action).resolves.toEqual({ status: "stale" });
    expect(adopt).not.toHaveBeenCalled();
  });

  it("maps native user cancellation to a quiet cancelled outcome", async () => {
    const notify = vi.fn();
    const outcome = await runWorkspaceMutation({
      phase: "ready",
      initializingRef: { current: false },
      openingRef: { current: false },
      operationRef: { current: false },
      generationRef: { current: 1 },
      setOpening: vi.fn(),
      clearProgress: vi.fn(),
      adopt: vi.fn(),
      notify,
    }, () => Promise.reject("Invalid request: GitHub clone was cancelled"), "Could not clone");

    expect(outcome).toEqual({ status: "cancelled" });
    expect(notify).not.toHaveBeenCalled();
  });
});
