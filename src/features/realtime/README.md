# Real-time WebSocket feature

This folder owns the WebSocket client presentation and its small application
controller. Native socket work stays in Rust; React invokes typed Tauri commands
and renders only events received from `aone-websocket-event`.

## Files

- `WebSocketConsole.tsx` renders connection controls, message composer, and a
  bounded event transcript, including explicit dropped-message summaries when
  the backend rate limit protects the renderer.
- `model.ts` parses user input and derives connection state from backend events.
- `useWebSocketController.ts` exposes typed command actions and keeps the latest
  bounded event window for the app shell.

Connect remains disabled until the shared event subscription is installed, so
desktop lifecycle events cannot be lost during startup. Backend events remain
the primary connection state. A successful connect command temporarily confirms
the returned session as open when its `connecting`/`open` events were delivered
before the listener or animation-frame flush; `closed`/`error` events and a
completed disconnect clear that fallback.

Command errors preserve the backend's safe `Error.message` or string rejection;
unknown rejection shapes use a stable action-specific fallback.

Header values and message bodies live only in component memory. They are never
written to browser storage or workspace files. In desktop mode, the Rust backend
validates the destination and asks for native consent before opening a socket.
