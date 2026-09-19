import Editor, { type BeforeMount, type OnMount } from "@monaco-editor/react";
import { Code, FileCode, FloppyDisk, MagicWand, Warning } from "@phosphor-icons/react";
import { useCallback, useEffect, useRef } from "react";
import "../lib/monaco";
import { DEFAULT_WORKBENCH_SETTINGS } from "../features/editor/settings";
import type { WorkbenchSettings } from "../features/editor/model";
import { monacoThemeName, registerMonacoThemes } from "../features/editor/themes";
import type { SourceFile, SourceLocation } from "../types";

interface SourceViewProps {
  source: SourceFile | null;
  location?: SourceLocation;
  loading?: boolean;
  error?: string | null;
  editable?: boolean;
  dirty?: boolean;
  saving?: boolean;
  formatting?: boolean;
  workspaceBusy?: boolean;
  settings?: WorkbenchSettings;
  onChange?: (content: string) => void;
  onSave?: () => void;
  onFormat?: () => void;
  compact?: boolean;
  focusOnReveal?: boolean;
}

type EditorInstance = Parameters<OnMount>[0];

const MONACO_LANGUAGE_IDS: Record<string, string> = {
  "c#": "csharp",
  "c++": "cpp",
  css: "css",
  graphql: "graphql",
  html: "html",
  java: "java",
  javascript: "javascript",
  json: "json",
  kotlin: "kotlin",
  markdown: "markdown",
  "protocol buffers": "protobuf",
  python: "python",
  rust: "rust",
  shell: "shell",
  sql: "sql",
  text: "plaintext",
  toml: "toml",
  typescript: "typescript",
  yaml: "yaml",
};

export function monacoLanguageId(language: string): string {
  const normalized = language.trim().toLowerCase();
  return MONACO_LANGUAGE_IDS[normalized] ?? (normalized.replace(/[^a-z0-9_-]/g, "") || "plaintext");
}

export function SourceView({
  source,
  location,
  loading = false,
  error = null,
  editable = false,
  dirty = false,
  saving = false,
  formatting = false,
  workspaceBusy = false,
  settings = DEFAULT_WORKBENCH_SETTINGS,
  onChange,
  onSave,
  onFormat,
  compact = false,
  focusOnReveal = true,
}: SourceViewProps) {
  const editorRef = useRef<EditorInstance | null>(null);
  const locationDecorationsRef = useRef<string[]>([]);
  const saveRef = useRef(onSave);
  const formatRef = useRef(onFormat);
  saveRef.current = workspaceBusy ? undefined : onSave;
  formatRef.current = workspaceBusy ? undefined : onFormat;
  const beforeMount: BeforeMount = useCallback((monaco) => {
    registerMonacoThemes(monaco);
  }, []);

  const revealLocation = useCallback((editor: EditorInstance, nextLocation?: SourceLocation) => {
    if (!nextLocation) {
      if (typeof editor.deltaDecorations === "function") {
        locationDecorationsRef.current = editor.deltaDecorations(locationDecorationsRef.current, []);
      }
      editor.setPosition({ lineNumber: 1, column: 1 });
      editor.revealLineNearTop(1);
      return;
    }
    const start = Math.max(1, nextLocation.startLine);
    const end = Math.max(start, nextLocation.endLine);
    editor.setSelection({
      startLineNumber: start,
      startColumn: Math.max(1, nextLocation.startColumn),
      endLineNumber: end,
      endColumn: Math.max(1, nextLocation.endColumn),
    });
    if (typeof editor.deltaDecorations === "function") {
      locationDecorationsRef.current = editor.deltaDecorations(locationDecorationsRef.current, [{
        range: {
          startLineNumber: start,
          startColumn: 1,
          endLineNumber: end,
          endColumn: 1,
        },
        options: {
          isWholeLine: true,
          className: "source-reported-line",
          linesDecorationsClassName: "source-reported-line-gutter",
        },
      }]);
    }
    editor.revealLineInCenter(start);
    if (focusOnReveal) editor.focus();
  }, [focusOnReveal]);

  const onMount: OnMount = useCallback((editor, monaco) => {
    editorRef.current = editor;
    revealLocation(editor, location);
    if (!monaco || typeof editor.addCommand !== "function") return;
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => saveRef.current?.());
    editor.addCommand(monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF, () => formatRef.current?.());
  }, [location, revealLocation]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor || !source) return;
    const frame = window.requestAnimationFrame(() => revealLocation(editor, location));
    return () => window.cancelAnimationFrame(frame);
  }, [location, revealLocation, source?.contentHash, source?.relativePath]);

  if (loading) {
    return (
      <div className="source-state" aria-live="polite">
        <div className="source-skeleton-lines">
          {Array.from({ length: 12 }, (_, index) => <span key={index} style={{ width: `${32 + ((index * 17) % 54)}%` }} />)}
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="source-state" role="alert">
        <Warning size={28} weight="duotone" />
        <strong>Source unavailable</strong>
        <span>{error}</span>
      </div>
    );
  }

  if (!source) {
    return (
      <div className="source-state">
        <Code size={30} weight="thin" />
        <strong>No source selected</strong>
        <span>Choose a file or open a graph node to inspect code.</span>
      </div>
    );
  }

  const rangeLabel = location
    ? `L${location.startLine}${location.endLine && location.endLine !== location.startLine ? `-${location.endLine}` : ""}`
    : `${source.content.split(/\r?\n/).length} lines`;

  return (
    <section className={`source-view${compact ? " is-compact" : ""}`} aria-label={`Source file ${source.relativePath}`}>
      <header className="source-header">
        <FileCode size={14} weight="duotone" aria-hidden="true" />
        <span className="source-path">{source.relativePath}{dirty ? " •" : ""}</span>
        <span className="source-language">{source.language}</span>
        <span className="source-range">{rangeLabel}</span>
        {workspaceBusy && <span className="source-range" role="status" aria-live="polite">Opening workspace… editor is read-only</span>}
        {editable && (
          <div className="source-actions">
            <button type="button" onClick={onFormat} disabled={workspaceBusy || formatting || saving} title="Format document (⇧⌥F)"><MagicWand size={13} />{formatting ? "Formatting" : "Format"}</button>
            <button type="button" onClick={onSave} disabled={workspaceBusy || !dirty || saving || formatting} title="Save (⌘S)"><FloppyDisk size={13} />{saving ? "Saving" : "Save"}</button>
          </div>
        )}
      </header>
      <div className="source-editor">
        <Editor
          path={source.relativePath}
          language={monacoLanguageId(source.language)}
          value={source.content}
          theme={monacoThemeName(settings.theme)}
          beforeMount={beforeMount}
          onMount={onMount}
          onChange={(value) => { if (!workspaceBusy) onChange?.(value ?? ""); }}
          loading={<div className="editor-loading">Loading editor…</div>}
          options={{
            readOnly: !editable || workspaceBusy,
            minimap: { enabled: settings.minimap },
            fontFamily: settings.fontFamily,
            fontSize: settings.fontSize,
            lineHeight: settings.lineHeight,
            padding: { top: 12, bottom: 18 },
            smoothScrolling: true,
            scrollBeyondLastLine: false,
            renderLineHighlight: "all",
            renderWhitespace: "selection",
            overviewRulerBorder: false,
            overviewRulerLanes: 0,
            folding: true,
            glyphMargin: false,
            lineNumbersMinChars: 4,
            wordWrap: settings.wordWrap,
            wordWrapColumn: settings.wordWrapColumn,
            rulers: [settings.wordWrapColumn],
            tabSize: settings.tabSize,
            insertSpaces: settings.insertSpaces,
            detectIndentation: false,
            formatOnPaste: false,
            formatOnType: false,
            automaticLayout: true,
            bracketPairColorization: { enabled: true },
            guides: { bracketPairs: true, indentation: true },
            accessibilitySupport: "auto",
          }}
        />
      </div>
    </section>
  );
}
