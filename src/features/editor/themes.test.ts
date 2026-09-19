import { describe, expect, it, vi } from "vitest";
import {
  applyWorkbenchTheme,
  DEFAULT_WORKBENCH_THEME,
  isWorkbenchTheme,
  monacoThemeName,
  registerMonacoThemes,
  themeDescriptor,
  WORKBENCH_THEME_OPTIONS,
} from "./themes";

describe("workbench theme registry", () => {
  it("uses VS Code Dark Modern as the backward-compatible first-run theme", () => {
    expect(DEFAULT_WORKBENCH_THEME).toBe("dark-modern");
    expect(themeDescriptor(DEFAULT_WORKBENCH_THEME)).toMatchObject({
      label: "VS Code Dark Modern",
      dark: true,
    });
    expect(WORKBENCH_THEME_OPTIONS.map((theme) => theme.id)).toEqual([
      "dark-modern",
      "light-modern",
      "high-contrast",
      "cursor-dark",
      "tokyo-night",
      "catppuccin-mocha",
    ]);
  });

  it("applies one descriptor to semantic workbench variables", () => {
    const root = document.createElement("div");
    applyWorkbenchTheme("tokyo-night", root);

    expect(root.dataset.theme).toBe("tokyo-night");
    expect(root.style.colorScheme).toBe("dark");
    expect(root.style.getPropertyValue("--workbench-editor")).toBe("#1a1b26");
    expect(root.style.getPropertyValue("--workbench-primary")).toBe("#7aa2f7");
  });

  it("registers every Monaco skin under the stable Aone name", () => {
    const defineTheme = vi.fn();
    registerMonacoThemes({ editor: { defineTheme } });

    expect(defineTheme).toHaveBeenCalledTimes(WORKBENCH_THEME_OPTIONS.length);
    expect(defineTheme).toHaveBeenCalledWith(
      monacoThemeName("cursor-dark"),
      themeDescriptor("cursor-dark").monaco,
    );
    expect(themeDescriptor("cursor-dark").terminal.background).toBe("#141414");
  });

  it("accepts only registry-backed persisted theme identifiers", () => {
    expect(isWorkbenchTheme("catppuccin-mocha")).toBe(true);
    expect(isWorkbenchTheme("neon-from-storage")).toBe(false);
  });
});
