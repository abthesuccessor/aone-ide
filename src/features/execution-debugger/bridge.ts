import type {
  ControlDebugSession200Response,
  ControlDebugSessionRequestRequest,
  GetDebugSession200Response,
  StopRunRequestRequest,
} from "../../generated/ipc/models";
import { ControlDebugSessionRequestRequestActionEnum } from "../../generated/ipc/models";
import { isTauriRuntime } from "../../lib/bridge";
import type { DebuggerControlAction, DebugSessionView } from "./contracts";
import { adaptDebugSession } from "./adapter";

export interface DebugControlInput {
  runId: string;
  debugSessionId: string;
  action: DebuggerControlAction;
  expectedSequence: number;
  expectedControlEpoch: number;
}

export interface DebugControlOutcome {
  accepted: boolean;
  session: DebugSessionView;
}

export interface DebugSessionTransport {
  getSession: (runId: string) => Promise<DebugSessionView>;
  control: (input: DebugControlInput) => Promise<DebugControlOutcome>;
}

function nativeOnly(): never {
  throw new Error("Instrumented Debug requires the native desktop runtime and a managed child process");
}

async function invokeNative<T>(command: string, request: object): Promise<T> {
  if (!isTauriRuntime()) nativeOnly();
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, { request });
}

function generatedAction(action: DebuggerControlAction): ControlDebugSessionRequestRequest["action"] {
  switch (action) {
    case "pause": return ControlDebugSessionRequestRequestActionEnum.Pause;
    case "resume": return ControlDebugSessionRequestRequestActionEnum.Resume;
    case "stepInto": return ControlDebugSessionRequestRequestActionEnum.StepInto;
    case "stepOver": return ControlDebugSessionRequestRequestActionEnum.StepOver;
    case "stop": return ControlDebugSessionRequestRequestActionEnum.Stop;
  }
}

export async function getDebugSession(runId: string): Promise<DebugSessionView> {
  const request: StopRunRequestRequest = { runId };
  const snapshot = await invokeNative<GetDebugSession200Response>("get_debug_session", request);
  return adaptDebugSession(snapshot);
}

export async function controlDebugSession(input: DebugControlInput): Promise<DebugControlOutcome> {
  const request: ControlDebugSessionRequestRequest = {
    runId: input.runId,
    debugSessionId: input.debugSessionId,
    action: generatedAction(input.action),
    expectedSequence: input.expectedSequence,
    expectedControlEpoch: input.expectedControlEpoch,
  };
  const result = await invokeNative<ControlDebugSession200Response>("control_debug_session", request);
  return { accepted: result.accepted, session: adaptDebugSession(result.snapshot) };
}

export const nativeDebugSessionTransport: DebugSessionTransport = {
  getSession: getDebugSession,
  control: controlDebugSession,
};
