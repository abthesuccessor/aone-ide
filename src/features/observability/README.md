# Local trace observability

This module owns the renderer-side controls for Aone's opt-in, loopback-only
OTLP/HTTP JSON trace receiver. `useOtlpReceiver.ts` keeps the workspace-scoped
receiver snapshot synchronized; `OtlpReceiverBar.tsx` exposes lifecycle,
exporter configuration, bounded counts, and explicit in-memory trace deletion.

The native receiver, validation, semantic allowlist, and runtime-event
conversion live under `src-tauri/src/otlp`. Browser demo mode never claims to
open a listening socket.
