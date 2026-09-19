import type { GraphSnapshot } from "../types";

export type MainView = "graph" | "source" | "debugger";
export type AppPhase = "loading" | "ready" | "error";
export type ActiveRun = { id: string; mode: "run" | "observe" | "debug"; stopping: boolean };

export const EMPTY_GRAPH: GraphSnapshot = {
  nodes: [],
  edges: [],
  truncated: false,
};
