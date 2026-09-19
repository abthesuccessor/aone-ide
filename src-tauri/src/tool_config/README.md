# AI tool and project-guidance configuration module

This module exposes metadata for a backend-owned allowlist of AI CLI, MCP,
agent, skill, instruction, and rule files. It never reads configuration
contents into the renderer and never accepts a renderer-provided path,
template, executable, or command.

- `catalog.rs` defines eleven fixed artifacts, path/CLI detection hints, and
  credential-free minimal templates.
- `commands.rs` returns a pure static `notInspected` catalog, performs one
  explicitly consented metadata inspection at a time, and opens one selected
  artifact through fixed macOS `/usr/bin/open -t` after separate consent.
- `files.rs` rejects symlinks, creates missing parent directories safely, and installs a new `0600` file atomically without overwriting an existing file.
- `confirmation.rs` explains inspection scope without inspecting first, then
  separately explains owner, safe path hint, creation behavior, and the
  potentially sensitive nature of a selected file before opening it.

The allowlist follows primary documentation available on 2026-08-17:

- OpenAI Codex: <https://developers.openai.com/codex/mcp/>
- Claude Desktop / MCP: <https://modelcontextprotocol.io/quickstart/user>
- Cursor: <https://docs.cursor.com/context/model-context-protocol>
- Gemini CLI: <https://geminicli.com/docs/tools/mcp-server/>
- Visual Studio Code: <https://code.visualstudio.com/docs/agent-customization/mcp-servers>

The workspace catalog additionally contains `AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, one Cursor rule, GitHub Copilot instructions, and a portable Agent
Skill. Static listing touches no home, PATH, configuration, or CLI path.
Inspection begins only after a native warning and performs bounded metadata
checks without reading files, executing tools, invoking a shell, or recursing
through the Mac.

Existing files are never modified. A missing artifact is created only from its
compiled template when `createIfMissing: true` is explicitly requested and
natively approved. Templates contain no scripts, executable commands,
credentials, or secret placeholders.
