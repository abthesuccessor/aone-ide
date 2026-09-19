import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SourceFile, SourceLocation } from "../types";
import { monacoLanguageId, SourceView } from "./SourceView";

const editor = vi.hoisted(() => ({
  deltaDecorations: vi.fn(() => ["trace-line"]),
  focus: vi.fn(),
  revealLineInCenter: vi.fn(),
  revealLineNearTop: vi.fn(),
  setPosition: vi.fn(),
  setSelection: vi.fn(),
}));
const monacoProps = vi.hoisted(() => ({
  onChange: undefined as undefined | ((value?: string) => void),
  options: undefined as undefined | { readOnly?: boolean },
}));

vi.mock("@monaco-editor/react", async () => {
  const React = await import("react");
  return {
    default: ({ language, onMount, onChange, options }: {
      language: string;
      onMount?: (instance: typeof editor) => void;
      onChange?: (value?: string) => void;
      options?: { readOnly?: boolean };
    }) => {
      monacoProps.onChange = onChange;
      monacoProps.options = options;
      React.useEffect(() => {
        onMount?.(editor);
      }, [onMount]);
      return React.createElement("div", { "data-testid": "editor", "data-language": language });
    },
  };
});

vi.mock("../lib/monaco", () => ({}));

const source: SourceFile = {
  relativePath: "src/main.ts",
  language: "TypeScript",
  content: "export const value = 1;",
  contentHash: "hash-1",
};

beforeEach(() => {
  for (const mock of Object.values(editor)) mock.mockClear();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  monacoProps.onChange = undefined;
  monacoProps.options = undefined;
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SourceView", () => {
  it("maps backend language labels to Monaco language IDs", () => {
    expect(monacoLanguageId("TypeScript")).toBe("typescript");
    expect(monacoLanguageId("C#")).toBe("csharp");
    expect(monacoLanguageId("C++")).toBe("cpp");
    expect(monacoLanguageId("Protocol Buffers")).toBe("protobuf");
    expect(monacoLanguageId("Text")).toBe("plaintext");
  });

  it("updates selection and reveal when the source location changes", async () => {
    const first: SourceLocation = {
      relativePath: source.relativePath,
      startLine: 2,
      startColumn: 3,
      endLine: 4,
      endColumn: 8,
    };
    const next: SourceLocation = { ...first, startLine: 8, startColumn: 2, endLine: 9, endColumn: 5 };
    const { rerender } = render(<SourceView source={source} location={first} />);

    expect(screen.getByTestId("editor")).toHaveAttribute("data-language", "typescript");
    await waitFor(() => expect(editor.setSelection).toHaveBeenCalledWith({
      startLineNumber: 2,
      startColumn: 3,
      endLineNumber: 4,
      endColumn: 8,
    }));

    editor.setSelection.mockClear();
    editor.revealLineInCenter.mockClear();
    rerender(<SourceView source={source} location={next} />);

    await waitFor(() => expect(editor.setSelection).toHaveBeenCalledWith({
      startLineNumber: 8,
      startColumn: 2,
      endLineNumber: 9,
      endColumn: 5,
    }));
    expect(editor.revealLineInCenter).toHaveBeenCalledWith(8);
  });

  it("lights a reported line without stealing focus in the trace workbench", async () => {
    const location: SourceLocation = {
      relativePath: source.relativePath,
      startLine: 8,
      startColumn: 2,
      endLine: 8,
      endColumn: 5,
    };

    render(<SourceView source={source} location={location} focusOnReveal={false} />);

    await waitFor(() => expect(editor.deltaDecorations).toHaveBeenCalledWith([], [{
      range: {
        startLineNumber: 8,
        startColumn: 1,
        endLineNumber: 8,
        endColumn: 1,
      },
      options: {
        isWholeLine: true,
        className: "source-reported-line",
        linesDecorationsClassName: "source-reported-line-gutter",
      },
    }]));
    expect(editor.focus).not.toHaveBeenCalled();
  });

  it("makes Monaco read-only and blocks editor actions while a workspace opens", () => {
    const onChange = vi.fn();
    const onSave = vi.fn();
    const onFormat = vi.fn();
    const { rerender } = render(
      <SourceView
        source={source}
        editable
        dirty
        workspaceBusy
        onChange={onChange}
        onSave={onSave}
        onFormat={onFormat}
      />,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Opening workspace… editor is read-only");
    expect(monacoProps.options?.readOnly).toBe(true);
    expect(screen.getByRole("button", { name: /Format/ })).toBeDisabled();
    expect(screen.getByRole("button", { name: /Save/ })).toBeDisabled();
    act(() => monacoProps.onChange?.("post-consent edit"));
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));
    expect(onChange).not.toHaveBeenCalled();
    expect(onSave).not.toHaveBeenCalled();
    expect(onFormat).not.toHaveBeenCalled();

    rerender(<SourceView source={source} editable dirty onChange={onChange} onSave={onSave} />);
    expect(monacoProps.options?.readOnly).toBe(false);
    act(() => monacoProps.onChange?.("edit after cancel"));
    expect(onChange).toHaveBeenCalledWith("edit after cancel");
  });
});
