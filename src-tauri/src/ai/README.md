# AI feature

## Responsibilities

This feature owns explanations through hosted OpenAI/Anthropic APIs, loopback
Ollama, or the audited Codex CLI adapter. It stores native provider
configuration, retains separate bounded System and neighborhood evidence
projections per workspace, asks for native consent, calls the selected fixed
transport, parses bounded output, and labels the result as inference. The
Project Agent adds stateless, report-grounded setup questions without giving
the provider source access or execution authority.

## Security invariants

- An imported provider key never enters the renderer. A directly entered key exists briefly in the
  masked WebView field and typed Tauri IPC request, then is retained only in zeroizing native session
  memory. Keys never enter returned evidence, status, logs, the graph, or workspace files.
- Rust owns every provider task and instruction. The generic graph explanation ignores its compatibility
  question field. Project Agent questions are separately bounded, redacted, JSON-encoded as untrusted input,
  and disclosed by byte count in a fresh native consent prompt for every call.
- Only one explanation may await consent or execute at a time, enforced by an atomic RAII reservation.
- Evidence item count, metadata, prompt input, provider response, provider error, model name, and output tokens are bounded.
- Evidence text is redacted for loaded secrets, sensitive JSON keys, URL user information, and sensitive query parameters.
- Project Agent context comes only from the retained deterministic environment report. Environment-variable
  names may be included as hints; values, absolute executable paths, run arguments, errors, and source bodies are omitted.
- Native consent reveals only provider, model, evidence count, and the fixed task—not evidence or credentials.
- Hosted and loopback HTTP transports disable redirects and ambient proxies.
  OpenAI storage is requested off; Anthropic and Ollama receive only the same
  bounded transient message payload. Codex runs with a cleared environment,
  fixed arguments, a private empty workspace, bounded I/O, and no shell.

## Data structures and algorithms

- `SecretState` uses `RwLock` for provider configuration and two bounded
  workspace graph-node buckets, plus `AtomicBool` for call admission. Concurrent
  System/neighborhood refreshes update only their own bucket.
- `BoundedEvidence` contains the exact provider input and the references returned to the engineer.
- Evidence is selected by requested IDs, resolving System nodes before
  neighborhood nodes, with append-before-reference accounting so citations
  match transmitted evidence. Runtime-event IDs are rejected until a dedicated
  event-preview consent path exists.
- Recursive JSON redaction preserves structure while URL scanners remove credentials and sensitive parameter values from arbitrary text.

## Flow

1. Reserve the single AI call slot and read backend-only key/model state.
2. Select and redact bounded System-first/neighborhood-second graph evidence;
   workspace synchronization has already reset stale buckets.
3. Show the Rust-owned native provider confirmation.
4. Send through the selected fixed OpenAI, Anthropic, loopback Ollama, audited
   Codex CLI, or audited Claude Code CLI transport.
5. Read and parse a bounded response, mark the answer as inference, publish an inferred event, and release the reservation through `Drop`.

Project Agent follows the same flow for each individual question and returns a
discriminated `completed` or expected `error` outcome. It does not retain chat
history. Anthropic is supported both through the hosted API and through the
Claude Code CLI adapter.

## CLI adapters

`CliKind` owns everything that differs between locally installed agent CLIs:
discovery locations, the version contract, the readiness probe, the fixed
argument list, the process environment, and the response shape. Nothing about a
CLI is taken from the renderer or from ambient configuration, and the resolved
executable identity is re-verified immediately before every run.

- **Codex CLI** is pinned to one audited version because its
  `permissions.*` sandbox keys are version-specific. It runs with a fully
  cleared environment apart from `CODEX_HOME`.
- **Claude Code** is accepted from a minimum audited version, because the flags
  Aone relies on are stable published CLI contract. It runs with `--tools ""`,
  `--disable-slash-commands`, `--strict-mcp-config` and no `--mcp-config`,
  `--setting-sources ""`, `--no-session-persistence`, a schema-constrained
  `--json-schema` answer, and `--max-budget-usd`. `HOME` and `CLAUDE_CONFIG_DIR`
  are reintroduced so the CLI can reach its own credentials; every other
  variable, including any ambient `ANTHROPIC_*`, stays cleared. Readiness
  requires both a zero exit status and `loggedIn` in `claude auth status --json`.

## Tests

The split test modules cover hosted selection and direct-key safety, ambiguous
dual-key rejection, Ollama loopback readiness, the exact audited Codex command
surface, fixed prompt ownership, single-call admission, confirmation
disclosure, all response shapes, independent dual-projection updates,
System-first ID resolution, workspace reset, evidence limits, credential/URL
redaction, URL-shape preservation, and missing-evidence rejection.
Project Agent coverage additionally checks question character/byte bounds,
secret redaction, prompt-injection framing, per-question consent disclosure,
all existing provider request shapes, and its typed outcome contract.
