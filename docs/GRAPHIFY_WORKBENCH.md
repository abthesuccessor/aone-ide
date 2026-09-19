# Graphify-inspired code knowledge workbench

Aone presents indexed source and supported text documents as one resizable
knowledge workbench. Graphify informed useful interaction patterns—API-first
navigation, progressive graph disclosure, clickable source evidence, and a
clear distinction between extracted and inferred relationships.

Aone is an independent implementation over Rust, SQLite, React, Monaco, and D3.
It does not embed a Graphify runtime, call Graphify, require a Graphify service,
or copy Graphify source. No graph database server is required.

## Workbench layout

The graph activity view lazily mounts five coordinated surfaces:

1. **Knowledge Navigator** — paged API call paths and indexed files.
2. **Indexed source** — Monaco opened at the selected exact range.
3. **Relationships graph** — the current bounded snapshot rendered with D3
   force, pan, zoom, drag, selection, and Fit controls.
4. **Evidence inspector** — selected node, relationships, source, evidence, and
   explicit explanation action.
5. **Terminal/runtime panel** — a vertically resizable or collapsed xterm and
   session-event surface.

The force simulation is presentation only. It does not alter graph identity,
evidence, source ranges, or structural-community membership. The cooperative
debugger is a different, focused four-pane view with a deterministic ordered
API path rather than a force layout.

## Code and document knowledge

Tree-sitter-backed analyzers extract bounded code facts. Supported Markdown,
MDX, TXT, RST, and AsciiDoc files also produce exact `heading` and `sentence`
nodes. Document hierarchy/order is represented only by declared `contains` and
`precedes` edges.

Document ranges are one-based Monaco UTF-16 with exclusive ends. Markdown
backtick/tilde fences in Markdown/MDX/TXT/RST and AsciiDoc `[source]` delimited
blocks are skipped. Discovery stops at 250,000 lines or 5,000 facts per file and
reports the corresponding truncation reason. These facts are structural, not AI
summaries or semantic topics.

Binary PDFs, images, audio, video, and other binary media are unsupported by
the document parser and are not automatically uploaded.

## API and file navigation

The **API call paths** tab follows deterministic cursor pages from the complete
indexed HTTP inventory, up to the renderer's explicit 10,000-operation safety
ceiling. Searching is performed by the backend and does not turn one loaded page
into a claim about the full index.

Selecting an operation coordinates three actions:

- root the bounded execution-flow graph at its canonical operation node;
- open the endpoint or handler's exact source range; and
- expand a bounded endpoint-to-handler/service/repository/model call tree.

The nested tree is bounded to depth 12, 180 nodes, and 32 children per item,
with visible cycle and truncation markers. A missing or inferred edge remains
missing or inferred; the renderer does not fabricate an end-to-end path.

The **Indexed files** tab keeps a bounded file tree and a complete-index path
search. Selecting a file opens it beside the graph rather than leaving the
workbench.

## Graph search and structural communities

The graph search box accepts a control-free query of at most 256 characters.
It requests a depth-two neighborhood capped at 500 nodes. Rust also validates
query depth, result limits, and optional node-kind and edge-kind filters. A
`truncated` snapshot is explicitly incomplete.

Rust deterministically annotates each returned bounded snapshot with structural
communities. Returned edges are treated as undirected and deduplicated;
dangling edges are ignored and isolates receive singleton communities. Each
node records its `communityId`, returned-snapshot `communitySize`,
`deterministicLabelPropagationV1` algorithm, `boundedGraphSnapshot` basis, and
whether the community is complete.

Communities are graph-structural and snapshot-relative. They are not semantic
topics, architectural services, ownership teams, or observed runtime clusters.
When the graph is truncated, `communityComplete` is false.

## Current-view artifacts

**Export current graph view** creates three browser downloads from the active
snapshot:

- `graph.json`, the serialized graph and artifact metadata;
- `GRAPH_REPORT.md`, a readable report of evidence, hubs, communities, counts,
  and limitations; and
- `graph.html`, a standalone offline viewer for the same data.

The artifacts describe the current bounded view only. They are not whole-index
exports. Their truncation state and structural-community basis remain explicit.
The HTML viewer makes no network request, but labels, relative paths, and
metadata may still be sensitive and should be reviewed before sharing.

## Optional AI explanation

AI is not required to open, scan, index, search, navigate, or export a workspace.
The setup dialog provides **Continue code-only**. Hosted API, loopback Ollama,
and the audited Codex CLI adapter are optional explanation transports.

When the engineer explicitly asks to explain a selected node, the renderer
selects a deterministic undirected two-hop corridor from the active bounded
snapshot, selected node first, up to 16 nodes. Rust rebuilds provider-safe
evidence with those nodes and eligible edges between them, capped at 24 total
items and 32 KiB. Every call requires separate native consent.

AI discourse relationships are inferred unless the supplied evidence already
declares or observes the relationship. Provider output cannot change graph
evidence, claim runtime observation, or gain workspace authority.

## Evidence boundary

- `declared` means directly present in source or supported document structure.
- `resolved` means deterministically linked by analysis.
- `inferred` means a heuristic/static correlation or AI interpretation.
- `observed` means admitted during an approved run or API operation.

Selection, force layout, community assignment, API rooting, export, and AI text
never upgrade these evidence classes. Request/response bodies, header values,
SQL text, query binds, environment values, and arbitrary runtime values are not
accepted as graph evidence.

## Run locally

Use the native application for indexing, process/debug authority, the PTY, and
native consent:

```bash
npm ci
npm run desktop
```

`npm run dev` is a browser layout/demo path. It can exercise presentation but
does not supply native filesystem, process, terminal, consent, OTLP, or
debugger authority. Neither command alone proves a packaged or installed app.
