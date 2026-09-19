# Aone roadmap

## v0.1: local execution graph

- macOS Tauri desktop shell and local DMG build configuration; packaging,
  signing, installation, and launch require separate artifact/runtime proof
- full-workbench Getting Started onboarding with native Open Folder, credential-free public GitHub HTTPS clone, or direct-Documents-child creation; clone/create require native consent, refuse overwrite, and enter the shared scanner/index installer
- Rust-owned native workspace selection, repository-safe bounded scanner, and embedded SQLite WAL / `petgraph` graph store
- `Map`-backed 5,000-row initial Explorer plus debounced, generation-guarded complete-index path search with 256-character/200-result bounds and roving keyboard focus
- lazy VS Code-style **Shift+Command/Control+F** workspace Search with a 170 ms debounce, contentless FTS5 trigram document-key candidates, hash-verified exact source ranges, bounded 1-2-character fallback, grouped keyboard navigation, and path/content/symbol/endpoint/event result kinds
- multi-language Tree-sitter code structure plus exact Markdown/MDX/TXT/RST/AsciiDoc `heading`/`sentence` facts and declared `contains`/`precedes` edges; document extraction skips fenced/source blocks, stops at 250,000 lines or 5,000 facts per file, and reports truncation
- bounded D3 force relationship graph with pan/zoom/drag/Fit, exact source selection, a depth-two/500-node backend graph search, and deterministic snapshot-relative structural communities that are explicitly not semantic clusters
- left Knowledge Navigator with complete-index API search, exact filtered/indexed totals, stable 200-row cursor pages, service-aware method/path deduplication, producer/client/unknown coverage, indexed files, source navigation, and Trace-to-workflow entry
- current-bounded-view artifact export as `graph.json`, `GRAPH_REPORT.md`, and standalone offline `graph.html`, retaining truncation/community/evidence limitations rather than claiming a whole-index export
- exact VS Code Dark Modern default plus Light Modern, High Contrast, Cursor Dark, Tokyo Night, and Catppuccin Mocha through one typed workbench/Monaco/xterm registry; bounded font, line-height, tab/space, minimap, format-on-save, terminal-font, cursor, and word-wrap preferences
- safe indexed UTF-8 saves bound to a backend workspace ID/current-workspace commit lock, with a post-leave-consent workspace-opening mutation guard, pre-commit AST analysis, optimistic hashes, atomic rename, and recoverable derived-index refresh; built-in normalization plus a consented fixed external-formatter allowlist
- deterministic backend-owned run profiles, explicit Run/Debug authorization without a redundant native dialog, immediate path-identity revalidation, single-process supervision, shared 50 events/s stdout/stderr FIFO, 100 ms renderer batches, stable graph projection for unsourced logs, stop, and an opt-in nonce-bound bounded `AONE_TRACE_V1` stdout protocol; ordinary browser refreshes remain uninstrumented
- cooperative `AONE_DEBUG_V1` safe-point debugging for one explicitly instrumented managed child, with a four-pane API trace workbench, deterministic ordered workflow nodes, source synchronization, shape-only safe-point inspection, separate explicit request/response views, next-safe-point Pause, one-checkpoint Step hints, Continue, history replay, and authoritative process-group Stop; no ambient browser interception, OS/thread suspension, stack frames, variables, or arbitrary uninstrumented lines
- consented loopback-only, token-gated OTLP/HTTP JSON trace ingestion with bounded semantic metadata, W3C API-client trace-context injection for validated local/private destinations, inferred source correlation, explicit trace deletion, and selection-only replay animation; no collector, remote listener, raw payload values, or reverse execution
- one consented integrated workspace PTY with backend-discovered shell profiles, a clean environment, Unix `O_NONBLOCK` descriptor clones, 5 ms cancel-aware I/O retries, bounded dedicated input writer, byte-order-preserving 32-by-16 KiB output backpressure, failure-driven registry cleanup, HUP/SIGKILL process-group shutdown, guarded pre-open event buffering, visible IPC failures, and xterm.js rendering
- local-only Git status/required-per-file `--no-renames` diff/init/stage/unstage/commit with hard-denied path rejection, staged-content fingerprint recheck, and native mutation consent; separate approved onboarding metadata shows sanitized identities/remotes and public `.pub` fingerprints without private-key access or any GitHub mutation
- static eleven-entry AI-tool catalog and separately consented metadata inspection/launcher for allowlisted Codex, Claude Desktop, Cursor, Gemini, Copilot/VS Code, MCP/CLI, agent, instruction, rule, and skill files; contents stay outside the WebView and canonical parent/target identities are re-attested around create/open consent
- one Project Agent surface combining cached current-workspace readiness, explicit two-stage inspection for 39 fixed runtime/build tools, exact filename stack evidence, bounded names-only environment/dependency hints, deterministic recommendations, and per-question consented AI guidance with no automatic install/configuration
- bounded destination-validated REST/GraphQL-style API requests, explicit Send authorization for IP-literal loopback without a managed-run prerequisite, bounded native confirmation for every other target, and runtime timeline
- bounded destination-validated WebSocket sessions, native per-connect consent, text/base64 sending, and typed real-time lifecycle events
- optional AI through masked direct OpenAI/Anthropic entry, native env import, verified loopback Ollama, or the audited Codex CLI adapter; **Continue code-only** preserves workspace/index/graph use, while an explicit explanation sends selected nodes and eligible edges from a deterministic 16-node two-hop corridor within Rust's 24-item/32-KiB pack after per-call consent; discourse links remain inferred unless supplied
- OpenAPI 3.1 documentation for 56 Tauri commands, AsyncAPI 3.1 documentation for five backend event channels, reproducible OpenAPI Generator TypeScript models, build verification, and local read-only Swagger UI
- feature-folder source structure with declaration/re-export-only Rust `mod.rs` files, per-module READMEs, and an automated 500-line source ceiling
- intentionally no extension host, general-purpose component framework, PGlite/WebView database, bundled graph database daemon, bundled language/runtime toolchain, remote Git mutation, MCP runtime, or binary PDF/image/media parsing or auto-upload; SQLite nodes/edges plus bounded `petgraph` algorithms remain the single-user graph design
- production entry ranking, inferred repository/model/storage path placement,
  target-unverified outbound request wording, and no synthetic provider/cloud/
  webhook/schedule/execution claims in the high-level map

## v0.2: observed request path

- saved local API collections and explicit multi-spec conflict management
- SSE sessions
- persistent run comparison, richer SDK setup assistance, and explicit multi-service trace views over supported instrumentation
- deeper framework adapters for Express, Axum, FastAPI, Spring, Go HTTP, and ASP.NET
- generated Compose run plans stored outside the repository
- workspace-owned run-profile configuration and required-environment-key discovery
- approved environment-plan persistence, exact configuration diff/backup/atomic merge, and optional user-triggered runtime installer handoff; no silent package installation
- LSP-backed completion/diagnostics/rename for selected languages without an unrestricted extension host
- graph generation diff and run comparison
- configurable per-workspace scan/cache budgets with resource telemetry
- signed and notarized public macOS release after a Developer ID certificate, a `notarytool` Keychain profile, and a controlled HTTPS domain are supplied
- Intel or universal packaging after a measured compatibility decision

## v0.3: debugger-adapter integration

- Debug Adapter Protocol client
- `lldb-dap` for Rust/C/C++
- Node Inspector and Python debugpy adapters
- true breakpoints, stack-aware step controls, stack frames, variables, and source synchronization
- observed API-to-handler-to-query paths where instrumentation supports them

## Later platform work

- gRPC reflection/client and an MCP runtime/server manager; the v0.1 configuration launcher does not connect to MCP servers
- SQL, Redis, queue, and cache correlation
- sandboxed WASI plugin SDK
- distributed cross-service trace graph
- persisted historical comparison and state-aware replay research beyond selection-only trace animation
- deep semantic adapters for all target languages
- allocation, lock, task, thread, and operating-system event integrations
- stronger process sandboxing

Generic time-travel debugging and universal zero-instrumentation tracing are research-scale goals. They remain product direction, not v0.1 acceptance criteria.
