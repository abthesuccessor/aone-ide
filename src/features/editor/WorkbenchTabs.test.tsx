import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EditorDocument } from "./model";
import { WorkbenchTabs } from "./WorkbenchTabs";

const documents: EditorDocument[] = [
  {
    relativePath: "src/example.ts",
    language: "TypeScript",
    content: "export const example = true;",
    contentHash: "hash-1",
    savedContent: "export const example = false;",
    dirty: true,
  },
];

describe("WorkbenchTabs", () => {
  it("selects a document without nesting another interactive control", () => {
    const onDocument = vi.fn();
    const onClose = vi.fn();
    const { container } = render(
      <WorkbenchTabs
        documents={documents}
        activePath="src/example.ts"
        onDocument={onDocument}
        onPin={vi.fn()}
        onClose={() => {
          onClose();
          return true;
        }}
      />,
    );

    const tab = screen.getByRole("tab", { name: "example.ts, unsaved" });
    fireEvent.click(tab);
    expect(onDocument).toHaveBeenCalledWith("src/example.ts");
    expect(tab.querySelector('[role="button"]')).toBeNull();
    expect(container.querySelector("button button")).toBeNull();
  });

  it("closes from the named close button or the ARIA-authorized Delete shortcut", () => {
    const onDocument = vi.fn();
    const onClose = vi.fn();
    render(
      <WorkbenchTabs
        documents={documents}
        activePath="src/example.ts"
        onDocument={onDocument}
        onPin={vi.fn()}
        onClose={() => {
          onClose();
          return true;
        }}
      />,
    );
    const tab = screen.getByRole("tab", { name: "example.ts, unsaved" });
    fireEvent.click(screen.getByRole("button", { name: "Close example.ts" }));
    fireEvent.keyDown(tab, { key: "Delete" });

    expect(onClose).toHaveBeenCalledTimes(2);
    expect(onDocument).not.toHaveBeenCalled();
    expect(tab).toHaveAttribute("aria-keyshortcuts", "Delete");
  });

  it("keeps one file tab in the Tab order and navigates only between files", () => {
    const onDocument = vi.fn();
    const openDocuments = [
      { ...documents[0]!, relativePath: "src/first.ts", dirty: false },
      { ...documents[0]!, relativePath: "src/second.ts", dirty: false },
    ];
    render(
      <WorkbenchTabs
        documents={openDocuments}
        activePath="src/second.ts"
        onDocument={onDocument}
        onPin={vi.fn()}
        onClose={() => true}
      />,
    );
    const first = screen.getByRole("tab", { name: "first.ts" });
    const second = screen.getByRole("tab", { name: "second.ts" });
    expect(second).toHaveAttribute("tabindex", "0");
    expect(first).toHaveAttribute("tabindex", "-1");
    expect(screen.queryByRole("tab", { name: "Graph" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Debug" })).not.toBeInTheDocument();

    second.focus();
    fireEvent.keyDown(second, { key: "Home" });
    expect(first).toHaveFocus();
    expect(onDocument).toHaveBeenCalledWith("src/first.ts");
    fireEvent.keyDown(first, { key: "End" });
    expect(second).toHaveFocus();
    expect(onDocument).toHaveBeenCalledWith("src/second.ts");
  });

  it("moves focus to an adjacent tab after deleting the focused document", async () => {
    function Harness() {
      const [openDocuments, setOpenDocuments] = useState<EditorDocument[]>([
        { ...documents[0]!, relativePath: "src/first.ts", dirty: false },
        { ...documents[0]!, relativePath: "src/second.ts", dirty: false },
      ]);
      const [activePath, setActivePath] = useState<string | null>(
        "src/second.ts",
      );
      return (
        <WorkbenchTabs
          documents={openDocuments}
          activePath={activePath}
          onDocument={setActivePath}
          onPin={vi.fn()}
          onClose={(path) => {
            setOpenDocuments((current) =>
              current.filter((document) => document.relativePath !== path),
            );
            setActivePath("src/first.ts");
            return true;
          }}
        />
      );
    }

    render(<Harness />);
    const second = screen.getByRole("tab", { name: "second.ts" });
    second.focus();
    fireEvent.keyDown(second, { key: "Delete" });

    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "first.ts" })).toHaveFocus(),
    );
  });

  it("preserves focus when a dirty close is cancelled", () => {
    render(
      <WorkbenchTabs
        documents={documents}
        activePath="src/example.ts"
        onDocument={vi.fn()}
        onPin={vi.fn()}
        onClose={() => false}
      />,
    );
    const document = screen.getByRole("tab", { name: "example.ts, unsaved" });
    document.focus();
    fireEvent.keyDown(document, { key: "Delete" });
    expect(document).toHaveFocus();
  });

  it("exposes an italic preview tab that can be kept open explicitly or by double-click", () => {
    const onDocument = vi.fn();
    const onPin = vi.fn();
    render(
      <WorkbenchTabs
        documents={[{ ...documents[0]!, dirty: false, preview: true }]}
        activePath="src/example.ts"
        onDocument={onDocument}
        onPin={onPin}
        onClose={() => true}
      />,
    );

    const preview = screen.getByRole("tab", { name: "example.ts, preview" });
    expect(preview.closest(".editor-tab-shell")).toHaveClass("is-preview");
    expect(preview.querySelector(".editor-tab-name")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Keep example.ts open" }));
    expect(onPin).toHaveBeenCalledWith("src/example.ts");

    fireEvent.doubleClick(preview);
    expect(onDocument).toHaveBeenCalledWith("src/example.ts");
    expect(onPin).toHaveBeenCalledTimes(2);
  });
});
