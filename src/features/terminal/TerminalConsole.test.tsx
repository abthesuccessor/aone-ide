import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_WORKBENCH_SETTINGS } from "../editor/settings";
import type { TerminalEvent } from "./model";
import { TerminalConsole } from "./TerminalConsole";

const xterm = vi.hoisted(() => ({
  instance: null as null | {
    write: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
  },
  onData: null as null | ((value: string) => void),
  fit: vi.fn(),
  dataDispose: vi.fn(),
  resize: null as ResizeObserverCallback | null,
}));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown>;
    loadAddon = vi.fn();
    open = vi.fn();
    clear = vi.fn();
    focus = vi.fn();
    dispose = vi.fn();
    write = vi.fn();
    writeln = vi.fn();
    constructor(options: Record<string, unknown>) {
      this.options = options;
      xterm.instance = this;
    }
    onData(listener: (value: string) => void) {
      xterm.onData = listener;
      return { dispose: xterm.dataDispose };
    }
  },
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class { fit = xterm.fit; },
}));

const bridge = vi.hoisted(() => ({
  closeTerminal: vi.fn(),
  listTerminalProfiles: vi.fn(),
  openTerminal: vi.fn(),
  resizeTerminal: vi.fn(),
  subscribeTerminalEvents: vi.fn(),
  writeTerminal: vi.fn(),
  listener: null as null | ((event: TerminalEvent) => void),
  unsubscribe: vi.fn(),
}));

vi.mock("../../lib/terminalBridge", () => ({
  closeTerminal: bridge.closeTerminal,
  listTerminalProfiles: bridge.listTerminalProfiles,
  openTerminal: bridge.openTerminal,
  resizeTerminal: bridge.resizeTerminal,
  subscribeTerminalEvents: bridge.subscribeTerminalEvents,
  writeTerminal: bridge.writeTerminal,
}));

beforeEach(() => {
  vi.clearAllMocks();
  xterm.instance = null;
  xterm.onData = null;
  xterm.resize = null;
  Object.defineProperty(globalThis, "ResizeObserver", {
    configurable: true,
    value: class {
      constructor(callback: ResizeObserverCallback) { xterm.resize = callback; }
      disconnect() {}
      observe() {}
      unobserve() {}
    },
  });
  bridge.listener = null;
  bridge.listTerminalProfiles.mockResolvedValue([{ id: "zsh", label: "zsh", shellPath: "/bin/zsh" }]);
  bridge.subscribeTerminalEvents.mockImplementation(async (listener: (event: TerminalEvent) => void) => {
    bridge.listener = listener;
    return bridge.unsubscribe;
  });
  bridge.openTerminal.mockResolvedValue({
    sessionId: "terminal:test",
    profileId: "zsh",
    shellLabel: "zsh",
    cwd: "/workspace",
  });
  bridge.closeTerminal.mockResolvedValue({ accepted: true });
  bridge.writeTerminal.mockResolvedValue({ accepted: true });
  bridge.resizeTerminal.mockResolvedValue({ accepted: true });
});

describe("TerminalConsole", () => {
  it("opens, streams, resizes, closes, and cleans up a terminal session", async () => {
    const { unmount } = render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);

    await waitFor(() => expect(bridge.openTerminal).toHaveBeenCalledWith({
      profileId: "zsh",
      columns: 80,
      rows: 24,
    }));
    expect(await screen.findByText("zsh · /workspace")).toBeVisible();

    act(() => xterm.onData?.("ls\r"));
    expect(bridge.writeTerminal).toHaveBeenCalledWith({
      sessionId: "terminal:test",
      data: "ls\r",
      encoding: "text",
    });
    act(() => bridge.listener?.({
      sessionId: "terminal:test",
      kind: "data",
      timestamp: "2026-08-17T00:00:00Z",
      data: btoa("hello"),
      encoding: "base64",
      byteLength: 5,
    }));
    expect(xterm.instance?.write).toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(bridge.closeTerminal).toHaveBeenCalledWith({ sessionId: "terminal:test" }));
    expect(screen.getByText("No terminal session")).toBeVisible();

    fireEvent.click(newButton);
    await waitFor(() => expect(bridge.openTerminal).toHaveBeenCalledTimes(2));
    unmount();
    expect(bridge.unsubscribe).toHaveBeenCalledTimes(1);
    expect(xterm.dataDispose).toHaveBeenCalledTimes(1);
    expect(xterm.instance?.dispose).toHaveBeenCalledTimes(1);
    expect(bridge.closeTerminal).toHaveBeenLastCalledWith({ sessionId: "terminal:test" });
  });

  it("buffers immediate output and exit until the opening session ID is known", async () => {
    let finishOpen: ((value: {
      sessionId: string;
      profileId: string;
      shellLabel: string;
      cwd: string;
    }) => void) | undefined;
    bridge.openTerminal.mockImplementationOnce(() => new Promise((resolve) => { finishOpen = resolve; }));
    render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);

    act(() => {
      bridge.listener?.({
        sessionId: "terminal:early",
        kind: "data",
        timestamp: "2026-08-17T00:00:00Z",
        data: btoa("early prompt"),
        encoding: "base64",
        byteLength: 12,
      });
      bridge.listener?.({
        sessionId: "terminal:early",
        kind: "exit",
        timestamp: "2026-08-17T00:00:01Z",
        exitCode: 0,
      });
    });
    expect(xterm.instance?.write).not.toHaveBeenCalled();

    await act(async () => finishOpen?.({
      sessionId: "terminal:early",
      profileId: "zsh",
      shellLabel: "zsh",
      cwd: "/workspace",
    }));
    expect(xterm.instance?.write).toHaveBeenCalledTimes(1);
    expect((xterm.instance as unknown as { writeln: ReturnType<typeof vi.fn> }).writeln)
      .toHaveBeenCalledWith("\r\n[process exited 0]");
    expect(screen.getByText("No terminal session")).toBeVisible();
  });

  it("surfaces terminal input IPC rejections instead of dropping them", async () => {
    bridge.writeTerminal.mockRejectedValueOnce("Terminal input was rejected");
    render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);
    await screen.findByText("zsh · /workspace");

    act(() => xterm.onData?.("pwd\r"));

    expect(await screen.findByRole("alert")).toHaveTextContent("Terminal input was rejected");
  });

  it("clears a stale session when terminal input is not accepted", async () => {
    bridge.writeTerminal.mockResolvedValueOnce({ accepted: false });
    render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);
    await screen.findByText("zsh · /workspace");

    act(() => xterm.onData?.("pwd\r"));

    expect(await screen.findByRole("alert")).toHaveTextContent("Terminal session is no longer active");
    expect(bridge.closeTerminal).toHaveBeenCalledWith({ sessionId: "terminal:test" });
    expect(screen.getByText("No terminal session")).toBeVisible();
  });

  it("best-effort closes and clears a stale session when a terminal resize is not accepted", async () => {
    bridge.resizeTerminal.mockResolvedValueOnce({ accepted: false });
    bridge.closeTerminal.mockRejectedValueOnce(new Error("Native close also failed"));
    const { container } = render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);
    await screen.findByText("zsh · /workspace");
    const host = container.querySelector<HTMLElement>(".terminal-host");
    expect(host).not.toBeNull();
    Object.defineProperty(host!, "clientWidth", { configurable: true, value: 640 });
    Object.defineProperty(host!, "clientHeight", { configurable: true, value: 240 });

    act(() => xterm.resize?.([], {} as ResizeObserver));

    expect(await screen.findByRole("alert")).toHaveTextContent("Terminal session is no longer active");
    expect(bridge.closeTerminal).toHaveBeenCalledWith({ sessionId: "terminal:test" });
    expect(screen.getByText("No terminal session")).toBeVisible();
  });

  it("does not create a subscription when profile discovery fails", async () => {
    bridge.listTerminalProfiles.mockRejectedValueOnce(new Error("Profiles unavailable"));
    const { unmount } = render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Profiles unavailable");
    expect(bridge.subscribeTerminalEvents).not.toHaveBeenCalled();
    unmount();
    expect(bridge.unsubscribe).not.toHaveBeenCalled();
  });

  it("disposes a subscription that resolves after the console unmounts", async () => {
    let finishSubscribe: ((unsubscribe: () => void) => void) | undefined;
    bridge.subscribeTerminalEvents.mockImplementationOnce(() => new Promise((resolve) => {
      finishSubscribe = resolve;
    }));
    const { unmount } = render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    await waitFor(() => expect(bridge.subscribeTerminalEvents).toHaveBeenCalledTimes(1));
    unmount();
    await act(async () => finishSubscribe?.(bridge.unsubscribe));
    expect(bridge.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("bounds events received during native open consent", async () => {
    let finishOpen: ((value: {
      sessionId: string;
      profileId: string;
      shellLabel: string;
      cwd: string;
    }) => void) | undefined;
    bridge.openTerminal.mockImplementationOnce(() => new Promise((resolve) => { finishOpen = resolve; }));
    render(<TerminalConsole settings={DEFAULT_WORKBENCH_SETTINGS} visible />);
    const newButton = await screen.findByRole("button", { name: "New" });
    await waitFor(() => expect(newButton).toBeEnabled());
    fireEvent.click(newButton);
    act(() => {
      for (let index = 0; index < 35; index += 1) {
        bridge.listener?.({
          sessionId: "terminal:bounded",
          kind: "data",
          timestamp: `2026-08-17T00:00:${String(index).padStart(2, "0")}Z`,
          data: btoa(String(index)),
          encoding: "base64",
          byteLength: String(index).length,
        });
      }
    });
    await act(async () => finishOpen?.({
      sessionId: "terminal:bounded",
      profileId: "zsh",
      shellLabel: "zsh",
      cwd: "/workspace",
    }));

    expect(xterm.instance?.write).toHaveBeenCalledTimes(32);
    expect((xterm.instance as unknown as { writeln: ReturnType<typeof vi.fn> }).writeln)
      .toHaveBeenCalledWith("\r\n[Aone terminal omitted 3 early events]");
  });
});
