# Workspace search

This feature provides bounded, VS Code-style search over the currently selected
workspace. SQLite FTS5 trigram indexes produce fast candidate files and AST
facts. The source index is contentless: exact lines and snippets are reread
through the scanner's canonical-path, no-symlink security boundary and accepted
only when their hash still matches the committed workspace index.
Returned ranges use 1-based lines and Monaco-compatible UTF-16 code-unit
columns with exclusive end positions, including matches containing astral text.

Literal matching follows code-search conventions: ASCII letters are
case-insensitive, while non-ASCII text is matched with exact case. Queries of
three or more characters use the trigram projection; one- and two-character
queries use a strictly bounded indexed-file fallback.
Semantic FTS candidates are exact-rechecked under those same rules before they
can become evidence, and endpoint/event/symbol matches are selected before
lower-ranked path and content matches even for a one-result request.

The request/result ceilings are 256 query characters, 500 returned matches,
50 exact occurrences per file, 64 MiB and 2,000 attempted source reads, and 750 ms.
Candidate ceilings are 2,000 source documents, 200 paths, and 1,000 semantic
facts; the short-query source fallback examines at most 4,000 indexed files.
Results expose `truncated` when any candidate, time, byte, per-file, or result
budget is reached.
Every attempted secure read is charged before opening, grown files are bounded
by the remaining budget, and a newer request cooperatively cancels older work.

`commands.rs` validates the workspace identity and IPC bounds. `engine.rs`
applies time, byte, result, and per-file match budgets. `matching.rs` owns exact
locations and preview construction. `limits.rs` is the single budget catalog.

The index is a rebuildable projection. Workspace scans, editor saves, and file
watcher updates all modify files, AST facts, and FTS rows in one SQLite
transaction. Search never accepts an absolute path from the renderer.
