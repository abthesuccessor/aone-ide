import { isTauriRuntime } from "../../lib/bridge";
import type {
  DeleteRuntimeTraceResult,
  OtlpReceiverSnapshot,
  StartOtlpReceiverRequest,
} from "../../types";

const stoppedOtlpReceiver: OtlpReceiverSnapshot = {
  status: "stopped",
  protocol: "http/json",
  authHeaderName: "x-aone-ingest-token",
  acceptedSpans: 0,
  rejectedSpans: 0,
  traceCount: 0,
  limitation: "The browser demo cannot open a local receiver. Use the signed desktop app.",
};

async function invokeNative<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export function getOtlpReceiver(): Promise<OtlpReceiverSnapshot> {
  if (!isTauriRuntime()) return Promise.resolve(stoppedOtlpReceiver);
  return invokeNative("get_otlp_receiver");
}

export function startOtlpReceiver(
  request: StartOtlpReceiverRequest = { preferredPort: 4318 },
): Promise<OtlpReceiverSnapshot> {
  if (!isTauriRuntime()) {
    return Promise.reject(new Error("The local OTLP receiver is available only in the desktop app"));
  }
  return invokeNative("start_otlp_receiver", { request });
}

export function stopOtlpReceiver(): Promise<OtlpReceiverSnapshot> {
  if (!isTauriRuntime()) return Promise.resolve(stoppedOtlpReceiver);
  return invokeNative("stop_otlp_receiver");
}

export function deleteRuntimeTrace(traceId: string): Promise<DeleteRuntimeTraceResult> {
  if (!isTauriRuntime()) {
    return Promise.reject(new Error("Runtime trace deletion is available only in the desktop app"));
  }
  return invokeNative("delete_runtime_trace", { request: { traceId } });
}
