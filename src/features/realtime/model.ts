import type {
  ApiHeader,
  WebSocketEvent,
  WebSocketEventKind,
} from "../../types";

export const MAX_WEBSOCKET_EVENTS = 300;
export const MAX_VISIBLE_WEBSOCKET_EVENTS = 120;

export type WebSocketPhase = "disconnected" | "connecting" | "open" | "error" | "closed";

export function webSocketActionError(error: unknown, fallback: string): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string" && error.trim()) return error;
  return fallback;
}

export function parseWebSocketHeaders(value: string): ApiHeader[] {
  const headers: ApiHeader[] = [];
  for (const [index, rawLine] of value.split("\n").entries()) {
    const line = rawLine.trim();
    if (!line) continue;
    const colon = line.indexOf(":");
    if (colon <= 0) {
      throw new Error(`Header line ${index + 1} must use Name: value`);
    }
    const name = line.slice(0, colon).trim();
    const headerValue = line.slice(colon + 1).trim();
    if (!name || !headerValue) {
      throw new Error(`Header line ${index + 1} must include a name and value`);
    }
    headers.push({ name, value: headerValue });
  }
  return headers;
}

export function parseWebSocketProtocols(value: string): string[] {
  const seen = new Set<string>();
  const protocols: string[] = [];
  for (const rawProtocol of value.split(",")) {
    const protocol = rawProtocol.trim();
    if (!protocol || seen.has(protocol)) continue;
    seen.add(protocol);
    protocols.push(protocol);
  }
  return protocols;
}

export function boundedWebSocketEvents(
  events: WebSocketEvent[],
  limit = MAX_WEBSOCKET_EVENTS,
): WebSocketEvent[] {
  return events.length <= limit ? events : events.slice(-limit);
}

export function eventsForSession(
  events: WebSocketEvent[],
  sessionId: string | null,
): WebSocketEvent[] {
  if (!sessionId) return [];
  return events
    .filter((event) => event.sessionId === sessionId)
    .sort((left, right) => Date.parse(left.timestamp) - Date.parse(right.timestamp))
    .slice(-MAX_VISIBLE_WEBSOCKET_EVENTS);
}

export function deriveWebSocketPhase(events: WebSocketEvent[]): WebSocketPhase {
  const latest = events.at(-1);
  if (!latest) return "disconnected";
  if (latest.kind === "message" || latest.kind === "dropped") return "open";
  return latest.kind;
}
