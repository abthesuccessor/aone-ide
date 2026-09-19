# AI configuration hub

This panel presents a static backend-owned allowlist of MCP, CLI, agent, skill,
instruction, and rule locations. Static listing performs no filesystem or PATH
inspection. The user must press **Inspect AI tools** and approve a native dialog
before Rust checks bounded path metadata. Configuration contents, source text,
environment values, and credentials never cross IPC.

Opening or creating a minimal empty template requires native confirmation.
Existing files are never rewritten, symlinks are refused, new files use private
permissions, and macOS opens them with the fixed system text-editor command.
The renderer distinguishes `notInspected`, `found`, and `notFound`; workspace
changes reset results to the static unknown state and stale inspection/open
responses cannot update the new workspace.
