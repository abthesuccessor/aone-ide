# Codex CLI adapter

This module is the native-process boundary for the optional Codex CLI AI transport. It detects only
fixed native executable locations, rejects workspace-contained or script-based binaries, captures
file identity, and revalidates that identity immediately before every process launch.

Readiness runs only fixed `--version` and `login status` arguments with a cleared environment,
bounded output, and a six-second timeout. Inference sends the Rust-owned prompt through stdin to
`codex --ask-for-approval never exec` in a private temporary workspace. User config, rules, shell
tools, plugins, apps, browser/computer tools, workspace dependencies, and tool network permission
are disabled. The final answer must match a fixed JSON schema; stdout, stderr, prompt, answer size,
and execution time are bounded.

Execution is pinned to audited Codex CLI version `0.144.6`. A different version
is reported as unsupported until its command and feature surface is reviewed;
the finite feature denylist is never assumed to contain capabilities added by
an unknown future release.

Codex is the only executable AI CLI adapter in this build. Claude Code and GitHub Copilot are
reported as unsupported and are never executed. Full descendant termination uses Unix process
groups; non-Unix builds can terminate only the direct child, and detection is limited to the native
locations encoded in `cli_adapter.rs`.
