# Async desktop-event contract

`aone-events.asyncapi.json` documents Rust-originated messages that React listens
to through Tauri's in-process event API. It uses AsyncAPI 3.1 and deliberately
does not advertise a public WebSocket, broker, or HTTP endpoint.

Its five channels are scan progress, runtime evidence, workspace changes,
WebSocket lifecycle/messages, and ephemeral base64 terminal data/exit/error
events. The terminal channel does not mean terminal output is persisted or
promoted to graph or AI evidence. Its backend pump uses a bounded 32-by-16 KiB
synchronous queue: pressure holds the PTY reader in cancellation-aware 5 ms
retries and preserves ANSI byte order instead of producing a synthetic
dropped-output event, and data events are
spaced by at least 16 ms (about 62.5/s). The renderer separately
retains at most 32 events that arrive while Open consent is in flight and shows
an omission notice if that pre-session buffer overflows.

Terminal input is command traffic, not an AsyncAPI event. Its OpenAPI contract
caps one decoded write at 64 KiB; Rust non-blockingly queues at most 64
outstanding messages or 256 KiB including the writer's in-progress write.
On Unix the PTY master is `O_NONBLOCK` before descriptor cloning, and 5 ms
cancel-aware read/write/full-queue retries make teardown independent of retained
slave descriptors. Reader failure enqueues an error before registry shutdown;
writer or child-wait failure also shuts down the registered session.
If command responses report `{ accepted: false }`, React best-effort invokes
`close_terminal` before clearing the matching stale session and showing an error.

OpenAPI owns command/request/response schemas. AsyncAPI reuses those payload
schemas so pushed event documentation cannot silently invent a second model.
Run `npm run asyncapi:check` to validate the document and its parity with the
renderer listener surface.
