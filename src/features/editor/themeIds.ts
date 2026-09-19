export const WORKBENCH_THEME_IDS = [
  "dark-modern",
  "light-modern",
  "high-contrast",
  "cursor-dark",
  "tokyo-night",
  "catppuccin-mocha",
] as const;

export type WorkbenchTheme = (typeof WORKBENCH_THEME_IDS)[number];
export const DEFAULT_WORKBENCH_THEME: WorkbenchTheme = "dark-modern";

export function isWorkbenchTheme(value: unknown): value is WorkbenchTheme {
  return typeof value === "string" && WORKBENCH_THEME_IDS.includes(value as WorkbenchTheme);
}
