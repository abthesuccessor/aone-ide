# Scanner feature

The scanner owns workspace containment, deterministic discovery, sensitive-path exclusion, secure
file reads, source analysis orchestration, and scan progress. `paths.rs` canonicalizes paths and
applies gitignore plus hard credential exclusions. `secure_read.rs` owns file-descriptor safety.
`scan.rs` discovers and analyzes candidates while enforcing aggregate budgets. `limits.rs` is the
single source of scan limits, `types.rs` contains indexed results, and `time.rs` formats timestamps.

## Budgets and data flow

- Individual index candidates are limited to 2 MiB; previews are limited to 4 MiB.
- A workspace scan allows at most 20,000 candidates, 256 MiB of source, 500,000 facts, and 120 seconds.
- Discovery retains no more than the file cap, sorts paths before analysis, and fails rather than
  silently truncating when a workspace budget is exceeded.
- The same deadline is passed into Tree-sitter extraction and checked between files.
- Each verified UTF-8 file is hashed, analyzed, and returned with metadata describing the opened bytes. Its source string remains only in the transient `IndexedFile` until the contentless search projection is transactionally updated.
- Scanner progress ends at `analyzed`; it does not claim that a workspace is usable. The command
  boundary owns `indexing` and the final `complete` event after transactional SQLite replacement
  and installation.

## Security

Git metadata, dependencies, build output, environment files, private-key formats, cloud/Kubernetes/
Docker credentials, Cargo credentials, gcloud data, and Terraform state are denied before reads.
Paths must stay beneath the canonical workspace and final symlinks are rejected. On Unix, files are
opened with `O_NOFOLLOW`; pre-open, canonical, descriptor, and post-open device/inode identities must
match. Identity, length, and modification metadata are checked again after the bounded read.

## Tests

`tests.rs` covers credential exclusions and ordinary-name allowances, gitignore behavior, traversal
and symlink escapes, descriptor identity swaps, byte/hash provenance, all aggregate budgets, the
deadline, deterministic path order, and persistence of repeated declarations without graph-ID
collisions.
