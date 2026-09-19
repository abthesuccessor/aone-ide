import type { AiCliAdapterStatus } from "../types";

/** Shown before the native backend has reported real adapter readiness. */
export const DEFAULT_CLI_ADAPTERS: AiCliAdapterStatus[] = [
  {
    id: "codex",
    label: "Codex CLI",
    installed: false,
    readiness: "not_installed",
    detail: "Run detection after Codex is installed and authenticated.",
  },
  {
    id: "claude",
    label: "Claude Code",
    installed: false,
    readiness: "not_installed",
    detail: "Run detection after Claude Code is installed and authenticated.",
  },
  {
    id: "copilot",
    label: "GitHub Copilot",
    installed: false,
    readiness: "not_supported",
    detail: "The Aone adapter is not available yet.",
  },
];

/** Maps backend readiness to the badge the dialog renders. */
export function cliState(adapter: AiCliAdapterStatus): { label: string; tone: "ready" | "muted" | "warning" } {
  switch (adapter.readiness) {
    case "ready":
      return { label: "Ready", tone: "ready" };
    case "authentication_required":
      return { label: "Sign in required", tone: "warning" };
    case "not_installed":
      return { label: "Not installed", tone: "muted" };
    case "error":
      return { label: "Error", tone: "warning" };
    default:
      return { label: "Unavailable", tone: "muted" };
  }
}
