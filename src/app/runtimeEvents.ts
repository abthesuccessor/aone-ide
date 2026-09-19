import type { ApiRequest, ApiResponse, RunProfile, RuntimeEvent } from "../types";
import type { ActiveRun } from "./model";

export function createBrowserRunEvent(
  runId: string,
  profile: RunProfile,
  mode: ActiveRun["mode"],
): RuntimeEvent {
  return {
    id: `start-${runId}`,
    runId,
    timestamp: new Date().toISOString(),
    kind: "process.started",
    label:
      mode === "debug"
        ? `${profile.name} started with cooperative debug enabled`
        : mode === "observe"
        ? `${profile.name} started with observation`
        : `${profile.name} started locally`,
    evidence: "observed",
    metadata: {
      executable: profile.executable,
      args: profile.args,
      observationMode: mode !== "run",
      debugMode: mode === "debug",
    },
  };
}

export function createBrowserHttpEvent(
  request: ApiRequest,
  response: ApiResponse,
): RuntimeEvent {
  const path = new URL(request.url).pathname;
  return {
    id: `event:${response.requestId}`,
    kind: "http.completed",
    timestamp: new Date().toISOString(),
    label: `${request.method} ${path} → ${response.status}`,
    evidence: "observed",
    traceId: response.headers.find((header) => header.name.toLowerCase() === "x-trace-id")?.value,
    metadata: {
      durationMs: response.durationMs,
      method: request.method,
      path,
      requestId: response.requestId,
      status: response.status,
    },
  };
}
