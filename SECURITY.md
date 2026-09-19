# Security policy

## v0.1 trust model

Aone is a single-user, local desktop application. It has no account authentication or remote control plane. The selected workspace, explicit source saves, locally approved tool actions, and explicitly launched local processes are the security boundary.

## Protections

- Only the local main WebView receives desktop capabilities. Its core capability is limited to event listen/unlisten plus `window.start_dragging` for the custom titlebar; no direct resize, arbitrary reposition, close, minimize, maximize, or other window authority is granted. The renderer cannot forge backend runtime events. Privileged application commands are explicitly registered and validate their paths, bounds, and structured inputs in Rust.
- Existing-workspace selection is a Rust-owned native folder dialog. Project bootstrap IPC accepts only a bounded public GitHub URL or safe single project-name component, never a renderer-supplied absolute destination.
- Every source path is canonicalized and checked against the active workspace. An external `.env`, `.env.*`, or explicit `local-runtime.env` is accepted only from a Rust-owned native file dialog; the selected regular non-symlink file is opened with `O_NOFOLLOW`, its descriptor identity is checked around the bounded read, and parsed content is zeroized.
- Symlinks cannot be used to escape source-read containment.
- `.env*`, cloud/container/toolchain credential locations, private keys, certificates, Terraform state, Git internals, dependencies, and build output are excluded from indexing.
- The initial Explorer list is capped at 5,000 indexed files. Non-empty search is a parameterized, case-insensitive path-substring query across the complete index; both renderer and Rust enforce a 256-character control-free query, Rust returns at most 200 matches, and renderer generation checks discard stale query/workspace responses.
- The separate workspace Search view requires the current opaque workspace ID and a 1-256-character control-free literal query. Its contentless FTS5 trigram tables return only bounded document-key candidates; Rust then re-opens each source through canonical containment/no-symlink checks and accepts it only when its BLAKE3 hash matches the committed file row. Exact matching is ASCII case-insensitive and non-ASCII case-exact. Results cap at 500, occurrences at 50 per file, attempted source reads at 64 MiB, and a cooperative 750 ms budget is checked between candidate operations; three-or-more-character source, path, and semantic candidate lists cap at 2,000/200/1,000, while one- and two-character source fallback caps at 4,000. Any reached ceiling is reported as truncation, and workspace/request generations reject stale renderer results.
- The curated `systemOverview` Architecture query is a bounded projection over existing
  SQLite facts, not an environment/configuration reader. It returns at most 60
  nodes and 120 edges, excludes test/E2E paths, `.env`/credential-like paths,
  raw `callTarget`/`module` nodes, and inferred `resolvesTo` links, and exposes
  exact/inferred lane placement separately from fact provenance. It never
  synthesizes a cloud provider, webhook, cron schedule, or observed execution.
  A static absolute non-local HTTP(S) label may support inferred External
  placement, but the remote target remains uncontacted/unverified and no
  provider identity is claimed. Other outbound request facts stay in Interfaces.
  The renderer repeats sensitive/noise filtering defensively, but Rust remains
  the authority boundary.
- Only after the curated overview returns, its sorted node IDs (at most 50) root
  the bounded depth-4/500-node Dependencies snapshot. That evidence view may
  include raw, inferred, and test facts; it is not reused as curated
  Architecture/Runtime truth. Scanner-level secret exclusions and
  normal source containment still apply.
- `executionFlow` is a separate bounded, cursor-paged projection rooted in an
  explicit graph/API/runtime fact. Rust applies the scanner hard-deny policy
  before root or descendant selection, excludes tests unless explicitly allowed,
  refuses ungrounded cross-language name resolution as a primary hop, marks
  recursive components without hiding their facts, and caps each response at
  500 nodes and 2,000 edges.
  Each page executes against the current Rust workspace context. The opaque
  cursor binds root, test inclusion, depth, and limit; renderer workspace and
  request generations reject late pages after a rescan or workspace change.
- API + Data is a separate read-only SQLite projection over indexed endpoint
  facts. Rust requires the current opaque workspace ID, accepts at most 128
  query characters, parameterizes values, and returns stable opaque-cursor pages
  of at most 200 operations with exact filtered and indexed totals. Duplicate
  operations merge only within the same service, protocol, method, and normalized
  path. Test/spec/E2E/fixture paths are excluded. Only allowlisted method, path,
  tag, operation ID, framework, service grouping, evidence, and source-location
  fields cross IPC. OpenAPI descriptions, examples, schemas, security data,
  request/response bodies, server defaults, source bodies, and remote references
  are neither persisted in the endpoint facts nor returned by the catalog.
- Static external HTTP(S) client targets are accepted only after URL parsing and
  are stored without username, password, query, or fragment. Relative server
  routes remain local paths. Catalog extraction performs no DNS, HTTP request,
  provider discovery, or code execution.
- Startup holds an initialization ref and keeps application phase non-ready
  until the initial snapshot and matching-generation workspace data are safely
  committed. Open Folder returns before native picker IPC during this interval,
  preventing a late startup result from overwriting a concurrent selection.
- Getting Started performs no host inspection on mount. After an explicit Clone/Create click, Rust resolves and attests the current user's Documents directory only to bind the exact native confirmation. It does not probe the proposed child, mutate disk, or contact GitHub before approval. The destination must be a new direct child; overwrite and symlink/identity changes are rejected. Public GitHub clone is credential-free, shallow, single-branch, and configuration-isolated, checks out ordinary tracked files, and does not initialize submodules or download Git LFS objects. Clone/Create serialize with workspace switching and use the normal scanner/index installer.
- An editor mutation counter is incremented before save/format IPC and synchronously prevents folder opening—without showing discard consent or invoking the picker—until every such operation finishes. After the engineer approves leaving dirty documents, the renderer sets `openingWorkspace` synchronously before invoking the native picker. Until selection plus backend scan/frontend load resolves or cancellation unwinds, old-workspace edit/save/format/close, new source-open, Git mutation, and tool-configuration create/open paths are disabled and guarded in their handlers. One shared renderer ref serializes Open Folder with explicit rescan; Rust independently reserves one renderer-requested workspace operation and waits for active context analysis off the async executor. Reopening the active canonical root uses its installed context/watcher and a transactional GraphStore replacement rather than a second handle to the same database. These controls prevent pre- and post-consent interaction races; backend workspace identities, containment, and consent checks remain authoritative.
- Editing is limited to files already present in the graph index. Save and format requests must carry the backend-issued ID of the workspace from which the file was read, and Rust rejects the operation if that workspace is no longer current. The target must remain contained, non-symlinked UTF-8 text without NUL bytes and no larger than 2 MiB. Rust analyzes new text before any mutation. A BLAKE3 expected-content hash rejects stale saves; a same-directory `create_new` temporary file preserves permissions and is fsynced before the original identity/hash is revalidated. A current-workspace read lock prevents workspace installation from crossing the short final-check/atomic-rename commit section.
- Rename is the editor commit point. Before it, any validation, analysis, write, fsync, identity, hash, or rename failure leaves the old target and returns an IPC error. After it, the command returns the committed content and new hash even if parent-directory fsync, transactional GraphStore replacement, or summary refresh fails. Those post-commit issues are logged without source contents; the watcher or **Rescan Workspace** can recover derived facts, and a failed graph transaction leaves the prior index record rather than a partial record.
- Formatting always has an in-process newline/trailing-whitespace normalizer. Its static capability catalog performs no `PATH` or filesystem inspection. External formatting is limited to backend-owned plans for `rustfmt`, `gofmt`, Black, Prettier, `clang-format`, `google-java-format`, `shfmt`, and Taplo. One native dialog approves a bounded allowlisted `PATH` discovery and a second shows the resolved executable, fixed arguments, and target file before execution. The process receives bounded source over stdin, a reduced environment, bounded output, and a ten-second timeout. Workspace formatter configuration or plugins may still execute code inside that approved process.
- Full and watcher-triggered scans share fail-closed workspace caps: 20,000 qualifying files, 256 MiB of aggregate candidate source, 500,000 extracted facts, and 120 seconds. Each indexed source file is limited to 2 MiB. AST and identifier walks are iterative and have independent depth, visit, and per-file fact caps.
- AI-provider and run-process environments use separate Rust stores. Imported hosted keys never return to the WebView; directly entered keys exist briefly in the masked field and typed IPC request before zeroizing native session retention. Provider credentials are not inherited by launched projects. Ollama is restricted to loopback HTTP and the Codex transport is pinned to an audited native executable/version with a cleared environment and fixed bounded argv. Configuration does not authorize an inference call; every send has a separate native confirmation.
- Process launches are restricted to a currently registered backend-detected profile, its exact program and argument array, and a contained working directory. Rust rejects loader/parent-control run variables, caps and deduplicates selected names, executes the canonical target, revalidates executable/source/directory identities immediately before spawning, and holds one global run slot through stop escalation and leader reaping. The explicit Run/Debug action is the authorization; there is no redundant native process dialog.
- **Observe** is not ambient browser or process introspection. After the explicit
  Run/Debug action Rust injects a per-run nonce and accepts only exact `AONE_TRACE_V1`
  records from that managed run's bounded stdout stream. The parser allowlists
  semantic kinds, bounded process-reported labels, phases, IDs, source ranges,
  rate, and totals; it accepts no attributes, header/body/query values, SQL
  parameters, or payload. Known secrets and sensitive assignments are redacted,
  but instrumentation authors must still keep labels free of user data.
  Process-reported source mapping remains inferred candidate evidence.
  A trace line cannot execute a command, read a path, change the graph, or
  upgrade a static code card to observed. Ordinary browser refreshes and
  uninstrumented internal calls remain unobserved. A rejected envelope becomes
  only a bounded `trace.rejected` timeline diagnostic, never a graph card.
- **Debug** is a separate cooperative protocol for one explicitly instrumented
  managed child. The explicit Debug action binds a fresh nonce and debugger session to
  the current workspace and run; Rust reserves child stdin for fixed
  `AONE_DEBUG_CONTROL_V1` controls and accepts only strict `AONE_DEBUG_V1`
  safe-point records from stdout. Workflow, sequence, control epoch, parent,
  kind, stage, state, source, and shape fields are bounded and allowlisted.
  Unknown, malformed, oversized, duplicate, cyclic, secret-bearing, or
  out-of-order markers fail the debugger protocol closed. Source and semantic
  labels remain process-reported candidate evidence. The schema has no dedicated
  value, body, header/query value, environment value, raw-query, bind, stack-local,
  or arbitrary-metadata field. Its bounded semantic strings are not a DLP
  boundary, so instrumentation authors must keep every identifier and label free
  of user data. Pause and Step act only when the child acknowledges a safe
  point; they are not OS/thread suspension or stack-aware debugging. Debug Stop
  remains native process-group `TERM` then `KILL` authority and does not depend
  on a healthy cooperative channel. Ordinary projects and browser activity are
  not instrumented automatically. See [Instrumented execution debugger](docs/INSTRUMENTED_DEBUGGER.md).
- The integrated terminal accepts only a backend-discovered opaque shell profile ID. Rust revalidates shell and workspace identities after native consent, starts one PTY at the canonical workspace root with a cleared small allowlist environment, excludes ambient credentials and loaded `.env` values, and owns process-group cleanup. On Unix the master becomes `O_NONBLOCK` before reader/writer descriptor cloning; partial-write/flush, read, and full-output-queue retry loops check the shared close flag at most every 5 ms. Decoded input is capped at 64 KiB per call; a dedicated writer accepts non-blocking enqueue only while no more than 64 messages or 256 KiB, including its in-progress write, are outstanding. Saturation is retryable; a closed, finished, missing, or disconnected writer returns `{ accepted: false }`. Writer failure, reader failure after enqueuing its error, or child-wait failure removes and shuts down the registry session. Explicit shutdown sends HUP, invokes the child killer, sends SIGKILL to the original process group, and closes master handles without waiting for retained slave descriptors. The 32-chunk output queue applies cancellation-aware backpressure at 16 KiB chunk boundaries instead of dropping arbitrary bytes from ANSI sequences. Raw terminal bytes are session-only and never enter SQLite, graph evidence, AI context, or WebView local storage.
- Source-control Git commands execute only fixed macOS `/usr/bin/git` with argv, literal contained UTF-8 file paths, a 12-second timeout, bounded output, `--no-renames`, no prompts/pager/hooks/fsmonitor/external diff/signing, and an exact repository-root requirement. Read-only inspection requires an in-place non-symlink `.git` directory; linked-worktree `.git` files, `commondir`, symlinked object directories, and local/HTTP object alternates are rejected. A diff requires exactly one validated relative file path; repository-wide, directory, symlink, escaped, control/direction-override, and scanner-hard-denied paths are rejected. Status and per-file diff enter a private configuration-free snapshot before their first Git process. Initialization uses isolated global/system configuration; stage, unstage, and commit require native consent and are serialized. Commit approval captures the validated staged paths plus a BLAKE3 fingerprint of bounded raw staged-index content, then rejects the commit if either changes during consent. No credential, push, pull, fetch, reset, discard, checkout, delete, or arbitrary-command endpoint exists.
- Git onboarding is a separate read-only, consented metadata path. It parses only bounded regular Git configuration/HEAD files and at most 16 regular non-symlink `.pub` files under the user's SSH directory, returning sanitized identity labels/remotes and public-key fingerprints. Unquoted inline `#`/`;` Git configuration comments are stripped before reporting. Candidate names without the `.pub` suffix are discarded before metadata access; private keys are never opened or statted. Aone does not run Git, SSH, GitHub CLI, contact a network, fork, create a branch, alter a remote, or push during inspection.
- The AI-tool configuration hub's initial eleven-entry catalog is static and performs no home, PATH, or filesystem lookup. A separate single-flight native approval permits metadata-only inspection of fixed Codex, Claude, Cursor, Gemini, Copilot/VS Code, agent, skill, instruction, rule, MCP, and CLI locations; contents never enter IPC and executables are not run. Rust records the canonical trusted root, each existing parent directory's filesystem identity, and any target-file identity before the separate create/open consent, revalidates the same chain afterward and immediately before launch, and rejects containment/identity changes or symlinks. An existing file is never rewritten; missing private directories/templates are created without overwrite using `0700`/`0600`, and fixed `/usr/bin/open -t` opens only the re-attested canonical file.
- Project-environment discovery is explicit, single-flight, and workspace-bound. Before the first native approval it reads no language summary, manifest marker, build/config content, PATH/home-tool location, or executable path. After approval, Rust uses a fixed 39-tool catalog, bounded filename metadata, and a fixed allowlist of already-indexed build/config files: at most 24 regular non-symlink files, 64 KiB each and 512 KiB total, opened through the scanner's containment and hard-deny checks. It extracts only recognized environment names and controlled PostgreSQL/Redis dependency hints; no configuration value enters the report, and every such result is labeled inferred rather than required or ready. Dedicated env files, known credential paths, shell startup files, and AI/CLI tool configuration contents remain excluded. Tool discovery admits at most 64 inherited PATH entries/128 total fixed search directories/four candidates per tool; it invokes no shell and evaluates no startup file. A second native approval shows the exact canonical paths and fixed version arguments for at most 16 relevant probes. Each probe revalidates filesystem identity, clears the environment, uses bounded concurrency/time/output, and cleans its process group. It never installs, downloads, or configures a runtime.
- HTTP runtime request/response events retain only method, a URL with every query value hidden, status/duration, header names, and byte counts. No header or body values enter runtime history; bounded renderer responses independently redact authorization, cookie, password, token, key, and secret fields.
- HTTP events are bound to the workspace that initiated the approved request.
  If that workspace changes before a start/failure/completion event is committed,
  the old event is suppressed and cannot enter or correlate against the new graph.
- API Explorer permits only HTTP(S), rejects `CONNECT`, `TRACE`, embedded URL credentials, fragments, and hop-by-hop request headers, and bounds URL, header count/bytes, body, timeout, and response bytes.
- An explicit API Send authorizes only an IP-literal loopback target, including IPv4 `127.0.0.0/8` and IPv6 `::1` under the shared classifier; no IDE-managed run is required. Every other HTTP target and every WebSocket connection requires native confirmation using the sanitized destination; declining or the HTTP dialog's 120-second timeout performs no DNS or other network activity. After authorization Rust resolves all destination addresses and applies one shared IPv4/IPv6 and cloud-metadata policy. It rejects metadata hostnames and unspecified, link-local, multicast, broadcast, deprecated, reserved, and other special-purpose addresses. Confirmed literal private-network destinations remain available for local development with an explicit warning.
- HTTP pins the validated DNS result into a fresh direct request client; redirects and ambient proxies are disabled. WebSocket opens TCP directly to the validated set while retaining the original hostname for handshake and TLS verification.
- API requests outside the exact IP-literal-loopback exception require native confirmation showing method, canonical origin, and a sanitized URL; header/body values are hidden. `localhost` remains a DNS name and therefore always follows the confirmation path.
- WebSocket connects accept only `ws`/`wss`, reject URL credentials, fragments, duplicate or client-managed handshake headers, and bound the URL, custom headers, subprotocols, timeout, and text/base64 messages.
- Every WebSocket connection requires native consent; the dialog shows the canonical scheme, host, explicit port, path, sorted query-parameter names, and header names, never query/header values or message data. Later sends are restricted to the backend-issued session ID.
- WebSocket connecting/open sessions are capped at eight and each outbound queue at eight messages. Message/frame sizes, buffers, operation timeouts, emitted previews, error details, secret-pattern sets, and per-second renderer events are bounded. Events expose only a sanitized destination origin; sensitive JSON fields, query/header values, and loaded run-environment values are redacted before preview truncation with leftmost-longest matching. Excess inbound messages produce typed dropped-count summaries, and no session-map guard survives an asynchronous wait.
- AI context is a redacted evidence selection, never the whole repository or environment. The renderer cannot alter either provider task: Rust supplies a fixed engineer-flow task or fixed Project Setup interpretation task, selects the configured OpenAI Responses, Anthropic Messages, loopback Ollama, or audited Codex CLI transport, limits input/output, requires native confirmation for every call, and permits only one pending or active call. HTTP transports disable redirects and ambient proxies; Codex receives the prompt over stdin in an empty private workspace with its shell, plugin, browser, computer, workspace, and tool-network capabilities disabled. Generic graph evidence uses separate bounded `systemOverview` and `neighborhood` buckets for the current workspace, preventing concurrent dual graph queries from overwriting each other; selected IDs resolve System first and neighborhood second. Workspace synchronization resets both buckets. Evidence and results remain workspace-bound across cache installation, selection, consent, provider latency, and renderer application. Generic Explain sends no ambient runtime events, and Rust rejects non-empty runtime-event IDs until an event-specific preview/consent exists. Project Setup evidence excludes absolute/alternate executable paths, source/configuration contents, version arguments, probe errors, and environment values; the report ID and workspace are checked before consent, before transmission, and after response.
- Graph queries, runtime history, process output lines, and emitted request evidence are independently bounded.

## Known v0.1 limitations

- A launched project runs with the current user's operating-system permissions. Stronger macOS sandbox profiles are planned.
- Selecting a malicious repository is equivalent to opening untrusted text. Aone does not automatically execute repository commands.
- A user-started run profile may execute untrusted project code and can write anywhere the current macOS user can write. The explicit Run/Debug action is an authority checkpoint, not a process sandbox.
- Saving deliberately modifies the selected workspace. Optimistic concurrency prevents silent overwrite after a detected external edit, but Aone is not a versioned backup system; Git or another backup remains the recovery boundary.
- Once the editor's atomic rename commits, a parent-directory fsync failure is a durability warning rather than a false save failure: the new bytes are visible, but persistence across an immediate operating-system crash is not confirmed. A temporary lag in graph/summary projections is also possible until the watcher or a manual rescan recovers them.
- An approved external formatter may load repository configuration or plugins and therefore can execute untrusted project-controlled code with the current user's permissions. Use the built-in normalizer when that authority is not acceptable.
- The integrated shell is intentionally an unrestricted user shell after approval, not a sandbox. Commands typed into it have the current macOS user's permissions. Terminal output is raw and may contain escape sequences handled by xterm.js.
- Integrated terminal launch is explicitly Unix-only until an equally interruptible non-Unix PTY implementation exists. A real-PTY regression retains an unread slave and verifies that cancellation still releases both cloned-descriptor workers within its bounded deadline; teardown is not justified by assuming blocking I/O eventually returns.
- Read-only Git status/diff use a private configuration-free metadata snapshot and empty attribute tree, so repository or user filters/helpers cannot execute. Explicitly approved staging may invoke user or repository clean filters even though hooks are disabled; the native confirmation warns about this authority. Initialization, index changes, and commits modify repository state. The UI deliberately provides no reset/discard or remote action.
- Opening an allowlisted MCP/CLI, agent, rule, instruction, or skill file delegates further edits to the macOS text editor. Those files may contain secrets or instructions; Aone does not read, validate, redact, or manage existing contents. A newly created template is credential-free, but later edits are outside Aone's boundary.
- Ordinary path-based process execution still has a very small identity-check-to-`exec` race. Fully closing it requires platform-specific descriptor execution or a stronger sandbox and remains future hardening.
- API Explorer can access any IP-literal loopback service after explicit Send without a second native prompt or a managed-run ownership check. Other API targets and all WebSocket targets require confirmation. Destination controls prevent common metadata/special-address requests, but they are not a substitute for operating-system network isolation.
- WebSocket consent authorizes the connection, not each later message. Text or decoded binary supplied by the user is sent to that approved remote session; engineers must avoid pasting secrets they do not intend to transmit.
- TLS interception is not implemented. Aone observes its own HTTP/WebSocket client traffic and explicitly instrumented applications only.
- A formal security diff scan cannot produce a sealed release verdict while this
  workspace has no resolvable Git `HEAD` and every file is untracked. A manual
  frozen-source P0-P2 review found no remaining code defect after debugger
  lifecycle, parser, process-authority, and truth-state fixes, but
  this is not equivalent to a completed baseline diff scan.
- The current 8,582,779-byte ARM64 debugger local-test DMG matches its regular-`0644`
  canonical, target-specific, and staged copies and SHA-256
  `33283907e66468d8d343ce850c8361ff5b1c6907aca0b7636400ebe9df944917`;
  `hdiutil` verified overall CRC `6C50E30D` / HFS CRC `0A246C2B`.
  Deep/strict verification passes with hardened runtime, but the signature is ad
  hoc with no Team ID or staple. Gatekeeper rejection is expected, and manifest
  trust/public flags remain false. Public distribution still requires Developer
  ID signing, a `notarytool` Keychain profile, and a controlled HTTPS origin.
  The release gate rejects raw Apple credential environment variables and binds
  final staged bytes to the expected team, identity, architecture, and minimum
  macOS version before any public-ready claim. A fresh post-debugger package
  package was mounted read-only for the final native interaction check.

## Reporting

Do not include secrets, private source, or customer data in a report. Until a public security contact is configured, report issues directly to the repository owner through a private channel.
