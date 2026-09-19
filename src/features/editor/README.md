# Editor workbench

This feature owns the user-configurable Monaco editing experience, open-document
tabs, dirty state, save/format orchestration, and local non-secret preferences.
The Rust backend remains authoritative for workspace containment, optimistic
content hashes, formatter process approval, and file writes.

Settings use a versioned WebView local-storage key because they contain only UI
preferences. Source content, terminal output, environment values, and tool
configuration contents are never persisted here.

The first-run `VS Code Dark Modern` palette intentionally follows the familiar
density and contrast of VS Code/Cursor without claiming a byte-exact upstream
theme. `themes.ts` is the typed source of truth for workbench tokens, Monaco,
and xterm. Light Modern, High Contrast, Cursor Dark, Tokyo Night, and Catppuccin
Mocha are curated alternatives. Monaco remains the editor engine; this feature
does not implement an extension host.

Dirty documents synchronously gate every folder-opening entry point with one
bounded native browser confirmation. Save and format completions carry both the
workspace ID and workspace-open generation, so a late result cannot mutate an
identical relative path after a switch or same-folder reopen. Editor tabs use a
single roving tab stop with arrow, Home, End, and Delete keyboard behavior.
Single-click source navigation owns one clean italic preview tab. A later
single-click replaces only that preview; editing, formatting, double-clicking,
or using the named Keep open action pins it. Pinned and dirty documents are
never replaced by later graph, search, Git, or Explorer selection.
