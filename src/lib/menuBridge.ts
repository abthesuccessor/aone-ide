import { isTauriRuntime } from "./bridge";

export type NativeMenuAction =
  | "newFile"
  | "openFolder"
  | "zoomIn"
  | "zoomOut"
  | "resetZoom";

export async function subscribeToMenuActions(
  handler: (action: NativeMenuAction) => void,
): Promise<() => void> {
  if (!isTauriRuntime()) return () => undefined;
  const { listen } = await import("@tauri-apps/api/event");
  return listen<{ action: NativeMenuAction }>("aone-menu-action", ({ payload }) => {
    if (
      payload.action === "newFile"
      || payload.action === "openFolder"
      || payload.action === "zoomIn"
      || payload.action === "zoomOut"
      || payload.action === "resetZoom"
    ) {
      handler(payload.action);
    }
  });
}

export function stopMenuSubscription(stop: () => void): void {
  try {
    // Tauri's runtime unlisten function can return a rejected promise even
    // though its public callback type is void. Strict Mode may dispose a
    // subscription before registration finishes, so absorb that stale stop.
    void Promise.resolve(stop()).catch(() => undefined);
  } catch {
    // A listener that is already gone is equivalent to a completed cleanup.
  }
}
