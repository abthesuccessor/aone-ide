# Environment-file loading

This feature loads variables only from the exact file selected through a backend-owned native file
dialog. `loader.rs` validates and bounds the attested file; `parser.rs` handles the deliberately small
dotenv grammar. Distribution of provider versus runtime values remains the responsibility of the
commands and runtime features.

## Budgets, data flow, and security

- Only `.env`, `.env.*`, and the explicit `local-runtime.env` filename are accepted.
- The native selection must be a regular non-symlink file and is limited to 1 MiB.
- The selected device/inode/mode/length/change-time identity is captured before
  opening. The canonical target is opened once with `O_NOFOLLOW`, and the same
  descriptor identity is rechecked after the bounded read. A swapped path or a
  file changed during the read is rejected.
- Content is read with a bounded reader and zeroized immediately after parsing.
- Variable names use portable shell identifier syntax, NUL bytes are rejected, and values are never
  expanded or executed. A UTF-8 BOM, CRLF, quotes, comments, and the optional `export` prefix are
  parsed as data. Identical duplicate assignments are deduplicated; conflicting values are rejected
  without echoing either value.
- The function intentionally permits a native-dialog-attested file outside the workspace; renderer
  code cannot supply this privileged path directly.

The loader returns an in-memory name/value map. Commands then place only the
allowlisted OpenAI/Anthropic provider selector, key, and model values in
`SecretState`, or explicitly selected non-provider values in the workspace-scoped runtime state.

## Tests

`tests.rs` verifies BOM/CRLF input, quotes, comments, `export`, literal
non-expansion, duplicate handling, escaped newlines, the native-dialog trust
boundary for external `.env.*` and `local-runtime.env` files, and symlink
rejection. Runner and command tests verify that provider credentials cannot
enter child-process environments and that invalid imports preserve the active
provider.
