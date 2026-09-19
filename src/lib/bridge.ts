import {
  createDemoApiResponse,
  demoAiEnv,
  demoExplanation,
  demoFiles,
  demoGraph,
  demoRunProfiles,
  demoRunEnv,
  demoRuntimeEvents,
  demoSnapshot,
  demoWorkspace,
  getDemoSource,
} from "../data/demo";
import type {
  AiExplainRequest,
  AiConfigurationStatus,
  AiCliAdapterStatus,
  AiExplanation,
  ApiRequest,
  ApiResponse,
  AppSnapshot,
  ConfigureHostedAiRequest,
  ConfigureOllamaRequest,
  EnvLoadResult,
  GraphQuery,
  GraphSnapshot,
  RunProfile,
  RuntimeEvent,
  ScanProgress,
  SourceFile,
  StartRunRequest,
  WebSocketConnectRequest,
  WebSocketConnectResult,
  WebSocketDisconnectRequest,
  WebSocketDisconnectResult,
  WebSocketEvent,
  WebSocketSendRequest,
  WebSocketSendResult,
  WorkspaceFile,
  OpenedWorkspaceFile,
  WorkspaceSummary,
} from "../types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

const DEMO_DELAY_MS = 90;
const demoWebSocketListeners = new Set<(event: WebSocketEvent) => void>();
const demoWebSocketSessions = new Map<
  string,
  { destination: string; protocol?: string }
>();
let demoWebSocketSequence = 0;
let demoAiConfigurationStatus: AiConfigurationStatus = {
  configured: false,
  provider: null,
  model: null,
  transport: null,
  inferenceAvailable: false,
};

function delay(ms = DEMO_DELAY_MS): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function demoDestination(rawUrl: string): string {
  try {
    const parsed = new URL(rawUrl);
    if (parsed.protocol !== "ws:" && parsed.protocol !== "wss:") {
      throw new Error("Only ws and wss URLs are supported");
    }
    if (parsed.username || parsed.password || parsed.hash) {
      throw new Error("WebSocket URLs cannot include credentials or fragments");
    }
    return parsed.origin;
  } catch {
    throw new Error("Enter a valid ws or wss URL without credentials or a fragment");
  }
}

function emitDemoWebSocketEvent(event: WebSocketEvent): void {
  for (const listener of demoWebSocketListeners) listener(event);
}

function demoPayloadByteLength(data: string, encoding: WebSocketSendRequest["encoding"]): number {
  if (encoding === "text") return new TextEncoder().encode(data).byteLength;
  const normalized = data.replace(/=+$/, "");
  return Math.floor((normalized.length * 3) / 4);
}

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

async function invokeOrDemo<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  demo: () => T | Promise<T>,
): Promise<T> {
  if (isTauriRuntime()) return invokeDesktop<T>(command, args);
  await delay();
  return demo();
}

export function getAppSnapshot(): Promise<AppSnapshot> {
  return invokeOrDemo("get_app_snapshot", undefined, () => demoSnapshot);
}

export function getAiConfigurationStatus(): Promise<AiConfigurationStatus> {
  return invokeOrDemo("get_ai_configuration_status", undefined, () => demoAiConfigurationStatus);
}

export function configureHostedAi(request: ConfigureHostedAiRequest): Promise<AiConfigurationStatus> {
  return invokeOrDemo("configure_hosted_ai", { request }, () => {
    demoAiConfigurationStatus = {
      configured: true,
      provider: request.provider,
      model: request.model?.trim() || (request.provider === "openai" ? "gpt-5.6-luna" : "claude-sonnet-4-20250514"),
      transport: "api",
      inferenceAvailable: true,
    };
    return demoAiConfigurationStatus;
  });
}

export function configureOllama(request: ConfigureOllamaRequest): Promise<AiConfigurationStatus> {
  return invokeOrDemo("configure_ollama", { request }, () => {
    demoAiConfigurationStatus = {
      configured: true,
      provider: "ollama",
      model: request.model,
      transport: "local_http",
      inferenceAvailable: true,
    };
    return demoAiConfigurationStatus;
  });
}

export function listAiCliAdapters(): Promise<AiCliAdapterStatus[]> {
  return invokeOrDemo("list_ai_cli_adapters", undefined, () => [
    {
      id: "codex",
      label: "Codex CLI",
      installed: false,
      readiness: "not_installed",
      detail: "Install Codex CLI in the desktop app environment to connect it.",
    },
    {
      id: "claude",
      label: "Claude Code",
      installed: false,
      readiness: "not_installed",
      detail: "Install Claude Code in the desktop app environment to connect it.",
    },
    {
      id: "copilot",
      label: "GitHub Copilot CLI",
      installed: false,
      readiness: "not_supported",
      detail: "The Aone execution adapter is not implemented in this build.",
    },
  ]);
}

/** CLI adapters this build can drive. Mirrors SUPPORTED_CLI_KINDS in Rust. */
export const SUPPORTED_CLI_ADAPTERS = ["codex", "claude"] as const;

export type SupportedCliAdapter = (typeof SUPPORTED_CLI_ADAPTERS)[number];

export function isSupportedCliAdapter(id: string): id is SupportedCliAdapter {
  return (SUPPORTED_CLI_ADAPTERS as readonly string[]).includes(id);
}

export function configureAiCli(adapter: SupportedCliAdapter): Promise<AiConfigurationStatus> {
  return invokeOrDemo("configure_ai_cli", { request: { adapter } }, () => {
    throw new Error("CLI adapters require the native desktop runtime");
  });
}

export async function openWorkspace(): Promise<WorkspaceSummary | null> {
  return invokeOrDemo("pick_and_open_workspace", undefined, () => demoWorkspace);
}

export async function openWorkspaceFile(): Promise<OpenedWorkspaceFile | null> {
  return invokeOrDemo("pick_and_open_file", undefined, () => ({
    workspace: demoWorkspace,
    relativePath: "src/web/CheckoutPage.tsx",
  }));
}

export async function rescanWorkspace(): Promise<WorkspaceSummary> {
  if (!isTauriRuntime()) await delay(760);
  return invokeOrDemo("rescan_workspace", undefined, () => ({
    ...demoWorkspace,
    lastScannedAt: new Date().toISOString(),
  }));
}

export function getWorkspaceFiles(query?: string, limit = 5_000): Promise<WorkspaceFile[]> {
  return invokeOrDemo("get_workspace_files", { query, limit }, () => {
    const normalized = query?.trim().toLowerCase();
    const files = normalized
      ? demoFiles.filter((file) => file.relativePath.toLowerCase().includes(normalized))
      : demoFiles;
    return files.slice(0, limit);
  });
}

export function readWorkspaceFile(relativePath: string): Promise<SourceFile> {
  return invokeOrDemo("read_workspace_file", { relativePath }, () => getDemoSource(relativePath));
}

export function queryGraph(query: GraphQuery = {}): Promise<GraphSnapshot> {
  return invokeOrDemo("query_graph", { query }, () => demoGraph);
}

export function detectRunProfiles(): Promise<RunProfile[]> {
  return invokeOrDemo("detect_run_profiles", undefined, () => demoRunProfiles);
}

export async function pickAndLoadEnvFile(): Promise<EnvLoadResult | null> {
  if (!isTauriRuntime()) {
    await delay();
    demoAiConfigurationStatus = {
      configured: true,
      provider: "openai",
      model: "gpt-5.6-luna",
      transport: "api",
      inferenceAvailable: true,
    };
    return demoAiEnv;
  }
  return invokeDesktop<EnvLoadResult | null>("pick_and_load_env_file");
}

export async function pickAndLoadRunEnvFile(): Promise<EnvLoadResult | null> {
  if (!isTauriRuntime()) {
    await delay();
    return demoRunEnv;
  }
  return invokeDesktop<EnvLoadResult | null>("pick_and_load_run_env_file");
}

export function startRun(request: StartRunRequest): Promise<{ runId: string }> {
  return invokeOrDemo("start_run", { request }, () => ({ runId: `demo-run-${Date.now()}` }));
}

export function stopRun(runId: string): Promise<{ stopped: boolean }> {
  return invokeOrDemo("stop_run", { request: { runId } }, () => ({ stopped: true }));
}

export function listRuntimeEvents(limit = 500): Promise<RuntimeEvent[]> {
  return invokeOrDemo("list_runtime_events", { request: { limit } }, () => demoRuntimeEvents.slice(-limit));
}

export function sendApiRequest(request: ApiRequest): Promise<ApiResponse> {
  return invokeOrDemo("send_api_request", { request }, () => createDemoApiResponse(request));
}

export async function connectWebSocket(
  request: WebSocketConnectRequest,
): Promise<WebSocketConnectResult> {
  if (isTauriRuntime()) {
    return invokeDesktop<WebSocketConnectResult>("connect_websocket", { request });
  }

  await delay();
  const sessionId = `demo-websocket-${++demoWebSocketSequence}`;
  const destination = demoDestination(request.url);
  const protocol = request.protocols[0];
  emitDemoWebSocketEvent({
    sessionId,
    kind: "connecting",
    timestamp: new Date().toISOString(),
    destination,
    truncated: false,
    detail: "Browser demo uses a deterministic local simulation; no socket was opened.",
  });
  await delay(45);
  demoWebSocketSessions.set(sessionId, { destination, protocol });
  emitDemoWebSocketEvent({
    sessionId,
    kind: "open",
    timestamp: new Date().toISOString(),
    destination,
    protocol,
    truncated: false,
    detail: "Synthetic browser-demo connection opened.",
  });
  return { sessionId, protocol };
}

export async function sendWebSocketMessage(
  request: WebSocketSendRequest,
): Promise<WebSocketSendResult> {
  if (isTauriRuntime()) {
    return invokeDesktop<WebSocketSendResult>("send_websocket_message", { request });
  }

  await delay(45);
  const session = demoWebSocketSessions.get(request.sessionId);
  if (!session) throw new Error("The browser-demo WebSocket session is not open");
  emitDemoWebSocketEvent({
    sessionId: request.sessionId,
    kind: "message",
    timestamp: new Date().toISOString(),
    destination: session.destination,
    protocol: session.protocol,
    data: request.data,
    encoding: request.encoding,
    byteLength: demoPayloadByteLength(request.data, request.encoding),
    truncated: false,
    detail: "Synthetic echo response; no network request was made.",
  });
  return { accepted: true };
}

export async function disconnectWebSocket(
  request: WebSocketDisconnectRequest,
): Promise<WebSocketDisconnectResult> {
  if (isTauriRuntime()) {
    return invokeDesktop<WebSocketDisconnectResult>("disconnect_websocket", { request });
  }

  await delay(45);
  const session = demoWebSocketSessions.get(request.sessionId);
  if (!session) return { disconnected: false };
  demoWebSocketSessions.delete(request.sessionId);
  emitDemoWebSocketEvent({
    sessionId: request.sessionId,
    kind: "closed",
    timestamp: new Date().toISOString(),
    destination: session.destination,
    protocol: session.protocol,
    truncated: false,
    detail: "Synthetic browser-demo connection closed.",
  });
  return { disconnected: true };
}

export function explainNode(request: AiExplainRequest): Promise<AiExplanation> {
  return invokeOrDemo("ai_explain", { request }, () => demoExplanation);
}

interface EventHandlers {
  onScanProgress?: (progress: ScanProgress) => void;
  onRuntimeEvent?: (event: RuntimeEvent) => void;
  onWorkspaceChanged?: (paths: string[]) => void;
  onWebSocketEvent?: (event: WebSocketEvent) => void;
}

export async function subscribeToDesktopEvents(handlers: EventHandlers): Promise<() => void> {
  if (!isTauriRuntime()) {
    if (handlers.onWebSocketEvent) demoWebSocketListeners.add(handlers.onWebSocketEvent);
    return () => {
      if (handlers.onWebSocketEvent) demoWebSocketListeners.delete(handlers.onWebSocketEvent);
    };
  }

  const { listen } = await import("@tauri-apps/api/event");
  const unlisten: Array<() => void> = [];
  try {
    if (handlers.onScanProgress) {
      unlisten.push(await listen<ScanProgress>(
        "aone-scan-progress",
        ({ payload }) => handlers.onScanProgress?.(payload),
      ));
    }
    if (handlers.onRuntimeEvent) {
      unlisten.push(await listen<RuntimeEvent>(
        "aone-runtime-event",
        ({ payload }) => handlers.onRuntimeEvent?.(payload),
      ));
    }
    if (handlers.onWorkspaceChanged) {
      unlisten.push(await listen<{ paths: string[] }>(
        "aone-workspace-changed",
        ({ payload }) => handlers.onWorkspaceChanged?.(payload.paths),
      ));
    }
    if (handlers.onWebSocketEvent) {
      unlisten.push(await listen<WebSocketEvent>(
        "aone-websocket-event",
        ({ payload }) => handlers.onWebSocketEvent?.(payload),
      ));
    }
  } catch (error) {
    for (const stop of unlisten.reverse()) stop();
    throw error;
  }

  return () => {
    for (const stop of unlisten.reverse()) stop();
  };
}
