# AONE IDE open-source readiness

Review date: 2026-09-07. This is a local source review, not a public release,
legal opinion, complete security audit, or assurance of zero defects.

## Decision

AONE's original code can be offered under MIT based on the owner's statement
that it is theirs to license. The local tree now contains the standard
[MIT license](../LICENSE), npm/Cargo MIT metadata, and contribution guidance.
Publication still needs the specific unresolved checks below. License choice,
code quality, GitHub access, and permission to use a company computer are
different questions.

MIT permits commercial reuse, modification, and redistribution, including in
closed-source products, subject to retaining the required notices. It does not
certify quality or grant rights in someone else's material. See the
[standard MIT terms](https://choosealicense.com/licenses/mit/).

## Ownership and the AONE name

Amra stated during this review that they own AONE IDE, have not incorporated
AONE, and want AONE-only attribution. `Copyright (c) 2026 AONE` uses that chosen
project attribution. It does not assert that a legal company exists. The
identified current owner remains Amra; the notice itself does not transfer
ownership to a future company. Using a person's full legal name with the
project name would identify the owner more clearly if desired later.

Before publication, keep an accurate ownership/contribution record and check
that all included source, artwork, and imported material may be shared. A later
company can receive rights through an appropriate assignment; changing the
branding or license header alone does not establish an assignment. Company-name
availability and trademark clearance were not investigated in this review.

This review relies on the owner's statement. It has not examined employment
agreements, outside contributors' agreements, or the device owner's policies.
GitHub's [legal guide](https://opensource.guide/legal/#what-does-my-companys-legal-team-need-to-know)
explains why employer approval can matter even for a personal project.

## What changed locally

| Change | Purpose |
| --- | --- |
| Root LICENSE and npm/Cargo license metadata | State the owner's selected license for original work |
| Third-party review, dependency inventory, and shadcn notice | Preserve verified attribution and make unresolved provenance explicit |
| CONTRIBUTING.md and DEVELOPMENT.md | Document boundaries, setup, checks, and platform support |
| Node/npm declarations and Rust toolchain pin | Make the tested development environment reproducible |
| Locked Cargo check scripts and a TypeScript check command | Keep validation from silently changing dependency resolution |
| Structure-check path normalization | Keep module checks/exclusions working with Windows path separators |
| Editor save/format error handling | Preserve bounded native conflict/error instructions and unsaved content |
| Canonical npm install and measured bundle baseline | Replace a stale mixed dependency tree and calibrate only the total-JS budget to repeatable locked builds |

The existing code has useful foundations: feature-local modules, a 500-line
structure gate, module documentation, generated IPC contracts, native authority
boundaries, bounded evidence queries, and substantial tests. Keep improving
these incrementally. A wholesale rewrite would need a concrete architectural
reason and would not establish correctness.

## Required publication follow-up

| Item | Current evidence and next action |
| --- | --- |
| Original ownership | Owner states they own the code; independently resolve any employer/contributor rights that apply |
| Device permission | Company-managed computer was reported; its personal-login, software, transfer, and publishing policies were not provided |
| Third-party source | Verified shadcn notice added; finish theme/artwork/generated-output provenance in [third-party review](THIRD_PARTY_REVIEW.md) |
| Dependencies | Versioned license metadata is reviewed, but exact redistributed notices and MPL obligations still need fulfillment |
| Secrets and private data | Gitleaks scan found one reviewed false positive (a localStorage preference key), no confirmed credential; repeat against the exact future staged set and inspect image contents |
| Security reporting | Choose a real private reporting contact before inviting public reports; SECURITY.md currently points to the owner without inventing an address |
| Portability | macOS is the current target; Windows source has Unix-only blockers and Linux/Intel/minimum-OS support needs actual implementation/testing |
| Binary release | Existing download manifest remains unsigned, unnotarized, and `publicReady: false`; do not advertise it as a trusted public build |

The MPL components do not automatically prevent original AONE source from
using MIT. They keep their own license, and distribution obligations depend on
the material actually shipped. The versioned review distinguishes normal,
development, and build dependencies; it is not a final binary bill of materials.
See [Mozilla's MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

## Can a personal GitHub account be used on this computer?

Technically GitHub supports personal accounts and browser account switching.
An anonymous HTTPS request to `https://github.com` from this computer returned
HTTP 200 during this review. That establishes website reachability only.
No login, credential inspection, SSH authentication, repository creation, remote
configuration, or push was performed.

A website login does not authenticate terminal Git. Git separately uses HTTPS
credentials or SSH, and a commit's author name/email does not select the account
that authenticates a push. Follow GitHub's
[authentication guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/about-authentication-to-github)
and [account-switching guidance](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/switching-between-accounts).

First verify that the device owner's policy permits personal GitHub use and
publication/transfer of this project. A private personal repository still
transfers the code outside the computer. Do not assume it is approved merely
because it is private. If personal use is allowed, select the intended personal
account explicitly and use the approved credential manager or SSH setup; never
put a token in a remote URL or reuse company credentials for a personal project.
If it is disallowed, follow the approved transfer process to a permitted device.

## First version without custom branding

The owner reported that they have no logo/theme source records and asked whether
branding can be deferred. Yes: a first source release does not need a marketing
screenshot or a custom logo. The checkout nevertheless already contains
`assets/aone-icon-source.png`, `src-tauri/icons/**`, `site/assets/aone-icon.png`,
and `site/assets/aone-workbench.png`. Their presence is established; their origin
is not. They were not deleted during this review.

Use this concrete cleanup order before publication:

1. Remove the optional website screenshot/favicon and their references in
   `site/index.html`; update the corresponding image checks in
   `scripts/test-download-site.mjs`. Keep the release/security assertions.
2. Replace required native icon inputs with neutral icons of documented origin,
   then remove unused unknown variants. Tauri's current Unix context generation
   reads `src-tauri/icons/icon.png`, including macOS development builds; an empty
   icon list does not remove that requirement. Windows builds need an ICO input
   too. Deleting the directory blindly would break native compilation.
3. Keep the functional Phosphor UI glyphs with their upstream MIT notice. These
   are ordinary licensed dependencies, separate from AONE's custom branding.
4. For editor presets without source records, either freshly author neutral
   AONE palettes or deliberately import licensed upstream palettes at recorded
   revisions with their notices. Renaming the existing presets alone does not
   establish their provenance. Update saved-theme fallbacks and affected tests.
5. Rerun frontend/site checks and native compilation. Defer the public installer
   until packaging, notices, signing, and native smoke tests are complete.

This sequence is recorded as remaining work; the current tree must not be
described as having completed branding removal or cleared image/theme rights.

## Future first-publication procedure

These are instructions for a later authorized action. None of the write or
publication steps were executed in this review, and no `.github` file was
created or changed.

1. Finish ownership, device-policy, attribution, and private-data review. Keep
   source publication separate from distributing a signed application.
2. Rehearse the documented setup on a clean supported machine and record
   `npm run check` plus native smoke-test results for the exact source.
3. Run an approved dedicated secret scanner over the candidate working tree;
   if history later exists, scan that history too. Do not paste findings that
   contain credentials into public issues. Rotate any real leaked credential.
4. Review the exact files to be committed. Exclude credentials, `.env` values,
   private datasets, local indexes, dependency/build directories, binaries, and
   screenshots of private work. `.gitignore` is a convenience, not proof.
5. Set repository-local author identity deliberately, review what email becomes
   public, and make the first local commit only when ready. This checkout had
   zero tracked files, an unborn `master`, and no remote when inspected; a
   blank ordinary Git diff therefore did not mean there was nothing to review.
6. Under the intended personal account, create an empty repository. Avoid
   initializing a second README, license, or ignore file. A private staging
   repository is optional only after external transfer is permitted.
7. Configure and verify the destination and authentication for that repository,
   then separately authorize the first push. The source license is independent
   of repository visibility and of npm's `private: true` flag.
8. Review the uploaded tree before making it public. Public forks/copies may
   persist even if visibility is changed back later. Publish binary assets only
   after their separate signing, notarization, notice, and native checks pass.

Read-only commands useful before the future staging review:

```bash
git status --short
git ls-files --cached --others --exclude-standard
git diff --cached --stat
git diff --cached --check
```

GitHub documents the later import workflow in
[Adding locally hosted code](https://docs.github.com/en/migrations/importing-source-code/using-the-command-line-to-import-source-code/adding-locally-hosted-code-to-github)
and the difference between visibility and licensing in
[Licensing a repository](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/licensing-a-repository).

## Validation record

The final local validation record is maintained below. Passing source checks
does not prove clean-machine setup, live native behavior, complete vulnerability
coverage, ownership, or other operating systems.

Final source gate: **passed** on macOS 26.5.2 / ARM64, Node 24.18.0, npm 12.0.1,
Rust/Cargo 1.97.0. The final command was `RUSTUP_TOOLCHAIN=stable npm run check`:
the installed `stable` alias was verified as 1.97.0, so validation did not need
to download another copy of the exact compiler selected by the new pin.

| Check | Result |
| --- | --- |
| Fresh temporary npm installation | Passed: 182 packages installed using the npm lock |
| Workspace `npm ci --no-audit --no-fund` | Passed; replaced the stale mixed installation |
| Selected actual dependency resolution | Monaco → DOMPurify 3.4.13; Vite → Rolldown 1.2.4 |
| Structure and contract gates | Passed: 778 checked files, 28 Rust module indexes, 56 IPC commands, 5 event channels |
| Production frontend and size gates | Passed; two isolated canonical builds produced identical measurements recorded in PERFORMANCE.md |
| Frontend tests | 384 passed in 76 files |
| Release/security tests | 24 passed; download-site and version checks also passed |
| Rust format / Clippy | Passed with warnings treated as errors |
| Rust tests | 394 passed; one existing manual persistence benchmark intentionally ignored |
| npm advisory audit | Zero known advisories returned for the npm dependency inventory at review time |
| Gitleaks 8.30.1 candidate scan | One reviewed false positive: `generic-api-key` on the public localStorage preference name at `src/app/useApplicationZoom.ts:11`; no confirmed credential |
| Documentation / license inventory | Local links, code fences, manifest/lock metadata and inventory hashes checked |

The Gitleaks executable came from its official release, with archive SHA-256
verified against release API metadata, and ran locally with redacted reporting
and no project-specific suppressions. No source was sent to a scanning service.
The initial dedicated scan covered 968 candidate files selected by Git's
cached/untracked/nonignored listing, without Git metadata or dependency/build
directories. Review of the preference-key finding established that it stores UI
zoom, not authentication material. See [Gitleaks usage](https://github.com/gitleaks/gitleaks#usage).
Run a fresh scan after further edits and against the final staged files; this
result does not cover ignored files, image contents, archives, or future history.

No current Rust advisory-database audit, clean-machine native installation,
manual desktop/file-watching smoke test, minimum-macOS/Intel/Windows/Linux run,
new DMG, signing, notarization, public account authentication, or publication was
performed. The build still reports large Monaco-related chunks and an
ineffective dynamic-import warning; the measured bundle gates pass. The optional
`fsevents` installation-script warning is documented in DEVELOPMENT.md.

This was a bounded architecture/risk review with automated checks, not a
line-by-line proof of the entire application. No Git staging, commit, branch or
remote change, push, or `.github` configuration was performed.
