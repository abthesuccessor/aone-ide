# Runner lifecycle state

This module isolates managed-process lifecycle transitions from the runtime
registry. A leader exit is recorded only after `Child::wait` returns an exit
status. A wait error instead leaves the process state unknown, terminalizes any
cooperative debug protocol as failed, and retains process-group authority until
the bounded TERM-to-KILL cleanup completes.

Signal delivery is not treated as exit proof. Normal stop cleanup releases the
global run slot after both observed leader exit and escalation completion; the
wait-failure recovery path releases it after escalation because no further
reaper result can be obtained.
