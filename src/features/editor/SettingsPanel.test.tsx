import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DEFAULT_WORKBENCH_SETTINGS } from "./settings";
import { SettingsPanel } from "./SettingsPanel";

describe("SettingsPanel", () => {
  it("offers the familiar default and only the curated theme registry", () => {
    render(
      <SettingsPanel
        open
        settings={DEFAULT_WORKBENCH_SETTINGS}
        onChange={vi.fn()}
        onReset={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    const theme = screen.getByLabelText("Color theme");
    expect(theme).toHaveValue("dark-modern");
    expect(screen.getByRole("option", { name: "VS Code Dark Modern" })).toBeVisible();
    expect(screen.getByRole("option", { name: "Cursor Dark" })).toBeVisible();
    expect(screen.getAllByRole("option")).toHaveLength(9);
    expect(screen.getByText(/familiar VS Code and Cursor-style dark workbench/i)).toBeVisible();
  });

  it("moves focus into the modal, closes on Escape, and restores invoking focus", () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <>
        <button type="button">Open settings</button>
        <SettingsPanel
          open={false}
          settings={DEFAULT_WORKBENCH_SETTINGS}
          onChange={vi.fn()}
          onReset={vi.fn()}
          onClose={onClose}
        />
      </>,
    );
    const opener = screen.getByRole("button", { name: "Open settings" });
    opener.focus();
    rerender(
      <>
        <button type="button">Open settings</button>
        <SettingsPanel
          open
          settings={DEFAULT_WORKBENCH_SETTINGS}
          onChange={vi.fn()}
          onReset={vi.fn()}
          onClose={onClose}
        />
      </>,
    );

    expect(screen.getByRole("button", { name: "Close settings" })).toHaveFocus();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(
      <>
        <button type="button">Open settings</button>
        <SettingsPanel
          open={false}
          settings={DEFAULT_WORKBENCH_SETTINGS}
          onChange={vi.fn()}
          onReset={vi.fn()}
          onClose={onClose}
        />
      </>,
    );
    expect(opener).toHaveFocus();
  });

  it("contains Tab focus and blocks workbench command shortcuts while open", () => {
    const shortcut = vi.fn();
    window.addEventListener("keydown", shortcut);
    render(
      <SettingsPanel
        open
        settings={DEFAULT_WORKBENCH_SETTINGS}
        onChange={vi.fn()}
        onReset={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const first = screen.getByRole("button", { name: "Close settings" });
    const last = screen.getByRole("button", { name: "Done" });

    last.focus();
    fireEvent.keyDown(last, { key: "Tab" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(last).toHaveFocus();

    shortcut.mockClear();
    for (const key of ["k", "r", "1", "2"]) fireEvent.keyDown(first, { key, metaKey: true });
    fireEvent.keyDown(first, { key: "Enter", metaKey: true });
    fireEvent.keyDown(first, { key: "f", metaKey: true, shiftKey: true });
    fireEvent.keyDown(first, { key: "F5", shiftKey: true });
    expect(shortcut).not.toHaveBeenCalled();
    window.removeEventListener("keydown", shortcut);
  });
});
