# Aone IPC contract

`aone-ipc.openapi.yaml` is the machine-readable contract for Aone's public
53 Tauri commands. The paths are documentation paths, not HTTP endpoints. Each
operation carries `x-aone-tauri-command`. The non-routable
`https://tauri.invalid/ipc` server is a documentation surrogate; the transport
remains Tauri `invoke`, not HTTP.

The component files divide the contract by feature so schemas remain readable
and reviewable. TypeScript bindings are generated from this contract; generated
files must never be edited by hand.

The 17 developer-workbench operations are split into
`components/developer-paths.yaml`, with their editor, terminal, local Git, and
AI-tool descriptor payloads in `components/developer.yaml`. Project Setup and
its AI explanation use focused path/schema component files. They complement the
core workspace/graph/runtime/network/AI operations in the root document. Three
permission-first project bootstrap and Git-onboarding operations use the focused
`components/onboarding-paths.yaml` and `components/onboarding.yaml` files.
`get_workspace_files` documents both the 5,000-row unfiltered inventory ceiling
and the 256-character/200-result complete-index substring-search ceiling.
`search_workspace` is split through `components/search-paths.yaml` and
`components/search.yaml`; it documents the workspace-bound 500-result code-search
surface and its exact path/content/symbol/endpoint/event source ranges.
The frozen generated tree digest is
`8af564cb82c9c5bb4c1c4262a88974935a7f63c567d4c7e4a8b63f66ae6cd9fe`;
`npm run openapi:check` also verifies that Git diff requires one file path and
that editor save/format payloads carry the backend-issued `workspaceId`.

Commands can reject with a serialized `AoneError` string. Native pickers and
consent dialogs can also return `null` when the user cancels. Secret values are
never part of this contract.

Run the generator and drift check from the repository root:

```bash
npm run openapi:generate
npm run openapi:check
```

Use `npm run openapi:docs` for a pinned local Swagger UI with request submission
disabled. It is a searchable command/schema reference only; Aone does not
expose a localhost HTTP API. WebSocket, terminal, and other Tauri event channels are
documented in `../asyncapi/` because OpenAPI does not model asynchronous
message channels truthfully.
