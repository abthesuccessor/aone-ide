import type { MutableRefObject } from "react";
import { failureMessage } from "../../lib/errorMessage";
import type { WorkspaceSummary } from "../../types";

type MutationResult = { workspace: WorkspaceSummary };
export type MutationOutcome<T extends MutationResult> =
  | { status: "success"; result: T }
  | { status: "cancelled" | "stale" }
  | { status: "error"; message: string };

function isWorkspaceCancellation(message: string): boolean {
  return /^(?:invalid request:\s*)?(?:github clone was cancelled|project creation was cancelled)$/i.test(message.trim());
}

interface WorkspaceMutationContext {
  phase: "loading" | "ready" | "error";
  initializingRef: MutableRefObject<boolean>;
  openingRef: MutableRefObject<boolean>;
  operationRef: MutableRefObject<boolean>;
  generationRef: MutableRefObject<number>;
  setOpening: (opening: boolean) => void;
  clearProgress: () => void;
  adopt: (result: MutationResult) => Promise<void>;
  notify: (message: string, tone: "success" | "error") => void;
}

export async function runWorkspaceMutation<T extends MutationResult>(
  context: WorkspaceMutationContext,
  request: () => Promise<T | null>,
  fallback: string,
): Promise<MutationOutcome<T>> {
  if (context.phase !== "ready" || context.initializingRef.current
    || context.openingRef.current || context.operationRef.current) {
    return { status: "cancelled" };
  }
  const generation = context.generationRef.current + 1;
  context.generationRef.current = generation;
  context.openingRef.current = true;
  context.operationRef.current = true;
  context.setOpening(true);
  try {
    const result = await request();
    if (!result) return { status: "cancelled" };
    if (generation !== context.generationRef.current) return { status: "stale" };
    await context.adopt(result);
    return generation === context.generationRef.current
      ? { status: "success", result }
      : { status: "stale" };
  } catch (failure) {
    const message = failureMessage(failure, fallback);
    if (isWorkspaceCancellation(message)) return { status: "cancelled" };
    if (generation === context.generationRef.current) context.notify(message, "error");
    return { status: "error", message };
  } finally {
    if (generation === context.generationRef.current) {
      context.openingRef.current = false;
      context.operationRef.current = false;
      context.setOpening(false);
      context.clearProgress();
    }
  }
}
