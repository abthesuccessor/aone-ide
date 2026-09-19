import type { RuntimeEvent } from "../types";

const TERMINAL_EVENT_KINDS = new Set(["process.exited", "process.wait_failed"]);
const MAX_REMEMBERED_RUNS = 200;

export function rememberTerminalRun(
  terminalRunIds: Set<string>,
  event: RuntimeEvent,
): string | null {
  if (!event.runId || !TERMINAL_EVENT_KINDS.has(event.kind)) return null;
  terminalRunIds.add(event.runId);
  while (terminalRunIds.size > MAX_REMEMBERED_RUNS) {
    const oldest = terminalRunIds.values().next().value;
    if (!oldest) break;
    terminalRunIds.delete(oldest);
  }
  return event.runId;
}

export function runAlreadyTerminated(terminalRunIds: ReadonlySet<string>, runId: string): boolean {
  return terminalRunIds.has(runId);
}
