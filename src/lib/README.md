# Renderer libraries

`bridge.ts` is the single typed boundary for Tauri commands and backend events.
Other files contain deterministic graph and presentation transforms that can be
tested without the desktop runtime. Secret values and arbitrary filesystem paths
must never cross this layer from the WebView.

`runtimeGraph.ts` projects at most the newest 80 graph-relevant runtime events.
Ordinary `process.stdout` and `process.stderr` records remain in the runtime
timeline but are excluded from the graph unless the backend supplies a
`sourceNodeId`. Projections are cached by the static graph and relevant event
object sequence, so appending an unrelated log line returns the identical graph
object and cannot recompute the deterministic execution-flow layout. Flow and
Map both use bounded static SVG cards with pan/zoom; neither starts a force loop.
# Local bridge helpers

This folder contains narrow renderer adapters for Tauri IPC and deterministic
browser-demo behavior. Privileged authority remains in Rust. Feature-specific
bridges keep the main event/runtime bridge below the source-size ceiling and
make sensitive boundaries explicit.

- `bridge.ts` owns workspace, graph, runtime, HTTP, AI, and WebSocket calls.
- `editorBridge.ts` owns optimistic source writes and formatter requests.
- `projectEnvironmentBridge.ts` owns the two-dialog environment inspection and
  separately consented AI explanation, plus non-privileged browser demo data.
- `searchBridge.ts` owns the bounded, workspace-ID-bound indexed search request
  and deterministic browser-demo source/symbol results.
- `errorMessage.ts` converts JavaScript errors and native IPC string rejections
  to single-line, length-bounded presentation text without serializing unknown
  objects into the renderer.
- Terminal, Git, and tool-configuration bridges live beside these files and do
  not expose raw filesystem paths or secret-bearing configuration contents.
