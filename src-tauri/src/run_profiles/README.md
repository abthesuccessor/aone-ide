# Run profile discovery

This feature discovers safe, structured local run candidates from recognized
workspace manifests. It does not execute commands and it never evaluates a
manifest script body.

## Responsibilities

- `discovery.rs` performs a bounded, ignore-aware manifest walk and maps known
  ecosystem files to argument-vector profiles.
- `package_json.rs` reads bounded `package.json` files and turns script names
  into package-manager argument vectors.
- `profile.rs` creates stable profile identities and portable relative paths.
- `tests.rs` covers structured Node profiles and graceful handling of malformed
  or oversized package manifests.

`mod.rs` contains declarations and the `detect_profiles` re-export only.

## Invariants

1. Discovery follows neither symlinks nor hard-denied credential/build paths.
2. The walk is capped at depth 8 and 100 manifests; output is capped at 100
   unique profiles.
3. Package manifests are capped at 1 MiB and at 30 script-name profiles.
4. Profiles contain an executable plus an argument vector. Script bodies are
   never copied into a shell command.
5. Malformed, unreadable, oversized, or concurrently removed package manifests
   are skipped without hiding valid profiles from other services.

## Data flow

Workspace root -> bounded manifest walk -> lexically sorted manifest paths ->
ecosystem-specific mapping -> stable ID deduplication -> `Vec<RunProfile>`.

The runtime runner later validates a selected profile again and launches it
without a shell when the user explicitly selects Run or Debug. The renderer
cannot replace the registered executable or argument vector.

## Tests

Run the focused feature suite with:

```bash
cargo test run_profiles::tests
```
