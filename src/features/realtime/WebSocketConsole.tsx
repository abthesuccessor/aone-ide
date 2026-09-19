import {
  ArrowsClockwise,
  CircleNotch,
  LockKey,
  PaperPlaneTilt,
  Plug,
  PlugsConnected,
  Warning,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import type {
  WebSocketConnectRequest,
  WebSocketConnectResult,
  WebSocketDisconnectRequest,
  WebSocketDisconnectResult,
  WebSocketEncoding,
  WebSocketEvent,
  WebSocketSendRequest,
  WebSocketSendResult,
} from "../../types";
import {
  deriveWebSocketPhase,
  eventsForSession,
  parseWebSocketHeaders,
  parseWebSocketProtocols,
  type WebSocketPhase,
  webSocketActionError,
} from "./model";

type PendingAction = "connect" | "send" | "disconnect" | null;

interface WebSocketConsoleProps {
  events: WebSocketEvent[];
  eventSubscriptionReady: boolean;
  onConnect: (request: WebSocketConnectRequest) => Promise<WebSocketConnectResult>;
  onSend: (request: WebSocketSendRequest) => Promise<WebSocketSendResult>;
  onDisconnect: (
    request: WebSocketDisconnectRequest,
  ) => Promise<WebSocketDisconnectResult>;
}

function phaseLabel(phase: WebSocketPhase): string {
  if (phase === "disconnected") return "Not connected";
  if (phase === "connecting") return "Connecting";
  if (phase === "open") return "Open";
  if (phase === "closed") return "Closed";
  if (phase === "error") return "Error";
  return phase;
}

function eventSummary(event: WebSocketEvent): string {
  if (event.kind === "message") return event.data ?? "Empty message";
  if (event.kind === "dropped") {
    return event.detail ?? `${event.droppedCount ?? 0} inbound messages omitted`;
  }
  return event.detail ?? phaseLabel(event.kind);
}

function shortSessionId(sessionId: string): string {
  return sessionId.length <= 18 ? sessionId : `${sessionId.slice(0, 8)}…${sessionId.slice(-6)}`;
}

export function WebSocketConsole({
  events,
  eventSubscriptionReady,
  onConnect,
  onSend,
  onDisconnect,
}: WebSocketConsoleProps) {
  const [url, setUrl] = useState("ws://127.0.0.1:3000/ws");
  const [headers, setHeaders] = useState("x-aone-trace: true");
  const [protocols, setProtocols] = useState("json");
  const [timeoutMs, setTimeoutMs] = useState("15000");
  const [message, setMessage] = useState('{"type":"ping"}');
  const [encoding, setEncoding] = useState<WebSocketEncoding>("text");
  const [preferredSessionId, setPreferredSessionId] = useState<string | null>(null);
  const [openSessionFallbackId, setOpenSessionFallbackId] = useState<string | null>(null);
  const [disconnectedByCommandId, setDisconnectedByCommandId] = useState<string | null>(null);
  const [connectFailed, setConnectFailed] = useState(false);
  const [pending, setPending] = useState<PendingAction>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const newestEvent = events.at(-1);
  const newestSessionId = newestEvent?.sessionId ?? null;
  useEffect(() => {
    if (newestEvent?.kind === "connecting") {
      setPreferredSessionId(newestEvent.sessionId);
    }
  }, [newestEvent?.kind, newestEvent?.sessionId]);
  const activeSessionId = pending === "connect"
    ? (newestSessionId ?? preferredSessionId)
    : (preferredSessionId ?? newestSessionId);
  const sessionEvents = useMemo(
    () => eventsForSession(events, activeSessionId),
    [activeSessionId, events],
  );
  const eventPhase = deriveWebSocketPhase(sessionEvents);
  const hasOpenFallback = activeSessionId !== null && openSessionFallbackId === activeSessionId;
  const phase = connectFailed && eventPhase === "connecting"
    ? "error"
    : activeSessionId !== null && disconnectedByCommandId === activeSessionId
    ? "closed"
    : hasOpenFallback && (eventPhase === "disconnected" || eventPhase === "connecting")
      ? "open"
      : eventPhase;
  useEffect(() => {
    const latest = sessionEvents.at(-1);
    if (latest?.kind !== "closed" && latest?.kind !== "error") return;
    setOpenSessionFallbackId((current) => current === latest.sessionId ? null : current);
  }, [sessionEvents]);
  const isOpen = phase === "open";
  const canConnect = eventSubscriptionReady
    && pending === null && phase !== "connecting" && !isOpen;
  const canDisconnect = pending === null && activeSessionId !== null
    && phase !== "closed" && phase !== "disconnected";

  const connect = async () => {
    setPending("connect");
    setActionError(null);
    setConnectFailed(false);
    try {
      const timeout = Number(timeoutMs);
      if (!Number.isInteger(timeout) || timeout < 250 || timeout > 60_000) {
        throw new Error("Timeout must be an integer from 250 to 60000 milliseconds");
      }
      const result = await onConnect({
        url: url.trim(),
        headers: parseWebSocketHeaders(headers),
        protocols: parseWebSocketProtocols(protocols),
        timeoutMs: timeout,
      });
      setPreferredSessionId(result.sessionId);
      setOpenSessionFallbackId(result.sessionId);
      setDisconnectedByCommandId(null);
    } catch (error) {
      setConnectFailed(true);
      setActionError(webSocketActionError(error, "WebSocket connection failed"));
    } finally {
      setPending(null);
    }
  };

  const send = async () => {
    if (!activeSessionId) return;
    setPending("send");
    setActionError(null);
    try {
      await onSend({ sessionId: activeSessionId, data: message, encoding });
    } catch (error) {
      setActionError(webSocketActionError(error, "WebSocket message failed"));
    } finally {
      setPending(null);
    }
  };

  const disconnect = async () => {
    if (!activeSessionId) return;
    setPending("disconnect");
    setActionError(null);
    try {
      await onDisconnect({ sessionId: activeSessionId });
      setOpenSessionFallbackId((current) => current === activeSessionId ? null : current);
      setDisconnectedByCommandId(activeSessionId);
    } catch (error) {
      setActionError(webSocketActionError(error, "WebSocket disconnect failed"));
    } finally {
      setPending(null);
    }
  };

  return (
    <div className="websocket-console">
      <section className="websocket-composer" aria-label="WebSocket client">
        <div className="websocket-consent-note">
          <LockKey size={13} weight="duotone" aria-hidden="true" />
          <span>
            {eventSubscriptionReady
              ? "Desktop asks for native consent before connecting. Inputs stay in memory."
              : "Preparing the real-time event listener before connections are enabled."}
          </span>
        </div>

        <div className="websocket-connect-line">
          <input
            aria-label="WebSocket URL"
            value={url}
            onChange={(event) => setUrl(event.target.value)}
            spellCheck={false}
          />
          <button
            type="button"
            className="send-button"
            onClick={() => void connect()}
            disabled={!canConnect || !url.trim()}
          >
            {pending === "connect"
              ? <CircleNotch className="spin" size={13} />
              : <Plug size={13} weight="fill" />}
            {pending === "connect" ? "Requesting" : "Connect"}
          </button>
          <button
            type="button"
            className="websocket-disconnect-button"
            onClick={() => void disconnect()}
            disabled={!canDisconnect}
          >
            {pending === "disconnect"
              ? <CircleNotch className="spin" size={13} />
              : <PlugsConnected size={13} />}
            Disconnect
          </button>
        </div>

        <div className="websocket-settings">
          <label>
            <span>Headers</span>
            <textarea
              aria-label="WebSocket headers"
              value={headers}
              onChange={(event) => setHeaders(event.target.value)}
              spellCheck={false}
            />
          </label>
          <div className="websocket-setting-stack">
            <label>
              <span>Subprotocols</span>
              <input
                aria-label="WebSocket subprotocols"
                value={protocols}
                onChange={(event) => setProtocols(event.target.value)}
                spellCheck={false}
              />
            </label>
            <label>
              <span>Timeout ms</span>
              <input
                aria-label="WebSocket timeout"
                type="number"
                min="250"
                max="60000"
                value={timeoutMs}
                onChange={(event) => setTimeoutMs(event.target.value)}
              />
            </label>
          </div>
        </div>

        <div className="websocket-message-line">
          <select
            aria-label="WebSocket message encoding"
            value={encoding}
            onChange={(event) => setEncoding(event.target.value as WebSocketEncoding)}
          >
            <option value="text">Text</option>
            <option value="base64">Base64</option>
          </select>
          <textarea
            aria-label="WebSocket message"
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            spellCheck={false}
          />
          <button
            type="button"
            className="send-button"
            onClick={() => void send()}
            disabled={!isOpen || pending !== null || !message}
          >
            {pending === "send"
              ? <CircleNotch className="spin" size={13} />
              : <PaperPlaneTilt size={13} weight="fill" />}
            Send
          </button>
        </div>
        {actionError && (
          <div className="websocket-action-error" role="alert">
            <Warning size={13} /> {actionError}
          </div>
        )}
      </section>

      <section className="websocket-transcript" aria-label="WebSocket event transcript">
        <header>
          <span className={`websocket-phase phase-${phase}`}>
            <i aria-hidden="true" /> {phaseLabel(phase)}
          </span>
          {activeSessionId && (
            <code title={activeSessionId}>{shortSessionId(activeSessionId)}</code>
          )}
          <small>{sessionEvents.length}/120 events</small>
        </header>
        <div className="websocket-events" role="log" aria-live="polite">
          {sessionEvents.length === 0 ? (
            <div className="console-empty">
              <ArrowsClockwise size={21} weight="thin" />
              <span>Connection state and bounded messages appear from backend events.</span>
            </div>
          ) : sessionEvents.map((event, index) => (
            <article className={`websocket-event event-${event.kind}`} key={`${event.timestamp}-${index}`}>
              <time>
                {new Date(event.timestamp).toLocaleTimeString([], {
                  hour12: false,
                  hour: "2-digit",
                  minute: "2-digit",
                  second: "2-digit",
                })}
              </time>
              <strong>{event.kind}</strong>
              <code title={event.data}>{eventSummary(event)}</code>
              <small>
                {event.encoding ?? event.protocol ?? "event"}
                {event.byteLength === undefined ? "" : ` · ${event.byteLength} B`}
                {event.truncated ? " · truncated" : ""}
              </small>
            </article>
          ))}
        </div>
      </section>
    </div>
  );
}
