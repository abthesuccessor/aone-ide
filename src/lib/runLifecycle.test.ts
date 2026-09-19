import { describe, expect, it } from "vitest";
import type { RuntimeEvent } from "../types";
import { rememberTerminalRun, runAlreadyTerminated } from "./runLifecycle";

function event(kind: string, runId?: string): RuntimeEvent {
  return {
    id: `${kind}:${runId ?? "none"}`,
    runId,
    kind,
    timestamp: "2026-08-16T10:00:00Z",
    label: kind,
    evidence: "observed",
    metadata: {},
  };
}

describe("run lifecycle", () => {
  it("remembers an exit that arrives before the start response", () => {
    const terminalRuns = new Set<string>();

    expect(rememberTerminalRun(terminalRuns, event("process.exited", "run-fast"))).toBe("run-fast");
    expect(runAlreadyTerminated(terminalRuns, "run-fast")).toBe(true);
  });

  it("does not treat stream or unrelated terminal events as a completed run", () => {
    const terminalRuns = new Set<string>();

    expect(rememberTerminalRun(terminalRuns, event("process.stdout", "run-live"))).toBeNull();
    expect(rememberTerminalRun(terminalRuns, event("process.wait_failed"))).toBeNull();
    expect(runAlreadyTerminated(terminalRuns, "run-live")).toBe(false);
  });
});
