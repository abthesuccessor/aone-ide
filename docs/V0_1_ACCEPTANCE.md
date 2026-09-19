# Aone v0.1 acceptance

This file defines acceptance criteria. It does not assert that Aone is currently
packaged, installed, signed, notarized, deployed, or connected to a live project.
Source inspection, automated tests, native runtime checks, packaging, and live
workspace exercises are separate evidence layers and must be reported separately.

## Source and structure

- [ ] `npm run structure` accepts every checked source file and module README.
- [ ] `npm run openapi:check` confirms generated schema and registered-command
  parity for the current tree.
- [ ] `npm run asyncapi:check` confirms event-schema and renderer-listener parity.
- [ ] The frontend build and focused UI tests pass for the current tree.
- [ ] Rust formatting, strict Clippy, and all-target tests pass for the current tree.
- [ ] Any package, install, signature, notarization, or live-workspace claim has
  its own dated artifact/runtime evidence instead of inheriting source-test proof.

## Code-only startup

- [ ] The first-use setup explains that AI is optional and exposes
  **Continue code-only**.
- [ ] A user can open, scan, index, search, navigate, inspect, and export a
  workspace without configuring an AI provider.
- [ ] Hosted OpenAI/Anthropic, loopback Ollama, and the audited Codex CLI adapter
  remain optional explanation transports.
- [ ] Configuring a transport sends no evidence. Every explanation is initiated
  explicitly and receives a separate native consent prompt.

## Workspace and facts

- [ ] Workspace authority originates from Rust-owned native selection or the
  bounded Clone/Create flows; the renderer cannot submit an absolute root.
- [ ] Scanning honors containment, ignore, secret/dependency/build exclusions,
  individual-file limits, and aggregate file/byte/fact/time ceilings.
- [ ] TypeScript, JavaScript, Rust, Python, Go, Java, C#, C++, and Kotlin use
  their declared Tree-sitter capability without overstating compiler resolution.
- [ ] `.md`, `.mdx`, `.txt`, `.rst`, `.adoc`, and `.asciidoc` safe UTF-8 files
  route through the document parser while retaining `textOnly` capability.
- [ ] Markdown ATX/Setext, RST underline, and AsciiDoc `=` headings produce exact
  `heading` nodes; prose produces exact `sentence` nodes.
- [ ] Document relationships are declared `contains` and `precedes` edges with
  no invented confidence. Locations use one-based Monaco UTF-16 columns and
  exclusive ends.
- [ ] Markdown-style fences in Markdown/MDX/TXT/RST and AsciiDoc `[source]`
  delimited blocks are skipped. A file stops at 250,000 visited lines or 5,000
  facts and reports the corresponding truncation reason.
- [ ] Binary PDFs, images, audio, video, and other binary media remain
  unsupported and are not automatically uploaded.

## Knowledge Navigator and source

- [ ] The graph activity view lazily mounts the Knowledge Navigator without
  changing Explorer or debugger behavior.
- [ ] **API call paths** follows deterministic backend cursor pages and keeps
  complete-index totals distinct from the currently loaded page.
- [ ] Selecting an endpoint can trace the bounded execution flow, open its exact
  source, and expand the bounded endpoint/handler/service/repository/model tree.
- [ ] Missing and inferred relationships are not presented as a complete
  observed end-to-end path.
- [ ] **Indexed files** exposes its bounded tree and complete-index path search;
  selecting a file opens source beside the relationship graph.
- [ ] Source ranges, graph selection, call-tree selection, and Monaco navigation
  stay synchronized without upgrading evidence.
- [ ] Existing indexed safe UTF-8 source opens in Monaco and retains dirty-state,
  optimistic-hash save, containment, and stale-workspace protections.

## Graph search, layout, and communities

- [ ] The main relationship view is a D3 force-directed graph with pan, zoom,
  drag, Fit, node selection, and exact source navigation.
- [ ] Force position is presentation only and never changes topology, evidence,
  source ranges, or community assignment.
- [ ] The separate cooperative debugger renders one API-scoped ordered path in
  its four-pane trace workbench; its no-force layout is not documentation for
  the main graph.
- [ ] Relationship search accepts at most 256 control-free characters and asks
  Rust for a depth-two neighborhood capped at 500 nodes.
- [ ] Rust bounds query depth/size and node/edge-kind filter lists, ignores stale
  renderer generations, and reports truncation rather than implying completeness.
- [ ] Each returned snapshot receives deterministic structural communities:
  edges are undirected/deduplicated, dangling edges ignored, and isolates
  singleton.
- [ ] Every returned node exposes `community:<32 hex>`, snapshot member count,
  `deterministicLabelPropagationV1`, `boundedGraphSnapshot`, and completeness.
- [ ] A truncated snapshot marks communities incomplete. UI and reports describe
  them as structural groupings, never semantic topics, services, teams, owners,
  or observed runtime clusters.

## Current-view artifacts

- [ ] **Export current graph view** creates `graph.json`, `GRAPH_REPORT.md`, and
  `graph.html` from the active bounded snapshot.
- [ ] All three artifacts retain counts, evidence, limitations, truncation, and
  structural-community basis appropriate to that current view.
- [ ] Documentation and UI do not call the artifacts complete-index exports.
- [ ] The standalone HTML viewer makes no network request.
- [ ] The export flow warns that labels, relative paths, and metadata may remain
  sensitive and should be reviewed before sharing.

## Optional AI explanation

- [ ] Selecting **Explain** constructs a deterministic undirected two-hop
  corridor from the active bounded graph, selected node first, capped at 16
  frontend-selected node IDs.
- [ ] Rust rebuilds provider-safe evidence from those nodes and eligible edges
  between them, capped at 24 total evidence items and 32 KiB.
- [ ] The renderer question cannot replace the fixed Rust-owned task, only one
  request may be pending/active, and workspace authority is rechecked across
  evidence selection, consent, and provider latency.
- [ ] AI discourse relationships are labeled inferred unless supplied evidence
  already declares or observes the relationship.
- [ ] Provider output never changes canonical graph evidence or claims runtime
  observation. Credentials and evidence contents are absent from consent text.

## Evidence and runtime truth

- [ ] `declared`, `resolved`, `inferred`, and `observed` retain distinct meanings
  in graph, inspector, export, and explanation views.
- [ ] Only admitted `AONE_TRACE_V1`, consented OTLP, cooperative debug safe
  points, or explicitly approved API actions create corresponding observed
  session evidence.
- [ ] Process-reported source correlation remains inferred unless a stronger
  backend authority exists. Downstream static code is never upgraded because an
  observed event points toward it.
- [ ] Uninstrumented browser/application activity is not presented as internal
  functions, SQL, payloads, async work, or a complete call chain.
- [ ] Debug remains an explicit cooperative protocol, not DAP, arbitrary-line
  stepping, stack/thread suspension, browser attachment, or reverse execution.
- [ ] HTTP/runtime retention remains value-free; terminal and WebSocket streams
  remain bounded and are not silently added to SQLite or AI evidence.

## Desktop authority and safety

- [ ] Filesystem reads/writes, process start/stop, PTY, Git mutation, network
  destination validation, secrets, and provider traffic remain Rust-owned.
- [ ] Each authority-bearing process, network, terminal, Git, formatter,
  tool-configuration, and AI action uses the explicit-action or native-approval
  boundary described by the IPC contract.
- [ ] Project Agent may restore only a matching approved in-memory report on
  mount; new inspection still uses two approvals and never installs a tool or
  edits project configuration.
- [ ] Local Git exposes only the bounded exact-root surface documented in the
  IPC contract; no push, pull, fetch, reset, discard, checkout, remote mutation,
  or arbitrary Git command is implied.
- [ ] The product does not imply an extension host, LSP, MCP runtime, graph
  database daemon, binary-document pipeline, or bundled project runtime.

## Manual proof still required for distribution claims

- [ ] A fresh native build is produced from the exact accepted source revision.
- [ ] The exact artifact passes independent size, signature, and checksum checks.
- [ ] Developer ID signing and Apple notarization are verified when public
  distribution is claimed.
- [ ] The exact package is installed/launched on each supported architecture.
- [ ] A controlled workspace exercise covers index, navigator, force graph,
  search, source opening, current-view export, code-only startup, and one
  separately consented explanation.
- [ ] Live API/WebSocket/debugger claims use controlled fixtures and explicitly
  distinguish observed, inferred, and absent evidence.
