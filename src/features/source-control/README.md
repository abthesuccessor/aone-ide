# Source control

This feature presents bounded local Git status, textual diffs, staging,
unstaging, repository initialization, and commits. It intentionally excludes
remote operations, destructive reset/discard actions, and credential handling.

The renderer sends workspace-relative paths only. Rust binds every operation to
the currently open canonical workspace, executes a fixed Git binary with argv
instead of a shell, disables external diff/textconv for previews, and asks for
native confirmation before mutations.

Selecting a changed file opens its source and diff, switches Relationships to
the Changes view, and immediately selects that file's synthetic Git-change
marker. This makes non-code files and files omitted from the bounded graph
visible too. Once the diff arrives, selection is refined to the smallest
indexed semantic node that intersects a changed new-file line. Modified source
facts and markers are green; insert, delete, and conflict markers are red.
