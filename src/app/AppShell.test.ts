import { describe, expect, it, vi } from "vitest";
import { routeRunIntent } from "./AppShell";

function controller(activeProfileId = "") {
  return {
    activeProfileId,
    setPrimarySidebarVisible: vi.fn(),
    setActivityView: vi.fn(),
    setMainView: vi.fn(),
    notify: vi.fn(),
    handleStart: vi.fn().mockResolvedValue(undefined),
  };
}

describe("routeRunIntent", () => {
  it("opens Project Agent instead of guessing how to debug a service", () => {
    const target = controller();

    routeRunIntent(target, "debug");

    expect(target.handleStart).not.toHaveBeenCalled();
    expect(target.setPrimarySidebarVisible).toHaveBeenCalledWith(true);
    expect(target.setActivityView).toHaveBeenCalledWith("agent-setup");
    expect(target.setMainView).toHaveBeenCalledWith("graph");
    expect(target.notify).toHaveBeenCalledWith(
      expect.stringMatching(/No runnable profile.*Project Agent.*will not guess commands or start dependencies/i),
      "warning",
    );
  });

  it("starts the selected runnable profile without opening setup", () => {
    const target = controller("api-dev");

    routeRunIntent(target, "run");

    expect(target.handleStart).toHaveBeenCalledWith("run");
    expect(target.setActivityView).not.toHaveBeenCalled();
    expect(target.notify).not.toHaveBeenCalled();
  });
});
