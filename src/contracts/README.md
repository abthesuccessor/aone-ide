# Frontend contracts

This folder is the handwritten boundary between generated IPC schemas and the
React application.

- `models.ts` derives application and paged API-inventory types from
  `src/generated/ipc`; it does not duplicate backend payload shapes.
- `evidence.ts` contains the small presentation mapping used by graph, source,
  runtime, and AI views.
- `src/types.ts` remains a compatibility-only re-export so feature imports stay
  stable.

Regenerate the source schemas with `npm run openapi:generate`, then verify
contract-to-command parity and generated hashes with `npm run openapi:check`.
Generated files are never edited by hand.
