# Workspace search

This feature is the VS Code-style, keyboard-first search surface for the
currently approved workspace. `Shift+Command+F` on macOS and
`Shift+Control+F` elsewhere select it and focus its bounded query input.

The renderer waits 170 ms, then sends the opaque workspace ID, a control-free
query of at most 256 characters, and a 500-result cap. Rust owns the contentless
SQLite FTS5 indexes, validates the active workspace, searches only safe indexed
files, and returns 1-based/end-exclusive source ranges. Results cover path and
content hits plus static AST symbols, API endpoints, event subscriptions, and
event publications; runtime log history is intentionally outside this index.

Requests bind to both a request generation and workspace ID/generation, so an
older response cannot appear after a newer query or folder switch. Results keep
backend relevance order, group by relative path, expose truncation, and reuse
the existing Monaco source-open path rather than introducing a second editor.

The result tree has one roving tab stop. Arrow, Home, and End keys move through
visible rows; Right expands or enters a file group, Left collapses or returns to
its parent, and Enter activates the focused button.
