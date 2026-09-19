# WebSocket feature

This module powers the real-time API console without exposing a general browser
network bridge.

- `request.rs` validates URLs, header names and values, subprotocols, timeouts,
  session IDs, and bounded outbound text/base64 messages.
- `destination.rs` resolves the host once after approval, rejects metadata, link-local,
  multicast, unspecified, broadcast, transition, documentation, and other
  special-purpose addresses, then retains only those validated socket addresses.
- `confirmation.rs` requires native consent before DNS resolution or any other
  network activity for every connection. The dialog
  shows the canonical scheme, host, explicit port, path, query parameter names,
  and header names, never query/header values or message data.
- `transport.rs` connects directly to a validated address while keeping the
  original hostname for TLS verification, disables redirects and proxies by
  construction, and runs one bounded reader/writer loop per session.
- `state.rs` uses `DashMap` only for the concurrently accessed session registry.
  An eight-permit semaphore includes connections still handshaking; each open
  session has an eight-message outbound channel and an independent shutdown
  signal. No map guard survives an `await`.
- `events.rs` emits the typed `aone-websocket-event` lifecycle stream. Text
  previews are capped at 256 KiB; binary previews retain at most 192 KiB before
  base64 encoding; original byte lengths and truncation are reported.
- `redaction.rs` recursively hides sensitive JSON fields and redacts connection
  secrets plus current loaded environment values with a cached leftmost-longest
  matcher. Patterns are deduplicated and capped at 256 values/256 KiB total;
  exceeding either cap hides the payload. Output is written through a cap.
- `rate_limit.rs` admits 120 inbound message events per session in each fixed
  one-second window. Excess messages are consumed and counted; a typed `dropped`
  summary is emitted at the next window or immediately before `closed`.
- `commands.rs` exposes the three Tauri IPC commands.
- `tests.rs` and `redaction_tests.rs` cover request, destination, transport,
  redaction, event, rate-limit, and registry limits.

The implementation intentionally has no ambient proxy support and does not
follow handshake redirects. Literal private or loopback destinations receive a
pre-resolution warning; resolved names remain pinned to the post-approval,
policy-validated address set.
