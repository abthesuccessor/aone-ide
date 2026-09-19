import { useCallback, useEffect, useState } from "react";
import type { WorkbenchSettings } from "./model";
import { DEFAULT_WORKBENCH_THEME, isWorkbenchTheme } from "./themeIds";

const SETTINGS_KEY = "aone.workbench.settings.v1";

export const DEFAULT_WORKBENCH_SETTINGS: WorkbenchSettings = {
  theme: DEFAULT_WORKBENCH_THEME,
  fontFamily: "SFMono-Regular, Menlo, Monaco, Consolas, monospace",
  fontSize: 13,
  lineHeight: 20,
  tabSize: 2,
  insertSpaces: true,
  wordWrap: "off",
  wordWrapColumn: 100,
  minimap: true,
  formatOnSave: false,
  terminalFontFamily: "SFMono-Regular, Menlo, Monaco, Consolas, monospace",
  terminalFontSize: 12,
  terminalCursorBlink: true,
};

function boundedNumber(value: unknown, fallback: number, minimum: number, maximum: number) {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(minimum, Math.min(maximum, Math.round(value)))
    : fallback;
}

export function parseWorkbenchSettings(value: unknown): WorkbenchSettings {
  const source = value && typeof value === "object" ? value as Partial<WorkbenchSettings> : {};
  return {
    theme: isWorkbenchTheme(source.theme) ? source.theme : DEFAULT_WORKBENCH_SETTINGS.theme,
    fontFamily: typeof source.fontFamily === "string" && source.fontFamily.trim()
      ? source.fontFamily.slice(0, 240)
      : DEFAULT_WORKBENCH_SETTINGS.fontFamily,
    fontSize: boundedNumber(source.fontSize, 13, 10, 24),
    lineHeight: boundedNumber(source.lineHeight, 20, 14, 40),
    tabSize: boundedNumber(source.tabSize, 2, 1, 8),
    insertSpaces: typeof source.insertSpaces === "boolean" ? source.insertSpaces : true,
    wordWrap: ["off", "on", "bounded"].includes(source.wordWrap ?? "")
      ? source.wordWrap as WorkbenchSettings["wordWrap"]
      : "off",
    wordWrapColumn: boundedNumber(source.wordWrapColumn, 100, 40, 240),
    minimap: typeof source.minimap === "boolean" ? source.minimap : true,
    formatOnSave: typeof source.formatOnSave === "boolean" ? source.formatOnSave : false,
    terminalFontFamily: typeof source.terminalFontFamily === "string" && source.terminalFontFamily.trim()
      ? source.terminalFontFamily.slice(0, 240)
      : DEFAULT_WORKBENCH_SETTINGS.terminalFontFamily,
    terminalFontSize: boundedNumber(source.terminalFontSize, 12, 10, 22),
    terminalCursorBlink: typeof source.terminalCursorBlink === "boolean"
      ? source.terminalCursorBlink
      : true,
  };
}

function loadSettings(): WorkbenchSettings {
  try {
    return parseWorkbenchSettings(JSON.parse(window.localStorage.getItem(SETTINGS_KEY) ?? "null"));
  } catch {
    return DEFAULT_WORKBENCH_SETTINGS;
  }
}

export function useWorkbenchSettings() {
  const [settings, setSettings] = useState<WorkbenchSettings>(loadSettings);

  useEffect(() => {
    let current = true;
    document.documentElement.dataset.theme = settings.theme;
    void import("./themes").then(({ applyWorkbenchTheme }) => {
      if (current) applyWorkbenchTheme(settings.theme);
    });
    try {
      window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    } catch {
      // The workbench remains usable when storage is blocked or full.
    }
    return () => { current = false; };
  }, [settings]);

  const updateSettings = useCallback((patch: Partial<WorkbenchSettings>) => {
    setSettings((current) => parseWorkbenchSettings({ ...current, ...patch }));
  }, []);

  const resetSettings = useCallback(() => setSettings(DEFAULT_WORKBENCH_SETTINGS), []);
  return { settings, updateSettings, resetSettings };
}
