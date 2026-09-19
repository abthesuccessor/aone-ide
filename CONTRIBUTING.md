# Contributing to AONE IDE

Start with [development setup](docs/DEVELOPMENT.md), then read the README in
the feature directory you plan to change and the [architecture](docs/ARCHITECTURE.md).
The project currently targets macOS. State the OS, architecture, tool versions,
and exact reproduction steps when reporting a defect.

## Keep changes understandable

- Keep one behavior change per patch. Explain the problem, resulting behavior,
  and how you verified it. Preserve useful comments about constraints.
- React function components own presentation; Rust owns filesystem, process,
  network, workspace identity, consent, and persistence. Do not move permission
  checks into the renderer or infer approval from an old cached result.
- Keep feature logic in its owning module. The structure check enforces a
  500-line maximum, module READMEs, and declaration-only Rust module indexes.
  Split by responsibility instead of adding indirection solely to meet a limit.
- Preserve evidence provenance and distinguish unknown, inferred, and observed
  facts. Never add invented runtime data to make a feature look complete.
- Edit contracts at their source and regenerate clients. Do not hand-edit
  `src/generated/ipc`; see [IPC contract](docs/IPC_CONTRACT.md).
- Use the existing bounded `failureMessage` helper for renderer-facing native
  IPC rejections. Preserve actionable errors without logging source or secrets.
- Use npm and the committed npm lockfile. Dependency upgrades need a reason,
  updated license review, and validation; avoid unrelated bulk upgrades.

## Verify behavior

Run the narrow existing checks while editing, then `npm run check` before a
release candidate. For a bug fix, add a regression when it proves the failure
and protects a meaningful behavior; documentation and trivial reversible edits
do not require new tests. Describe any checks that could not run.

Exercise changed native behavior manually on a disposable workspace. Test
cancellation, permission denial, stale workspace responses, and failure paths
when they are affected. A passing mocked renderer test does not prove a native
dialog, network request, packaged app, or Windows/Linux support works.

## Rights and reports

Contribute only material you have the right to share under the project's MIT
license. Preserve upstream copyright/license notices, document copied or adapted
material, and verify any employer approval that applies. Do not include private
company code, customer data, credentials, local configuration, or screenshots
containing them. See [third-party review](docs/THIRD_PARTY_REVIEW.md).

For vulnerabilities follow [SECURITY.md](SECURITY.md); use a private reporting
channel. Public issues are suitable for ordinary bugs only after the repository
is published. No public repository or contact address is assumed by this guide.
