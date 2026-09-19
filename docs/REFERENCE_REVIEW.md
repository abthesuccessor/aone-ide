# Lightweight reference review

This document records architectural lessons, not copied source.

## Understand Anything

Reviewed repository: [Egonex-AI/Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) at commit [`32944829e7a63a9fa9c55d811d7f98a9530c6a6a`](https://github.com/Egonex-AI/Understand-Anything/commit/32944829e7a63a9fa9c55d811d7f98a9530c6a6a).

The repository was shallow-cloned into an isolated temporary directory for read-only inspection. No project code or dependency was executed. The checkout and inventory files were deleted afterward, and `/private/tmp` was rechecked for leftovers. No dependency, copied module, generated graph, or vendored source from that checkout remains in Aone.

Helpful patterns independently implemented or retained:

- deterministic Tree-sitter/import facts stay separate from LLM interpretation
- deterministic path ordering, content fingerprints, and bounded filename/manifest evidence drive change detection before optional semantic enrichment
- changed-file facts are atomically replaced and derived cross-file links are rebuilt
- evidence queries are bounded instead of sending a whole repository to AI
- graphs use progressive disclosure, semantic lanes, focus/path concepts, and truthful staleness/diff concepts instead of a whole-repository hairball
- structural analysis remains useful when the AI provider is unavailable
- parser and capability limits are visible rather than hidden
- source-derived text is treated as untrusted evidence, and AI output remains a referenced interpretation rather than canonical structure

These are architectural lessons, not a source transplant: Aone implements them behind its own Rust domain/IPC/storage boundaries and its own React/Tailwind/D3 interaction model.

Patterns intentionally not copied:

- flat JSON as the canonical graph store
- React Flow, ELK, Graphology, Louvain, Fuse.js, Zustand, and dashboard packages
- the project-specific node schema and multi-agent prompt pipeline
- direct JSON graph persistence, automatic `npx`/package installation, broad symlink installers, and hooks that mutate host-agent configuration
- agent-authored temporary scripts and large all-in-one files; Aone keeps audited Rust helpers and a 500-line source gate
- AI-generated business/domain claims without Aone provenance classes

Understand Anything uses no graph database service; its canonical output is JSON and its graph libraries are in memory. It also contains no MCP server/config implementation and no direct OpenAI/Anthropic provider client: its skills inherit a host agent's configured model. Aone therefore uses it as an analysis/graph-UX reference, not as a claim that provider, MCP, or semantic-vector configuration is already solved. Its approach reinforces Aone's choice to keep graph storage embedded and add a separate service only after measured need.

## MiroFish

Reviewed repository: [666ghj/MiroFish](https://github.com/666ghj/MiroFish) at
commit [`b5b53acc57189a4a42e44a23e149dc655c98fe82`](https://github.com/666ghj/MiroFish/tree/b5b53acc57189a4a42e44a23e149dc655c98fe82),
committed 2026-08-03.

The repository was shallow-cloned into an isolated temporary directory for
read-only inspection. No dependency or application code was executed. The clone
and inventory files were removed afterward. MiroFish is AGPL-3.0, so this review
was especially strict: no source, component, style, schema, or generated asset
was copied into Aone.

Helpful interaction ideas independently implemented:

- selecting one definition highlights its immediate neighbors and dims unrelated
  facts, making relationship direction more important than decorative motion
- selected edges receive readable relationship labels and node details retain
  provenance/source context
- parallel edges and self-loops need explicit geometry rather than being drawn as
  indistinguishable straight lines
- zoom, fit, focus, and separate graph/workbench modes help users move between a
  system map and exact source
- graph information should remain usable beside a detail panel instead of
  requiring every fact to fit in one canvas

Patterns intentionally not copied:

- the 1,423-line `GraphPanel.vue` and its circular-node D3 force presentation;
  the same force canvas becomes a dense hairball as the graph grows
- polling and whole-graph reload behavior instead of workspace-bound incremental
  projections
- hard-coded light styling and mouse-first interaction
- a required Zep Cloud graph service and its project-specific entity/edge schema
- MiroFish's simulation, agent workflow, prompt, or product-domain architecture

Aone therefore ships no force behavior. **Flow** and **Map** both use stable
rectangular lanes, orthogonal links, progressive groups, selected-corridor
labels, measured Fit, and full-width Focus. Relationship focus and provenance
were learned as product principles, not transplanted code.

## Terax

Reviewed repository: [crynta/terax-ai](https://github.com/crynta/terax-ai) at commit [`f86ba9f7fbae6995c8faf80001bafa2f5a1ed023`](https://github.com/crynta/terax-ai/commit/f86ba9f7fbae6995c8faf80001bafa2f5a1ed023), current reference version 0.8.6 during this review.

The public 7 MB statement describes compressed installers. The reviewed 0.8.6 release artifacts were approximately:

- macOS ARM DMG: 6,739,016 bytes
- macOS Intel DMG: 7,091,052 bytes
- extracted ARM app allocation: about 8.96 MiB
- ARM executable: about 8.10 MiB

Useful patterns adopted or already present:

- Tauri system WebView instead of bundled Chromium
- strict Rust authority for files, processes, network calls, and secrets
- functional-core/thin-native-shell separation, lazy provider/language loading, and read-only worker roles for analysis
- mutating-agent approval/diff-preview concepts, applied in Aone as native consent plus deterministic plans rather than autonomous edits
- target-specific sidecar and tool-path detection concepts, applied as Aone's fixed permissioned runtime catalog and canonical path/version report
- one codegen unit, LTO, size optimization, abort-on-panic, and stripping
- unused Tauri command removal
- bounded runtime queues
- separate measurement of installer, app, executable, frontend, and runtime memory
- release claims gated by reproducible checks

Patterns intentionally not copied:

- Terax's terminal, PTY, Git, agent, theme, and editor implementation architecture; Aone's newer workbench features were independently designed around its own Rust consent/containment contracts
- Vercel AI SDK and multi-provider frontend provider logic
- broad automatic home/workspace authorization, startup keychain/bootstrap inspection, or login-shell evaluation before explicit user permission
- Atom One as the default theme; Aone defaults to VS Code Dark Modern and uses one registry for workbench, Monaco, and xterm
- bulk provider/component dependencies and large 1,000-plus-line modules; Aone keeps focused local components and the 500-line gate
- CodeMirror migration solely to chase an installer-size headline
- GitHub Release and updater assumptions, because Aone currently has no remote repository or GitHub Actions

The current debugger Aone ARM64 local-test DMG is 8,813,878 bytes (about 8.41 MiB), in
Terax's compressed-installer size class while including Getting Started, Monaco,
xterm.js, D3 selection/zoom, local Git, Project Setup, the configuration hub,
Rust-owned workspace search, the complete API catalog, deterministic Flow/Map,
strict `AONE_TRACE_V1` admission, and backend-only OpenAI/Anthropic setup. It is
current artifact evidence only for this exact post-debugger package.
Comparisons must distinguish
compressed installer, app allocation, logical app size, and executable size.
Replacing Monaco is justified only if later startup
or memory measurements fail the product budget. The UI uses locally owned
Shadcn Base UI source primitives for shared controls and keeps product-specific
workbench styling around them; it deliberately has no extension host.

## Resulting Aone boundary

- Deterministic Rust analysis and existing run profiles produce Project Agent facts first; optional AI can answer one bounded engineer question at a time from the retained report under backend-owned instructions.
- Tool discovery is explicit and two-stage: metadata/path inspection, then exact fixed version probes. It never starts a login shell, downloads a CLI, installs a runtime, or writes project configuration.
- The AI-tool hub begins as a static eleven-entry catalog for MCP/CLI, agent, instruction, rule, and skill locations. Metadata inspection, template creation/opening, and starting any future MCP process remain distinct permissions.
- Existing configuration contents never enter the WebView. New credential-free templates are minimal and opened in the system text editor after canonical path/identity revalidation.
- Skills and agent instruction files are treated as provider-owned text formats, not executable Aone plugins. Aone does not claim cross-provider semantic compatibility or an MCP runtime in v0.1.
- The default graph is a bounded, evidence-backed execution flow rooted in one
  selected entry/API/runtime fact; Architecture and Dependencies retain their
  separate bounded projections. Presentation placement never upgrades inference
  into fact or invents missing providers, webhooks, schedules, or execution.

## Storage decision

Aone keeps SQLite WAL as its embedded canonical structural graph store and petgraph for bounded in-memory algorithms.

Memgraph is much lighter operationally than a typical Neo4j deployment, but it is still a separately operated graph database. Bundling or requiring it would add installation, process lifecycle, port, migration, and resource-management work without solving a measured v0.1 problem. The repository abstraction keeps a future engine change possible.
