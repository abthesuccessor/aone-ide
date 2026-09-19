import type { SourceFile } from "../../types";
import type { WorkbenchTheme } from "./themeIds";

export type { WorkbenchTheme } from "./themeIds";
export type EditorWordWrap = "off" | "on" | "bounded";
export type EditorOpenMode = "preview" | "pinned";

export interface WorkbenchSettings {
  theme: WorkbenchTheme;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  tabSize: number;
  insertSpaces: boolean;
  wordWrap: EditorWordWrap;
  wordWrapColumn: number;
  minimap: boolean;
  formatOnSave: boolean;
  terminalFontFamily: string;
  terminalFontSize: number;
  terminalCursorBlink: boolean;
}

export interface EditorDocument extends SourceFile {
  dirty: boolean;
  savedContent: string;
  /** A clean, replaceable VS Code-style preview tab. */
  preview?: boolean;
}

export interface FormatterCapability {
  language: string;
  formatter: string;
  available: boolean;
  external: boolean;
}

export interface FormatDocumentRequest {
  workspaceId: string;
  relativePath: string;
  content: string;
  expectedContentHash: string;
  tabSize: number;
  insertSpaces: boolean;
  printWidth: number;
}

export interface FormatDocumentResult {
  content: string;
  formatter: string;
  changed: boolean;
  usedExternalTool: boolean;
}

export interface WriteWorkspaceFileRequest {
  workspaceId: string;
  relativePath: string;
  content: string;
  expectedContentHash: string;
}
