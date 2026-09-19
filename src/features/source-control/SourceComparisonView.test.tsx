import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { SourceComparisonView } from "./SourceComparisonView";
import { DEFAULT_WORKBENCH_SETTINGS } from "../editor/settings";

vi.mock("@monaco-editor/react", () => ({
  DiffEditor: ({ original, modified }: { original: string; modified: string }) => (
    <div data-testid="diff-editor" data-original={original} data-modified={modified} />
  ),
}));

vi.mock("../editor/themes", () => ({
  monacoThemeName: () => "vs-dark",
  registerMonacoThemes: vi.fn(),
}));

vi.mock("../../components/SourceView", () => ({
  monacoLanguageId: () => "typescript",
}));

describe("SourceComparisonView", () => {
  it("shows previous and current text in the Monaco diff editor", () => {
    const onClose = vi.fn();
    render(<SourceComparisonView
      diff={{
        relativePath: "src/changed.ts",
        staged: false,
        content: "@@ patch",
        truncated: false,
        originalContent: "const value = 1;",
        modifiedContent: "const value = 2;",
        comparisonTruncated: false,
      }}
      language="TypeScript"
      settings={DEFAULT_WORKBENCH_SETTINGS}
      onClose={onClose}
    />);

    expect(screen.getByTestId("diff-editor")).toHaveAttribute("data-original", "const value = 1;");
    expect(screen.getByTestId("diff-editor")).toHaveAttribute("data-modified", "const value = 2;");
    fireEvent.click(screen.getByRole("button", { name: "Close diff" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("falls back to the bounded patch when a text side is unavailable", () => {
    render(<SourceComparisonView
      diff={{
        relativePath: "assets/logo.bin",
        staged: true,
        content: "Binary files differ",
        truncated: false,
        comparisonTruncated: false,
      }}
      language="Text"
      settings={DEFAULT_WORKBENCH_SETTINGS}
      onClose={vi.fn()}
    />);
    expect(screen.getByText("Binary files differ")).toBeVisible();
  });
});
