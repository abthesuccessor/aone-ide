# Development and supported environments

Aone currently targets a macOS desktop proof of concept. A repeatable toolchain,
locked dependencies, automated checks, and manual acceptance checks reduce
defects; they cannot establish that the application has no bugs or runs in every
environment.

## Platform scope

| Environment | Current status |
| --- | --- |
| macOS on Apple Silicon | Primary development environment; configured minimum macOS 15.0 |
| macOS on Intel | The release script accepts an Intel target; a target option does not establish tested support |
| Linux desktop | Unverified; macOS tool-opening behavior and packaging need an implementation and native validation |
| Windows desktop | Unsupported by the current native source; Git inspection imports Unix filesystem APIs without a Windows implementation |
| Browser | Renderer layout and interaction development only; native capabilities require the desktop host |
| Containers, remote servers, mobile | No supported Aone desktop execution workflow is provided |

The local environment inspected on 2026-09-07 was macOS 26.5.2 on ARM64 with
Node.js 24.18.0, npm 12.0.1, and Rust/Cargo 1.97.0. That inventory is not evidence
of a clean-machine installation, a minimum-OS test, or a new packaged release.

Specific portability work remains in `src-tauri/src/git/inspection.rs` (Unix
filesystem identity), `src-tauri/src/git/process.rs` (fixed `/usr/bin/git`),
`src-tauri/src/tool_config/commands.rs` (fixed `/usr/bin/open`), and
`src-tauri/tauri.conf.json` (DMG packaging and macOS window settings). These paths
also enforce security boundaries; a future port must preserve containment,
identity revalidation, approved executable selection, and consent.

## Toolchain and dependencies

- `.nvmrc` and `.node-version` select Node.js **24.18.0** for compatible version
  managers. `package.json` declares the supported Node 24 and npm 12 ranges and
  records npm **12.0.1** as the package manager.
- `rust-toolchain.toml` selects Rust **1.97.0** with `rustfmt` and `clippy`.
  Rustup may download that named toolchain when it is not already installed.
  `src-tauri/Cargo.toml` also declares Rust 1.97 as the minimum compiler.
- `package-lock.json` is the canonical JavaScript dependency lock. Use npm for
  this repository. The retained `pnpm-lock.yaml` is not maintained by the
  documented npm workflow; do not treat a pnpm install as the same dependency
  or validation result.
- `src-tauri/Cargo.lock` is the canonical native dependency lock. Native test
  and Clippy scripts use `--locked` so a check cannot silently rewrite it.
- Native builds require Apple's command-line development tools. See the
  [official Tauri prerequisites](https://v2.tauri.app/start/prerequisites/#macos).
  A signing identity and notarization are separate distribution requirements.
- Docker is needed only when regenerating the IPC contract or running its
  Swagger UI documentation. The ordinary contract drift checks read local
  files and do not start Docker.

Use organization-approved installation methods on managed computers. These
files describe the development environment; they do not change ownership or
authorize software installation or publication.

## Run from a clean source copy

Use a source copy whose publication and transfer have already been authorized.
From its root, select the recorded toolchain and verify it:

```bash
node --version
npm --version
rustc --version
cargo --version
xcode-select -p
```

If a command is missing or reports another version, install or select the
required tool through your approved tool manager before continuing. Do not
weaken the dependency or compiler constraints to make a check pass.

```bash
npm ci
npm run check
npm run desktop
```

`npm ci` installs the recorded dependency graph and replaces any existing
`node_modules`; it fails when `package.json` and its npm lock disagree. See the
[npm ci reference](https://docs.npmjs.com/cli/v11/commands/npm-ci/).
The first dependency installation and Cargo build normally require registry
network access. Locked inputs improve repeatability, but do not promise an
offline build or identical binary bytes across SDKs and operating systems.

The 2026-09-07 clean npm installation warned that an optional `fsevents` install
script was blocked by npm's install-script policy. Build and test results must
be checked separately; do not approve dependency scripts automatically merely
to remove a warning. Native development-server file watching still needs a
manual smoke test on the intended machine.

`npm run desktop` starts the Vite development server and the native Tauri app.
The development server expects port **1420** to be available. AI credentials
are optional: choose **Continue code-only** to open, scan, search, edit, and
explore a project. Running that project's code still needs its own toolchain
and dependencies; Aone does not bundle every language runtime.

For renderer-only work, use `npm run dev`. Use `npm run build` followed by
`npm run preview` to inspect the production frontend. Neither browser command
supplies filesystem access, a native terminal, or the other Rust-backed
capabilities.

## Checks during development

| Command | What it establishes |
| --- | --- |
| `npm run typecheck` | TypeScript project checks without building the Vite bundle |
| `npm run structure` | Source size, module documentation, and declaration-only Rust module indexes |
| `npm run openapi:check` | Generated IPC/source digest and command registry agreement |
| `npm run asyncapi:check` | Documented event/listener/emitter agreement |
| `npm run build` | TypeScript validation and production frontend compilation |
| `npm run size` | Bundle budget compliance for the existing `dist` output; run after build |
| `npm test` | Existing frontend behavior and model regressions |
| `npm run lint:rust` | Rust formatting and Clippy warnings, with locked dependencies |
| `npm run test:rust` | Existing native unit/integration regressions, with locked dependencies |
| `npm run test:site` | Version consistency, download template behavior, and release security regressions |
| `npm run check` | Aggregate source, contract, bundle, test, and lint checks |

The aggregate is currently a macOS development gate. Its release tests also
exercise Unix shell and filesystem behavior. Do not report it as a Windows or
Linux validation run. A failure should be recorded with the command and relevant
sanitized error, then reproduced and fixed before claiming the gate passed.

For a focused frontend change, the existing runner accepts a test path:

```bash
npm test -- src/components/GraphCanvas.test.tsx
```

Generated IPC files belong to the OpenAPI generator. When the contract changes,
edit its owning specification, use `npm run openapi:generate`, inspect the
generated diff, and rerun the contract checks. The pinned generator image and
workflow are documented in [Architecture](ARCHITECTURE.md).

## Establishing release and portability evidence

Before describing another platform or OS version as supported, implement its
native adapters, add its packaging configuration, run the checks on that actual
environment, and record a manual smoke test. The smoke test should exercise
workspace open/scan/search, graph navigation, edit/save conflict handling, Git
inspection, terminal start/close, and process cleanup using a disposable local
project. Test network and AI features separately when those are claimed.

A macOS release also needs a fresh package and checks of that package's actual
architecture, launch behavior, signature, and notarization. Existing release
documentation and old artifacts are historical until rebuilt and verified for
the current source. See [macOS distribution](MACOS_DISTRIBUTION.md) for the
separate controlled release procedure.

Record the source revision, OS/architecture, compiler and package-manager
versions, commands, failures, and manual results for each supported environment.
A successful source check does not certify ownership, licensing, secret safety,
signing, or application behavior on machines that were not tested.
