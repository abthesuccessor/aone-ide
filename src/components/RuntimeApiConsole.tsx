import {
  CaretDown,
  Database,
  Globe,
  Pulse,
  TerminalWindow,
  Warning,
} from "@phosphor-icons/react";
import { lazy, Suspense, useMemo, useState } from "react";
import { WebSocketConsole } from "../features/realtime/WebSocketConsole";
import { ApiClientPane } from "../features/api-client/ApiClientPane";
import { useApiClientSession } from "../features/api-client/useApiClientSession";
import { DEFAULT_WORKBENCH_SETTINGS } from "../features/editor/settings";
import type { WorkbenchSettings } from "../features/editor/model";
import type {
  ApiRequest,
  ApiResponse,
  GraphSnapshot,
  RuntimeEvent,
  WebSocketConnectRequest,
  WebSocketConnectResult,
  WebSocketDisconnectRequest,
  WebSocketDisconnectResult,
  WebSocketEvent,
  WebSocketSendRequest,
  WebSocketSendResult,
} from "../types";
import { EvidenceBadge } from "./EvidenceBadge";
import { SlidingTabs } from "./SlidingTabs";

const LazyTerminalConsole = lazy(async () => {
  const module = await import("../features/terminal/TerminalConsole");
  return { default: module.TerminalConsole };
});

type ConsoleTab = "runtime" | "api" | "websocket" | "terminal";
const MAX_VISIBLE_RUNTIME_EVENTS = 50;

interface RuntimeApiConsoleProps {
  events: RuntimeEvent[];
  graph?: GraphSnapshot;
  isRunning: boolean;
  collapsed: boolean;
  embedded?: boolean;
  onToggleCollapsed?: () => void;
  onSelectRuntimeNode: (nodeId: string) => void;
  onSendRequest: (request: ApiRequest) => Promise<ApiResponse>;
  webSocketEvents: WebSocketEvent[];
  webSocketEventSubscriptionReady: boolean;
  onWebSocketConnect: (
    request: WebSocketConnectRequest,
  ) => Promise<WebSocketConnectResult>;
  onWebSocketSend: (request: WebSocketSendRequest) => Promise<WebSocketSendResult>;
  onWebSocketDisconnect: (
    request: WebSocketDisconnectRequest,
  ) => Promise<WebSocketDisconnectResult>;
  initialTab?: ConsoleTab;
  settings?: WorkbenchSettings;
  debugActive?: boolean;
  workspaceScope?: string | number;
}

function runtimeIcon(kind: RuntimeEvent["kind"]) {
  if (kind.includes("database")) return <Database size={13} weight="duotone" />;
  if (kind.includes("http")) return <Globe size={13} weight="duotone" />;
  if (kind.includes("error") || kind.includes("failed")) return <Warning size={13} weight="duotone" />;
  return <TerminalWindow size={13} weight="duotone" />;
}

function metadataText(event: RuntimeEvent, key: string): string | undefined {
  const value = event.metadata[key];
  return typeof value === "string" || typeof value === "number" ? String(value) : undefined;
}

function runtimeGraphNodeId(event: RuntimeEvent, events: RuntimeEvent[]): string | undefined {
  if (event.sourceNodeId) return event.sourceNodeId;
  if (event.kind === "process.stdout" || event.kind === "process.stderr") return undefined;
  const spanId = metadataText(event, "spanId");
  if (!event.runId || !event.traceId || !spanId) return `runtime:${event.id}`;
  const start = events.find((candidate) => candidate.runId === event.runId
    && candidate.traceId === event.traceId
    && metadataText(candidate, "spanId") === spanId
    && metadataText(candidate, "phase") === "start");
  return `runtime:${start?.id ?? event.id}`;
}

function eventLevel(event: RuntimeEvent): "info" | "success" | "warn" | "error" {
  if (event.kind.includes("error") || event.kind.includes("failed")) return "error";
  if (event.kind.includes("warn")) return "warn";
  if (event.kind.includes("completed") || event.kind.includes("started")) return "success";
  return "info";
}

export function RuntimeApiConsole({
  events,
  graph,
  isRunning,
  collapsed,
  embedded = false,
  onToggleCollapsed,
  onSelectRuntimeNode,
  onSendRequest,
  webSocketEvents,
  webSocketEventSubscriptionReady,
  onWebSocketConnect,
  onWebSocketSend,
  onWebSocketDisconnect,
  initialTab = "runtime",
  settings = DEFAULT_WORKBENCH_SETTINGS,
  debugActive = false,
  workspaceScope,
}: RuntimeApiConsoleProps) {
  const [tab, setTab] = useState<ConsoleTab>(initialTab);
  const [terminalVisited, setTerminalVisited] = useState(initialTab === "terminal");
  const apiClient = useApiClientSession({
    onSendRequest,
    debugActive,
    scopeKey: workspaceScope,
  });

  const sortedEvents = useMemo(
    () => [...events]
      .sort((a, b) => Date.parse(a.timestamp) - Date.parse(b.timestamp))
      .slice(-MAX_VISIBLE_RUNTIME_EVENTS),
    [events],
  );
  const graphEvidence = useMemo(() => {
    const counts = { declared: 0, resolved: 0, inferred: 0, observed: 0 };
    for (const node of graph?.nodes ?? []) counts[node.evidence] += 1;
    const maximum = Math.max(1, ...Object.values(counts));
    return { counts, maximum };
  }, [graph?.nodes]);

  return (
    <section className={`console-panel${collapsed ? " is-collapsed" : ""}${embedded ? " is-embedded" : ""}`} aria-label="Runtime and API console">
      <header className="console-header">
        <SlidingTabs
          value={tab}
          onChange={(next) => {
            setTab(next);
            if (next === "terminal") setTerminalVisited(true);
          }}
          label="Console view"
          options={embedded ? [
            { value: "runtime", label: "Runtime", count: events.length },
            { value: "api", label: "API Client" },
            { value: "terminal", label: "Terminal" },
          ] : [
            { value: "runtime", label: "Runtime", count: events.length },
            { value: "api", label: "API Client" },
            { value: "websocket", label: "WebSocket", count: webSocketEvents.length },
            { value: "terminal", label: "Terminal" },
          ]}
          className="console-tabs"
        />
        <div className="console-status">
          <span className={`run-status-dot${isRunning ? " is-live" : ""}`} />
          <span>{isRunning ? "Capturing local run" : "No active run"}</span>
          {onToggleCollapsed && (
            <button type="button" className="icon-button" onClick={onToggleCollapsed} aria-label={collapsed ? "Expand console" : "Collapse console"}>
              <CaretDown className={collapsed ? "rotate-180" : ""} size={14} />
            </button>
          )}
        </div>
      </header>

      {!collapsed && tab === "runtime" && (
        <div className="runtime-console">
          <div className="runtime-column-head">
            <span>Observed timeline</span>
            <small>Output is captured · internal spans require AONE_TRACE_V1</small>
            <EvidenceBadge evidence="observed" />
          </div>
          <div className="runtime-data-overview" aria-label="Indexed evidence distribution">
            {(Object.entries(graphEvidence.counts) as Array<[keyof typeof graphEvidence.counts, number]>).map(([evidence, count]) => (
              <div className="runtime-data-row" key={evidence} data-evidence={evidence}>
                <span>{evidence}</span>
                <div><i style={{ width: `${(count / graphEvidence.maximum) * 100}%` }} /></div>
                <strong>{count}</strong>
              </div>
            ))}
          </div>
          <div className="runtime-events" role="log" aria-live="polite">
            {sortedEvents.length === 0 ? (
              <div className="console-empty">
                <Pulse size={22} weight="thin" />
                <span>Run a profile or send an API request to capture events.</span>
              </div>
            ) : sortedEvents.map((event, index) => {
              const graphNodeId = runtimeGraphNodeId(event, sortedEvents);
              return <button
                type="button"
                key={`${event.id}-${index}`}
                className={`runtime-row level-${eventLevel(event)}`}
                onClick={() => graphNodeId && onSelectRuntimeNode(graphNodeId)}
                aria-disabled={!graphNodeId}
                tabIndex={graphNodeId ? 0 : -1}
                title={metadataText(event, "detail") ?? metadataText(event, "error") ?? event.traceId}
              >
                <time>{new Date(event.timestamp).toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time>
                <span className="runtime-kind">{runtimeIcon(event.kind)}</span>
                <span className="runtime-summary">{metadataText(event, "text") ?? event.label}</span>
                {metadataText(event, "durationMs") && <span className="runtime-duration">{metadataText(event, "durationMs")} ms</span>}
                <EvidenceBadge evidence={event.evidence} compact />
              </button>;
            })}
          </div>
        </div>
      )}

      {!collapsed && tab === "api" && (
        <ApiClientPane session={apiClient} debugActive={debugActive} />
      )}

      {!collapsed && tab === "websocket" && (
        <WebSocketConsole
          events={webSocketEvents}
          eventSubscriptionReady={webSocketEventSubscriptionReady}
          onConnect={onWebSocketConnect}
          onSend={onWebSocketSend}
          onDisconnect={onWebSocketDisconnect}
        />
      )}

      {terminalVisited && (
        <div className="terminal-tab" hidden={collapsed || tab !== "terminal"}>
          <Suspense fallback={<div className="console-empty">Preparing the local terminal…</div>}>
            <LazyTerminalConsole settings={settings} visible={!collapsed && tab === "terminal"} />
          </Suspense>
        </div>
      )}
    </section>
  );
}
