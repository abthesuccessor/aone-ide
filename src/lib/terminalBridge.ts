import type {
  TerminalActionResult,
  TerminalEvent,
  TerminalOpenRequest,
  TerminalOpenResult,
  TerminalProfile,
} from "../features/terminal/model";

const demoListeners = new Set<(event: TerminalEvent) => void>();
let demoSessionId: string | null = null;
let demoSessionSequence = 0;

function desktopRuntime() {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

function emitDemo(sessionId: string, text: string, kind: TerminalEvent["kind"] = "data") {
  if (sessionId !== demoSessionId) return;
  const bytes = new TextEncoder().encode(text);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const event: TerminalEvent = {
    sessionId,
    kind,
    timestamp: new Date().toISOString(),
    data: kind === "data" ? window.btoa(binary) : undefined,
    encoding: kind === "data" ? "base64" : undefined,
    byteLength: kind === "data" ? bytes.byteLength : undefined,
    detail: kind === "error" ? text : undefined,
  };
  for (const listener of demoListeners) listener(event);
}

export async function listTerminalProfiles(): Promise<TerminalProfile[]> {
  return desktopRuntime()
    ? invokeDesktop("list_terminal_profiles")
    : [{ id: "zsh", label: "zsh", shellPath: "/bin/zsh" }, { id: "bash", label: "bash", shellPath: "/bin/bash" }];
}

export async function openTerminal(request: TerminalOpenRequest): Promise<TerminalOpenResult> {
  if (desktopRuntime()) return invokeDesktop("open_terminal", { request });
  if (demoSessionId) throw new Error("A terminal session is already open");
  demoSessionSequence += 1;
  const sessionId = `terminal:demo-${Date.now()}-${demoSessionSequence}`;
  demoSessionId = sessionId;
  window.setTimeout(() => emitDemo(sessionId, "Aone browser-demo terminal\r\n$ "), 30);
  return { sessionId, profileId: request.profileId, shellLabel: request.profileId, cwd: "/demo/workspace" };
}

export async function writeTerminal(request: { sessionId: string; data: string; encoding: "text" | "base64" }): Promise<TerminalActionResult> {
  if (desktopRuntime()) return invokeDesktop("write_terminal", { request });
  if (request.sessionId !== demoSessionId) return { accepted: false };
  if (request.encoding === "text") emitDemo(request.sessionId, request.data.replace(/\r/g, "\r\n$ "));
  return { accepted: true };
}

export async function resizeTerminal(request: { sessionId: string; columns: number; rows: number }): Promise<TerminalActionResult> {
  return desktopRuntime() ? invokeDesktop("resize_terminal", { request }) : { accepted: request.sessionId === demoSessionId };
}

export async function closeTerminal(request: { sessionId: string }): Promise<TerminalActionResult> {
  if (desktopRuntime()) return invokeDesktop("close_terminal", { request });
  if (request.sessionId !== demoSessionId) return { accepted: false };
  emitDemo(request.sessionId, "", "exit");
  demoSessionId = null;
  return { accepted: true };
}

export async function subscribeTerminalEvents(listener: (event: TerminalEvent) => void): Promise<() => void> {
  if (!desktopRuntime()) {
    demoListeners.add(listener);
    return () => demoListeners.delete(listener);
  }
  const { listen } = await import("@tauri-apps/api/event");
  return listen<TerminalEvent>("aone-terminal-event", ({ payload }) => listener(payload));
}
