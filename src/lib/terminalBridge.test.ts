import { afterEach, describe, expect, it, vi } from "vitest";
import {
  closeTerminal,
  listTerminalProfiles,
  openTerminal,
  resizeTerminal,
  subscribeTerminalEvents,
  writeTerminal,
} from "./terminalBridge";

afterEach(() => {
  vi.useRealTimers();
});

describe("terminal browser-demo bridge", () => {
  it("enforces one session and emits typed data through an unsubscribeable listener", async () => {
    vi.useFakeTimers();
    expect(await listTerminalProfiles()).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: "zsh", shellPath: "/bin/zsh" }),
    ]));
    const events: Array<{ kind: string; sessionId: string; data?: string }> = [];
    const unsubscribe = await subscribeTerminalEvents((event) => events.push(event));
    const session = await openTerminal({ profileId: "zsh", columns: 80, rows: 24 });

    await expect(openTerminal({ profileId: "bash", columns: 80, rows: 24 }))
      .rejects.toThrow("already open");
    vi.advanceTimersByTime(30);
    expect(atob(events[0]?.data ?? "")).toContain("browser-demo terminal");

    await expect(writeTerminal({ sessionId: session.sessionId, data: "pwd\r", encoding: "text" }))
      .resolves.toEqual({ accepted: true });
    expect(atob(events.at(-1)?.data ?? "")).toBe("pwd\r\n$ ");
    await expect(resizeTerminal({ sessionId: "stale", columns: 90, rows: 30 }))
      .resolves.toEqual({ accepted: false });
    await expect(closeTerminal({ sessionId: session.sessionId })).resolves.toEqual({ accepted: true });
    expect(events.at(-1)?.kind).toBe("exit");

    unsubscribe();
    const replacement = await openTerminal({ profileId: "bash", columns: 80, rows: 24 });
    vi.advanceTimersByTime(30);
    expect(events.at(-1)?.kind).toBe("exit");
    await closeTerminal({ sessionId: replacement.sessionId });
  });

  it("does not route a closed session's delayed banner to its replacement", async () => {
    vi.useFakeTimers();
    const events: string[] = [];
    const unsubscribe = await subscribeTerminalEvents((event) => {
      if (event.data) events.push(atob(event.data));
    });
    const first = await openTerminal({ profileId: "zsh", columns: 80, rows: 24 });
    await closeTerminal({ sessionId: first.sessionId });
    const second = await openTerminal({ profileId: "bash", columns: 80, rows: 24 });
    vi.advanceTimersByTime(30);

    expect(events).toEqual(["Aone browser-demo terminal\r\n$ "]);
    await closeTerminal({ sessionId: second.sessionId });
    unsubscribe();
  });
});
