# OpenAPI schema components

Schemas are split by product capability so the root contract remains readable
and below 500 lines. `commands.yaml` contains only Tauri argument wrappers;
workspace, graph, runtime, network, WebSocket, AI, and common payloads stay in
their respective files. The root specification re-exports stable schema names
for deterministic TypeScript generation.
# OpenAPI components

The component files keep the OpenAPI 3.1 Tauri command contract readable and
below the source-size ceiling. `developer.yaml` owns editor, PTY terminal, local
Git, and allowlisted tool-configuration DTOs. `developer-paths.yaml` owns their
command path items. The documented server remains a non-routable surrogate;
these are Tauri IPC commands, not public HTTP endpoints.
