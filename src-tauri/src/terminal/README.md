# Integrated terminal

This feature provides one local interactive PTY for the open workspace. It is
ephemeral developer tooling: terminal bytes are never persisted, indexed,
attached to graph evidence, or sent to AI.

## Responsibilities

- `profiles.rs` detects executable login shells and binds stable profile IDs to
  canonical filesystem identities.
- `confirmation.rs` asks for native consent immediately before spawn and shows
  the exact shell, profile, canonical working directory, and raw-I/O warning.
- `environment.rs` builds a clean child environment from a small usability
  allowlist. Loaded project values and ambient credentials are not injected.
- `commands.rs` owns the Tauri command boundary and launches only a selected,
  backend-detected shell without a shell command string.
- `input.rs` gives the PTY writer to a dedicated worker. IPC uses non-blocking
  enqueue with a hard limit of 64 outstanding messages and 256 KiB total,
  including the write currently in progress.
- `pty_io.rs` sets `O_NONBLOCK` on the Unix PTY master before portable-pty
  clones its reader and writer descriptors. Non-Unix terminal launch fails
  explicitly until an equally interruptible implementation is available.
- `state.rs` enforces the single-session reservation, owns the PTY handles, and
  closes the child session/process group during explicit close or `Drop`.
- `io.rs` moves raw output through a 32-chunk bounded queue, limits each chunk
  to 16 KiB, caps data events to about 62 per second, and applies cancellable
  PTY backpressure instead of dropping arbitrary ANSI byte sequences.
- `workers.rs` tracks writer, reader, child-wait, and event-pump lifetimes for
  deterministic lifecycle regression tests.
- `events.rs` emits raw bytes only as base64 `aone-terminal-event` payloads.
- `request.rs` bounds decoded input to 64 KiB per call and clamps terminal
  dimensions.

`mod.rs` contains declarations and re-exports only.

## Security invariants

1. The renderer sends an opaque profile ID, never an executable path.
2. Shell and workspace identities are revalidated after consent and before
   spawn.
3. Only one session may be starting or active. `close_all` also invalidates an
   in-flight reservation. A workspace-switch reservation prevents a new
   terminal from opening while the next workspace is prepared.
4. The child starts in the canonical workspace root with a cleared environment.
   `PATH`, `HOME`, locale, temporary-directory, and XDG paths are selectively
   retained; credentials, loader hooks, and loaded `.env` values are excluded.
5. Input commands never perform PTY I/O. They use `try_send`; saturation returns
   an explicit backpressure error without closing the session, while a stopped
   or disconnected worker returns `accepted: false`.
6. Writer partial-write/flush loops, output reads, and full output-queue retries
   check the shared close signal at most every 5 ms. A writer, reader, or
   child-wait failure atomically removes and shuts down the registry session;
   natural reader EOF keeps its existing child-driven lifecycle.
7. Terminal input/output is bounded at IPC and event boundaries and never
   persisted by this feature.
8. Closing removes the session atomically, drops the input sender, signals and
   force-kills the original Unix process group, and closes PTY handles. It never
   locks or joins an I/O worker; non-blocking cloned descriptors observe the
   close signal and terminate even if another process retains the slave. State
   destruction performs the same cleanup.

## IPC contract

- Commands: `list_terminal_profiles`, `open_terminal`, `write_terminal`,
  `resize_terminal`, and `close_terminal`.
- Event: `aone-terminal-event` with `data`, `exit`, and `error` kinds.
- Output `data` is always base64 so arbitrary terminal bytes remain lossless.

Run the deterministic regression suite with:

```bash
cargo test terminal::
```

The suite includes a real Unix PTY whose slave is deliberately retained and
unread; it verifies that cancellation terminates both cloned-descriptor workers
and drops their handles within the bounded deadline.
