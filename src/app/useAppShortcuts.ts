import { useEffect, type Dispatch, type SetStateAction } from "react";
import type { GraphNode } from "../types";
import type { ActiveRun, MainView } from "./model";

interface AppShortcutOptions {
  activeRun: ActiveRun | null;
  selectedNode: GraphNode | null;
  handleStop: () => Promise<void>;
  openSourceForNode: (node: GraphNode) => void;
  setMainView: Dispatch<SetStateAction<MainView>>;
  setEnvOpen: Dispatch<SetStateAction<boolean>>;
}

export function useAppShortcuts({
  activeRun,
  selectedNode,
  handleStop,
  openSourceForNode,
  setMainView,
  setEnvOpen,
}: AppShortcutOptions) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const command = event.metaKey || event.ctrlKey;
      if (command && event.key.toLowerCase() === "k") {
        event.preventDefault();
        document.getElementById("workspace-search")?.focus();
      } else if (command && event.key === "1") {
        event.preventDefault();
        setMainView("graph");
      } else if (command && event.key === "2") {
        event.preventDefault();
        setMainView("source");
      } else if (command && event.key === "3") {
        event.preventDefault();
        setMainView("debugger");
      } else if (command && event.key === "Enter" && selectedNode) {
        event.preventDefault();
        openSourceForNode(selectedNode);
      } else if (event.key === "Escape") {
        setEnvOpen(false);
      } else if (event.shiftKey && event.key === "F5" && activeRun) {
        event.preventDefault();
        void handleStop();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeRun, handleStop, openSourceForNode, selectedNode, setEnvOpen, setMainView]);
}

export function useRunIntentShortcuts(
  activeRun: ActiveRun | null,
  onRunIntent: (mode: "run" | "debug") => void,
) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)
        || event.key.toLowerCase() !== "r"
        || activeRun) return;
      event.preventDefault();
      onRunIntent(event.shiftKey ? "debug" : "run");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeRun, onRunIntent]);
}
