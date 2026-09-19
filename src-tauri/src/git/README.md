# Git module

This module exposes a deliberately local-only Git surface for the open workspace.

- `commands.rs` implements status, bounded patch and previous/current text comparisons, stage, unstage, commit, and initialization commands. Text comparison sides are capped at 2 MiB; binary and oversized content remains unavailable to Monaco. Initialization uses an empty template so user-configured template hooks are not copied.
- `process.rs` invokes the fixed macOS `/usr/bin/git` executable with argv only, bounded output, a 12-second timeout, no pager, no terminal prompts, hooks disabled, and remote operations absent from the source-control API. Initialization also disables global and system configuration.
- `capability.rs` reads the fixed Git's version once and caches it. Inspection's attribute isolation (`--attr-source`) needs Git 2.40, and Aone runs `/usr/bin/git`, which on current Xcode is 2.39.5. There is no older fallback that keeps the guarantee, so inspection is refused with a message naming the requirement rather than proceeding with repository-controlled diff, textconv, or filter drivers in play. For the same reason `GIT_NO_LAZY_FETCH` replaces `--no-lazy-fetch` (Git 2.45): Git rejects an unknown top-level option outright, breaking every operation, whereas an older Git simply ignores the variable.
- `inspection.rs` gives status and diff a private, short-lived Git metadata snapshot. It copies the bounded index, pins the original object database as read-only input, uses an empty attribute tree, and supplies no repository/global/system configuration. Consequently status and diff cannot start clean/smudge/process filters, textconv drivers, external diff commands, hooks, credential helpers, fsmonitor helpers, or lazy-fetch helpers configured by the repository or user.
- v0.1 intentionally accepts only an in-place `.git` directory with an in-place object database. `.git` files, `commondir`, symlinked object directories, and object alternates are rejected, so a workspace cannot redirect read-only inspection to an unrelated external repository. Open the primary repository root rather than a linked worktree.
- `status.rs` establishes exact-root or ancestor status with bounded filesystem
  metadata only. It never starts Git for repository discovery. Branch and HEAD
  are read through no-follow bounded metadata access, and status then enters the
  private snapshot before the first read-only Git process.
- `paths.rs` treats renderer paths as literal workspace-relative file paths, rejects sensitive paths and symlinks, and bounds every request.
- `confirmation.rs` requires native consent immediately before each mutation.
  Staging warns that repository clean filters can execute local code; hooks are
  disabled. Read-only status/diff never use those filters. Commit approval is bound to both the validated path list
  and a bounded hash of the raw staged index, so same-path content changes
  during the confirmation invalidate the operation.

The source-control feature intentionally has no push, pull, fetch, reset,
discard, checkout, delete, or arbitrary Git-command endpoints. The separate
onboarding feature supports only a native-approved, credential-free public
GitHub HTTPS clone into a new direct child of Documents; it also owns a distinct
consented sanitized metadata/public-key-fingerprint report without private-key,
SSH, network, or Git mutation authority.
