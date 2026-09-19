# Native core

The native core is split into feature folders. Each folder has a declaration and
re-export-only `mod.rs`, a local README, cohesive implementation files, and tests.
`lib.rs` is the composition root that installs managed state and registers the
small, reviewed Tauri command surface.

Rust owns filesystem, process, network, secret, scanner, and persistence
authority. Handwritten source files have a 500-line hard ceiling enforced by
`npm run structure`; `cargo fmt`, strict Clippy, and the full Rust suite enforce
the behavioral boundary.
