import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_WORKBENCH_SETTINGS,
  parseWorkbenchSettings,
  useWorkbenchSettings,
} from "./settings";

const SETTINGS_KEY = "aone.workbench.settings.v1";

beforeEach(() => {
  window.localStorage.clear();
  delete document.documentElement.dataset.theme;
});

afterEach(() => vi.restoreAllMocks());

describe("workbench settings", () => {
  it("accepts supported values and clamps numeric preferences", () => {
    const parsed = parseWorkbenchSettings({
      theme: "light-modern",
      fontFamily: "Fira Code",
      fontSize: 99,
      lineHeight: -2,
      tabSize: 4.7,
      insertSpaces: false,
      wordWrap: "bounded",
      wordWrapColumn: 12,
      minimap: false,
      formatOnSave: true,
      terminalFontFamily: "JetBrains Mono",
      terminalFontSize: 40,
      terminalCursorBlink: false,
    });

    expect(parsed).toEqual({
      theme: "light-modern",
      fontFamily: "Fira Code",
      fontSize: 24,
      lineHeight: 14,
      tabSize: 5,
      insertSpaces: false,
      wordWrap: "bounded",
      wordWrapColumn: 40,
      minimap: false,
      formatOnSave: true,
      terminalFontFamily: "JetBrains Mono",
      terminalFontSize: 22,
      terminalCursorBlink: false,
    });
  });

  it("falls back from malformed persisted data and caps font-family length", () => {
    expect(parseWorkbenchSettings({ theme: "neon", wordWrap: "sometimes" }).theme)
      .toBe(DEFAULT_WORKBENCH_SETTINGS.theme);
    expect(parseWorkbenchSettings({ fontFamily: "x".repeat(300) }).fontFamily).toHaveLength(240);

    window.localStorage.setItem(SETTINGS_KEY, "{not-json");
    const { result } = renderHook(() => useWorkbenchSettings());
    expect(result.current.settings).toEqual(DEFAULT_WORKBENCH_SETTINGS);
  });

  it("persists validated updates, applies the theme, and resets defaults", async () => {
    window.localStorage.setItem(SETTINGS_KEY, JSON.stringify({ theme: "high-contrast", fontSize: 18 }));
    const { result } = renderHook(() => useWorkbenchSettings());

    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("high-contrast"));
    expect(result.current.settings.fontSize).toBe(18);

    act(() => result.current.updateSettings({ fontSize: 500, wordWrapColumn: 120 }));
    expect(result.current.settings.fontSize).toBe(24);
    expect(JSON.parse(window.localStorage.getItem(SETTINGS_KEY) ?? "{}")).toMatchObject({
      fontSize: 24,
      wordWrapColumn: 120,
    });

    act(() => result.current.resetSettings());
    expect(result.current.settings).toEqual(DEFAULT_WORKBENCH_SETTINGS);
    expect(document.documentElement.dataset.theme).toBe("dark-modern");
  });

  it("keeps working when browser storage rejects writes", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("Storage unavailable", "QuotaExceededError");
    });
    const { result } = renderHook(() => useWorkbenchSettings());

    act(() => result.current.updateSettings({ theme: "light-modern" }));
    expect(result.current.settings.theme).toBe("light-modern");
    expect(document.documentElement.dataset.theme).toBe("light-modern");
  });
});
