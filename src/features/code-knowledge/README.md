# Indexed code knowledge navigator

The navigator lists the complete bounded API inventory and expands a selected
operation into the currently loaded execution-flow graph. It traverses useful
call/data relations to a depth and node cap, detects cycles, and keeps each
node's declared, resolved, inferred, or observed evidence label.

Selecting an API loads its backend-owned execution-flow query and opens its
source beside the D3 graph. Selecting a nested function, handler, DTO, DAO,
repository, model, or data node opens the exact indexed source range when one
exists. Missing links remain missing; the UI does not invent a call chain.
