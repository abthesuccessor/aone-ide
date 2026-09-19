import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useCallback, useEffect, useState } from "react";
import { isTauriRuntime } from "../lib/bridge";
import { stopMenuSubscription, subscribeToMenuActions, type NativeMenuAction } from "../lib/menuBridge";

const MIN_ZOOM = 0.6;
const MAX_ZOOM = 1.6;
const ZOOM_STEP = 0.1;
// Reset legacy density experiments once. The v3 baseline is the actual-size
// VS Code workbench scale; user zoom choices made from here remain persistent.
const STORAGE_KEY = "aone.applicationZoom.v3";

type ZoomAction = "zoomIn" | "zoomOut" | "resetZoom";

function clamp(value: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(value * 10) / 10));
}

function initialZoom(): number {
  try {
    const stored = Number(window.localStorage.getItem(STORAGE_KEY));
    return Number.isFinite(stored) && stored >= MIN_ZOOM && stored <= MAX_ZOOM ? stored : 1;
  } catch {
    return 1;
  }
}

export function useApplicationZoom(): number {
  const [scale, setScale] = useState(initialZoom);
  const applyZoom = useCallback((action: ZoomAction) => {
    setScale((current) => {
      if (action === "zoomIn") return clamp(current + ZOOM_STEP);
      if (action === "zoomOut") return clamp(current - ZOOM_STEP);
      return 1;
    });
  }, []);

  useEffect(() => {
    const handleZoomShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      const zoomIn = event.key === "+" || event.key === "=" || event.code === "NumpadAdd";
      const zoomOut = event.key === "-" || event.code === "NumpadSubtract";
      const reset = event.key === "0" || event.code === "Numpad0";
      if (!zoomIn && !zoomOut && !reset) return;
      event.preventDefault();
      event.stopPropagation();
      applyZoom(zoomIn ? "zoomIn" : zoomOut ? "zoomOut" : "resetZoom");
    };
    window.addEventListener("keydown", handleZoomShortcut, { capture: true });
    return () => window.removeEventListener("keydown", handleZoomShortcut, { capture: true });
  }, [applyZoom]);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: () => void = () => undefined;
    void subscribeToMenuActions((action: NativeMenuAction) => {
      if (mounted && (action === "zoomIn" || action === "zoomOut" || action === "resetZoom")) {
        applyZoom(action);
      }
    }).then((stop) => {
      if (mounted) unsubscribe = stop;
      else stopMenuSubscription(stop);
    }).catch(() => undefined);
    return () => {
      mounted = false;
      stopMenuSubscription(unsubscribe);
    };
  }, [applyZoom]);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, String(scale));
    } catch {
      // Zoom still applies for this session when browser storage is unavailable.
    }
    document.documentElement.style.setProperty("--aone-application-zoom", String(scale));
    if (isTauriRuntime()) {
      document.body.style.zoom = "";
      void getCurrentWebview().setZoom(scale).catch(() => {
        document.body.style.zoom = String(scale);
      });
    } else {
      document.body.style.zoom = String(scale);
    }
  }, [scale]);

  return scale;
}
