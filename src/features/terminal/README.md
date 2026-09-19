# Integrated terminal

The renderer uses xterm.js 6 with the fit addon for a real PTY surface. It
selects only backend-discovered shell profile IDs, forwards bounded input and
resize messages, and renders base64 terminal bytes without persistence.

Rust owns native consent, canonical workspace cwd, isolated environment,
single-session reservation, PTY process lifetime, and cleanup. Terminal output
is deliberately excluded from the graph, AI evidence, SQLite, and browser demo
storage. Opening a different workspace is refused while a terminal is active.

The renderer discovers profiles before subscribing and enables **New** only
after subscription succeeds, so partial setup cannot leak a listener. While a
native open is in flight, at most 32 early events are retained and then bound
to the returned session ID. Older overflow events are discarded with a visible
notice; the queue is cleared on success, failure, or unmount.

If the backend rejects input or resize for the current session, the renderer
best-effort closes that session before clearing its local state. This releases
any remaining backend reservation while keeping a visible lifecycle error.
