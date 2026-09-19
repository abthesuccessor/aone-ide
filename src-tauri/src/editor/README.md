# Editor backend

This module owns source editing and document formatting. The WebView supplies only a
workspace-relative path and text; it cannot select a filesystem root, executable, or
formatter arguments.

## Security boundary

- Existing file operations require a file already present in the workspace
  index. New File uses a Rust-owned native save picker, accepts only a new
  workspace-contained non-sensitive path, and never overwrites an existing file.
- Existing scanner containment and hard-denial rules are applied before every read,
  format, and write.
- Files are UTF-8 text, contain no NUL bytes, and remain within the scanner's 2 MiB
  indexing limit.
- Writes require the BLAKE3 hash last read by the editor. A stale buffer never silently
  overwrites a newer disk version.
- Saves use a same-directory temporary file, preserve permissions, sync data, recheck
  the original, atomically replace it, and sync the parent directory.
- External formatters come from a fixed backend allowlist. The static capability
  catalog performs no PATH or filesystem inspection. A first native dialog
  approves one PATH lookup; if found, a second dialog identifies the exact
  executable, target file, and fixed arguments before execution. Source text is
  never placed in either dialog or process arguments.
- Formatting holds the backend workspace-operation reservation through both
  confirmations, executable revalidation, process completion, and the final
  document precondition check. Folder switching and indexing therefore cannot
  make the approved workspace stale before the formatter starts.
- Formatter subprocesses receive a reduced environment and bounded stdin, stdout,
  stderr, and execution time.

## Formatting behavior

The built-in `Aone safe normalizer` is always available. It normalizes line endings,
removes trailing horizontal whitespace, and supplies one final newline without
rewriting indentation or wrapping source code. When a recognized formatter is on the
backend's PATH, the command offers that tool after native consent. Missing or failed
external tools fall back to the built-in result and report the formatter honestly.

The atomic rename is the save commit point. After it succeeds, the command returns the
saved content and its new hash even if a derived graph-store or workspace-summary
refresh fails. Such post-commit failures are logged without source contents, and the
filesystem watcher or **Rescan Workspace** can rebuild the derived state. Normally the
file's AST and graph-store record are refreshed synchronously while the workspace
analysis reservation is held; the watcher may repeat the same idempotent replacement.
