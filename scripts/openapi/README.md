# OpenAPI tooling

This module owns deterministic IPC contract generation and drift validation.

- `generate.mjs` runs the pinned official OpenAPI Generator container and
  atomically replaces `src/generated/ipc`.
- `check.mjs` validates the source and generated digests, TypeScript major
  version, generator identity, and parity with Tauri's registered commands.
- `docs.mjs` serves the contract through the digest-pinned Swagger UI 5.32.11 container on
  `127.0.0.1` with network submission disabled.
- `contract.mjs` contains path confinement and deterministic hashing helpers.

The generator is a development tool only. Docker image layers, Java, and the
generator itself are not copied into the application or DMG.
