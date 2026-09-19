import { useCallback, useEffect, useRef, useState } from "react";
import {
  connectWebSocket,
  disconnectWebSocket,
  sendWebSocketMessage,
} from "../../lib/bridge";
import type {
  WebSocketConnectRequest,
  WebSocketDisconnectRequest,
  WebSocketEvent,
  WebSocketSendRequest,
} from "../../types";
import { boundedWebSocketEvents, MAX_WEBSOCKET_EVENTS } from "./model";

export function useWebSocketController() {
  const [webSocketEvents, setWebSocketEvents] = useState<WebSocketEvent[]>([]);
  const pendingEventsRef = useRef<WebSocketEvent[]>([]);
  const flushFrameRef = useRef<number | null>(null);

  const appendWebSocketEvent = useCallback((event: WebSocketEvent) => {
    pendingEventsRef.current.push(event);
    if (pendingEventsRef.current.length > MAX_WEBSOCKET_EVENTS) {
      pendingEventsRef.current = pendingEventsRef.current.slice(-MAX_WEBSOCKET_EVENTS);
    }
    if (flushFrameRef.current !== null) return;
    flushFrameRef.current = window.requestAnimationFrame(() => {
      flushFrameRef.current = null;
      const batch = pendingEventsRef.current;
      pendingEventsRef.current = [];
      setWebSocketEvents((current) => boundedWebSocketEvents([...current, ...batch]));
    });
  }, []);

  useEffect(() => () => {
    if (flushFrameRef.current !== null) window.cancelAnimationFrame(flushFrameRef.current);
    flushFrameRef.current = null;
    pendingEventsRef.current = [];
  }, []);

  const handleWebSocketConnect = useCallback(
    (request: WebSocketConnectRequest) => connectWebSocket(request),
    [],
  );

  const handleWebSocketSend = useCallback(
    (request: WebSocketSendRequest) => sendWebSocketMessage(request),
    [],
  );

  const handleWebSocketDisconnect = useCallback(
    (request: WebSocketDisconnectRequest) => disconnectWebSocket(request),
    [],
  );

  return {
    webSocketEvents,
    appendWebSocketEvent,
    handleWebSocketConnect,
    handleWebSocketSend,
    handleWebSocketDisconnect,
  };
}
