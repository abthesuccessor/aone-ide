# Analyzer feature

The analyzer turns one verified UTF-8 source file into evidence-bearing graph facts. `language.rs`
maps extensions to the supported Tree-sitter grammar and capability level. `extract.rs` owns parsing,
iterative AST traversal, graph-node/edge construction, deadlines, and truthful truncation metadata.
`syntax.rs` contains bounded symbol and TypeScript/JavaScript semantic heuristics. `text.rs` creates
compact labels without retaining or scanning an entire large subtree. `ids.rs` provides stable,
namespace-safe identifiers, while `types.rs` contains the feature's public data contracts.
`api.rs` classifies bounded static HTTP declarations and calls, `openapi.rs` extracts allowlisted
OpenAPI/Swagger operation facts from local JSON or YAML, and `deadline.rs` centralizes budget checks.
`document.rs` emits bounded document facts, while `document_structure.rs` contains deterministic
heading, fence, sentence-boundary, and source-span helpers.

Markdown/MDX, plain `.txt`, reStructuredText, and AsciiDoc files use the `Document` parser path while
retaining the `TextOnly` capability. Markdown ATX/Setext, reStructuredText underline, and AsciiDoc
headings become declared `heading` nodes. Prose becomes declared `sentence` nodes using conservative
punctuation boundaries; this is structural segmentation, not a semantic or discourse claim. Fenced
Markdown code and AsciiDoc `[source]` blocks are excluded. Declared `contains` edges preserve heading
nesting and ownership, and declared `precedes` edges preserve the extracted reading order. OpenAPI
extraction still runs first, and all other text-only extensions keep their file-only behavior.

OpenAPI operation facts retain only method, normalized path, primary tag, operation ID, spec
title/version, framework, source location, and service identity. Descriptions, bodies, examples,
security configuration, server defaults, and remote references are never persisted. Local JSON
Pointer path-item references are resolved with cycle and depth bounds. The allowlisted HTTP method
set includes CONNECT. Rust `#[utoipa::path]`
declarations retain exact handler edges. TypeScript/JavaScript server receivers and known HTTP
clients are separated as producer and consumer evidence; unrelated `.get()` calls are ignored.
Python FastAPI `app`/`router` route decorators with plain static path strings are declared producer
facts and point to the exact decorated function; dynamic names, formatted strings, concatenation,
escapes, and unrelated decorators are not interpreted as routes.
Static `app`/`router`/`*Router`/`server`/`fastify` method registrations require an explicit handler
argument. Their route literal and inline or referenced handler argument retain exact source ranges;
the HTTP interpretation and `handles` relationship remain explicitly inferred. Static `fetch`
defaults to GET, honors an allowlisted literal `method` option, and stays method-unknown when the
options object is dynamic, shorthand, or spread. Axios request objects retain literal URL/method
evidence. These source-only facts do not require an OpenAPI document.
Static outbound HTTP(S) URLs are normalized before graph creation, with user information, query,
and fragment removed, so credentials cannot enter labels or metadata. Relative targets and
sanitized absolute HTTP targets retain distinct `apiTargetForm` metadata.

JavaScript and TypeScript semantic extraction recognizes statically named event subscriptions through
`addEventListener`, `on`, `once`, `addListener`, and Tauri-style `listen`, plus publications through
`emit`, `dispatch`, `publish`, and DOM `dispatchEvent(new Event|CustomEvent(...))`. The event name must
be the first argument and a string literal or interpolation-free template no longer than 240 bytes.
Literals containing interpolation, backslash escapes, or control characters are rejected because the
analyzer does not evaluate JavaScript escapes. Dynamic or ambiguous names remain ordinary inferred
call targets rather than being presented as known runtime events. Subscription calls also require a
listener argument; a bare `on("name")` is not described as an active listener.
Event nodes use `kind=event`, point at the exact literal range, and record their inferred API family,
direction, operation, static-name basis, and confidence. `listensTo` and `emits` edges preserve the
owner relationship and confidence without claiming runtime observation.
All emitted source ranges use 1-based lines and Monaco-compatible UTF-16 code-unit columns; end
positions are exclusive. Tree-sitter's UTF-8 byte columns are converted before graph facts leave
the analyzer.

## Budgets and algorithms

- At most 5,000 extracted facts are retained per file.
- Document facts share that 5,000-fact cap. File metadata records parser/version, format, visited
  lines, considered segments, extracted facts, and truthful truncation reasons.
- Document line discovery is limited to 250,000 lines before heading and sentence extraction.
- AST traversal is iterative, limited to 250,000 visits and depth 512.
- Identifier searches are iterative, limited to 4,096 visits and depth 128.
- Compact labels inspect at most 4,096 input bytes and emit at most 240 bytes plus an ellipsis.
- A caller-supplied workspace deadline is checked before parsing, after parsing, and during traversal.
- Call IDs use the content hash, byte range, AST kind, owner, and ordinal; they never hash a subtree.
- Named definitions use a semantic identity plus deterministic occurrence ordinal. Repeated local
  declarations such as test-scoped `wrapper` functions or Rust associated `Rejection` types therefore
  remain distinct while preserving stable IDs across unrelated function-body edits.
- Static event occurrences use a semantic identity plus occurrence ordinal, so duplicate listeners
  remain distinct while their IDs survive unrelated edits inside the owning function.

## Data flow and security

The scanner supplies verified bytes, their content hash, relative path, and deadline. The analyzer
selects a grammar, parses once, walks named nodes in deterministic order, and emits declared,
inferred, or resolved evidence with source locations. If a structural budget is reached, the file
node records `analysisTruncated`, reasons, visits, and extracted facts, and `parse_errors` is set so
the UI cannot present a partial graph as complete. Provider access and filesystem reads are outside
this feature.

## Tests

`tests.rs`, `api_tests.rs`, and `document_tests.rs` cover every advertised grammar, framework, API and
event semantics, literal provenance,
dynamic-event rejection, deterministic IDs, repeated named declarations and events, duplicate calls,
OpenAPI caps and secret omission, deep AST and fact truncation, expired deadlines, bounded UTF-8
labels, large call subtrees, document routing, fenced-code exclusion, declared document topology,
stable segment IDs, truncation, and exact UTF-16 document ranges.
