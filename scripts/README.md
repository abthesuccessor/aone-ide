# Build, release, and verification scripts

The scripts directory contains local build checks and the fail-closed macOS release workflow. These scripts do not publish by themselves; public publication still requires the separately controlled hosting step.

## Entry points

- `release-macos.sh` runs the reproducible local-test or Apple-authorized public release pipeline. It sanitizes inherited process state, verifies toolchain and signing inputs, runs project checks, builds a fresh DMG, and stages the download site.
- `verify-release.mjs` is the stable release-verification CLI and public module facade. CLI arguments and the exported `verifyRelease` and `verifyPublicAppleArtifact` functions remain compatible with existing callers.
- `build-download-site.mjs` creates an atomic local-test or proof-authorized public-candidate site under `release/site`.
- `check-release-version.mjs` ensures npm, Cargo, Tauri, and site-template versions agree.
- `check-bundle-size.mjs` enforces frontend bundle budgets.
- `test-download-site.mjs` checks the static site's accessibility, trust copy, same-origin download behavior, and honest template state.
- `check-structure.mjs` enforces the 500-line source/contract limit, declaration-only Rust `mod.rs` indexes, and a README for every checked module folder.

## Release library modules

- `lib/release-common.mjs` preserves the existing shared import surface and the `validate-origin` / `issue-proof` CLI.
- `lib/release-metadata.mjs` validates versions, targets, architecture facts, HTTPS origins, filenames, and named CLI arguments.
- `lib/confined-files.mjs` performs no-follow, direct-parent-confined file inspection, hashing, reading, and exclusive copying.
- `lib/public-proof.mjs` issues and atomically consumes short-lived HMAC release proofs bound to exact artifact and release metadata.
- `lib/apple-verification.mjs` invokes fixed-path Apple tools in a sanitized environment and binds notarization, signature, Team ID, bundle metadata, architecture, and minimum-macOS evidence.
- `lib/release-manifest.mjs` validates staged manifests and checksums, demotes unverified public claims, invokes Apple verification, detects artifact changes, and atomically promotes verified manifests.

## Tests and checks

Run focused release and structure validation from the repository root:

```bash
npm run test:release-security
npm run structure
```

Validate the complete site-facing release surface with:

```bash
npm run test:site
```

The aggregate `npm run check` starts with structure, OpenAPI, and AsyncAPI drift
validation before compiling or running the remaining suites.
