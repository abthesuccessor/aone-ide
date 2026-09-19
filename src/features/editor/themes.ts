import type { ITheme } from "@xterm/xterm";
import type { editor } from "monaco-editor";
import {
  DEFAULT_WORKBENCH_THEME,
  isWorkbenchTheme,
  WORKBENCH_THEME_IDS,
  type WorkbenchTheme,
} from "./themeIds";

export {
  DEFAULT_WORKBENCH_THEME,
  isWorkbenchTheme,
  WORKBENCH_THEME_IDS,
  type WorkbenchTheme,
} from "./themeIds";

type ThemeTokens = {
  editor: string;
  sideBar: string;
  surface: string;
  titleBar: string;
  activityBar: string;
  input: string;
  hover: string;
  border: string;
  borderStrong: string;
  foreground: string;
  foregroundSecondary: string;
  foregroundMuted: string;
  foregroundEmphasis: string;
  primary: string;
  primaryHover: string;
  primaryMuted: string;
  primaryForeground: string;
  success: string;
  successMuted: string;
  warning: string;
  error: string;
  errorMuted: string;
  focus: string;
  selection: string;
  shadow: string;
  scrollbar: string;
};

export interface WorkbenchThemeDescriptor {
  id: WorkbenchTheme;
  label: string;
  description: string;
  dark: boolean;
  tokens: ThemeTokens;
  monaco: editor.IStandaloneThemeData;
  terminal: ITheme;
}

const cssVariableNames: Record<keyof ThemeTokens, `--workbench-${string}`> = {
  editor: "--workbench-editor",
  sideBar: "--workbench-sidebar",
  surface: "--workbench-surface",
  titleBar: "--workbench-titlebar",
  activityBar: "--workbench-activitybar",
  input: "--workbench-input",
  hover: "--workbench-hover",
  border: "--workbench-border",
  borderStrong: "--workbench-border-strong",
  foreground: "--workbench-foreground",
  foregroundSecondary: "--workbench-foreground-secondary",
  foregroundMuted: "--workbench-foreground-muted",
  foregroundEmphasis: "--workbench-foreground-emphasis",
  primary: "--workbench-primary",
  primaryHover: "--workbench-primary-hover",
  primaryMuted: "--workbench-primary-muted",
  primaryForeground: "--workbench-primary-foreground",
  success: "--workbench-success",
  successMuted: "--workbench-success-muted",
  warning: "--workbench-warning",
  error: "--workbench-error",
  errorMuted: "--workbench-error-muted",
  focus: "--workbench-focus",
  selection: "--workbench-selection",
  shadow: "--workbench-shadow",
  scrollbar: "--workbench-scrollbar",
};

const darkRules: editor.ITokenThemeRule[] = [
  { token: "comment", foreground: "6A9955" },
  { token: "keyword", foreground: "C586C0" },
  { token: "string", foreground: "CE9178" },
  { token: "number", foreground: "B5CEA8" },
  { token: "type", foreground: "4EC9B0" },
  { token: "function", foreground: "DCDCAA" },
];

const definitions: Record<WorkbenchTheme, WorkbenchThemeDescriptor> = {
  "dark-modern": {
    id: "dark-modern",
    label: "VS Code Dark Modern",
    description: "The familiar VS Code and Cursor-style dark workbench.",
    dark: true,
    tokens: {
      editor: "#1f1f1f", sideBar: "#181818", surface: "#202020",
      titleBar: "#181818", activityBar: "#181818", input: "#313131",
      hover: "#2a2d2e", border: "#2b2b2b", borderStrong: "#454545",
      foreground: "#cccccc", foregroundSecondary: "#b8b8b8",
      foregroundMuted: "#858585", foregroundEmphasis: "#f0f0f0",
      primary: "#007acc", primaryHover: "#1f8ad2", primaryMuted: "#007acc22",
      primaryForeground: "#ffffff", success: "#4ec9b0", successMuted: "#4ec9b01c",
      warning: "#dcdcaa", error: "#f48771", errorMuted: "#f487711a",
      focus: "#007fd4", selection: "#264f78", shadow: "#00000066",
      scrollbar: "#79797966",
    },
    monaco: {
      base: "vs-dark", inherit: true, rules: darkRules,
      colors: {
        "editor.background": "#1f1f1f", "editor.foreground": "#d4d4d4",
        "editorLineNumber.foreground": "#858585", "editorLineNumber.activeForeground": "#c6c6c6",
        "editor.selectionBackground": "#264f78", "editor.inactiveSelectionBackground": "#3a3d41",
        "editor.lineHighlightBackground": "#2a2d2e66", "editorCursor.foreground": "#aeafad",
        "editorIndentGuide.background1": "#404040", "editorIndentGuide.activeBackground1": "#707070",
        "editorWhitespace.foreground": "#3b3b3b", "editorGutter.background": "#1f1f1f",
      },
    },
    terminal: { background: "#1f1f1f", foreground: "#cccccc", cursor: "#aeafad", selectionBackground: "#264f78" },
  },
  "light-modern": {
    id: "light-modern", label: "VS Code Light Modern",
    description: "VS Code's clean light workbench with accessible contrast.", dark: false,
    tokens: {
      editor: "#ffffff", sideBar: "#f3f3f3", surface: "#f8f8f8",
      titleBar: "#dddddd", activityBar: "#f3f3f3", input: "#ffffff",
      hover: "#e8e8e8", border: "#d4d4d4", borderStrong: "#b9b9b9",
      foreground: "#1f1f1f", foregroundSecondary: "#3b3b3b", foregroundMuted: "#616161",
      foregroundEmphasis: "#111111", primary: "#0078d4", primaryHover: "#106ebe",
      primaryMuted: "#0078d41a", primaryForeground: "#ffffff", success: "#16825d",
      successMuted: "#16825d18", warning: "#8a6116", error: "#c42b1c",
      errorMuted: "#c42b1c14", focus: "#005fb8", selection: "#add6ff",
      shadow: "#00000033", scrollbar: "#64646466",
    },
    monaco: {
      base: "vs", inherit: true,
      rules: [
        { token: "comment", foreground: "008000" }, { token: "keyword", foreground: "AF00DB" },
        { token: "string", foreground: "A31515" }, { token: "number", foreground: "098658" },
        { token: "type", foreground: "267F99" }, { token: "function", foreground: "795E26" },
      ],
      colors: {
        "editor.background": "#ffffff", "editor.foreground": "#000000",
        "editor.selectionBackground": "#add6ff", "editor.lineHighlightBackground": "#f3f3f3",
        "editorCursor.foreground": "#000000",
      },
    },
    terminal: { background: "#ffffff", foreground: "#1f1f1f", cursor: "#000000", selectionBackground: "#add6ff" },
  },
  "high-contrast": {
    id: "high-contrast", label: "High Contrast Dark",
    description: "Strong borders and focus colors for maximum visibility.", dark: true,
    tokens: {
      editor: "#000000", sideBar: "#000000", surface: "#0a0a0a", titleBar: "#000000",
      activityBar: "#000000", input: "#000000", hover: "#1a1a1a", border: "#6fc3df",
      borderStrong: "#ffffff", foreground: "#ffffff", foregroundSecondary: "#ffffff",
      foregroundMuted: "#d0d0d0", foregroundEmphasis: "#ffffff", primary: "#1aebff",
      primaryHover: "#ffffff", primaryMuted: "#1aebff2b", primaryForeground: "#000000",
      success: "#3ff23f", successMuted: "#3ff23f22", warning: "#ffff00",
      error: "#ff6b6b", errorMuted: "#ff6b6b26", focus: "#f38518",
      selection: "#1aebff55", shadow: "#000000", scrollbar: "#ffffff99",
    },
    monaco: {
      base: "hc-black", inherit: true, rules: [],
      colors: { "editor.background": "#000000", "editor.foreground": "#ffffff", "editorCursor.foreground": "#ffffff", "editor.selectionBackground": "#1aebff55" },
    },
    terminal: { background: "#000000", foreground: "#ffffff", cursor: "#ffffff", selectionBackground: "#1aebff66" },
  },
  "cursor-dark": {
    id: "cursor-dark", label: "Cursor Dark",
    description: "A low-glare, near-black workbench inspired by Cursor.", dark: true,
    tokens: {
      editor: "#141414", sideBar: "#0f0f0f", surface: "#191919", titleBar: "#0b0b0b",
      activityBar: "#0b0b0b", input: "#252525", hover: "#242424", border: "#282828",
      borderStrong: "#3a3a3a", foreground: "#d6d6d6", foregroundSecondary: "#b5b5b5",
      foregroundMuted: "#808080", foregroundEmphasis: "#f2f2f2", primary: "#4d9fff",
      primaryHover: "#70b1ff", primaryMuted: "#4d9fff20", primaryForeground: "#08111c",
      success: "#5cc8a1", successMuted: "#5cc8a11c", warning: "#e6c07b",
      error: "#f07178", errorMuted: "#f071781a", focus: "#4d9fff",
      selection: "#244d72", shadow: "#00000088", scrollbar: "#66666666",
    },
    monaco: {
      base: "vs-dark", inherit: true, rules: darkRules,
      colors: { "editor.background": "#141414", "editor.foreground": "#d6d6d6", "editorGutter.background": "#141414", "editor.selectionBackground": "#244d72", "editor.lineHighlightBackground": "#202020", "editorCursor.foreground": "#d6d6d6" },
    },
    terminal: { background: "#141414", foreground: "#d6d6d6", cursor: "#d6d6d6", selectionBackground: "#244d72" },
  },
  "tokyo-night": {
    id: "tokyo-night", label: "Tokyo Night",
    description: "Deep blue surfaces with crisp violet and cyan accents.", dark: true,
    tokens: {
      editor: "#1a1b26", sideBar: "#16161e", surface: "#1f2335", titleBar: "#16161e",
      activityBar: "#16161e", input: "#24283b", hover: "#292e42", border: "#292e42",
      borderStrong: "#3b4261", foreground: "#c0caf5", foregroundSecondary: "#a9b1d6",
      foregroundMuted: "#737aa2", foregroundEmphasis: "#d5d6f5", primary: "#7aa2f7",
      primaryHover: "#89b4fa", primaryMuted: "#7aa2f722", primaryForeground: "#16161e",
      success: "#9ece6a", successMuted: "#9ece6a1c", warning: "#e0af68",
      error: "#f7768e", errorMuted: "#f7768e1a", focus: "#7aa2f7",
      selection: "#33467c", shadow: "#00000077", scrollbar: "#565f8966",
    },
    monaco: {
      base: "vs-dark", inherit: true,
      rules: [
        { token: "comment", foreground: "565F89" }, { token: "keyword", foreground: "BB9AF7" },
        { token: "string", foreground: "9ECE6A" }, { token: "number", foreground: "FF9E64" },
        { token: "type", foreground: "2AC3DE" }, { token: "function", foreground: "7AA2F7" },
      ],
      colors: { "editor.background": "#1a1b26", "editor.foreground": "#c0caf5", "editorGutter.background": "#1a1b26", "editor.selectionBackground": "#33467c", "editor.lineHighlightBackground": "#1f2335", "editorCursor.foreground": "#c0caf5" },
    },
    terminal: { background: "#1a1b26", foreground: "#c0caf5", cursor: "#c0caf5", selectionBackground: "#33467c" },
  },
  "catppuccin-mocha": {
    id: "catppuccin-mocha", label: "Catppuccin Mocha",
    description: "Warm pastel syntax on a soft dark workbench.", dark: true,
    tokens: {
      editor: "#1e1e2e", sideBar: "#181825", surface: "#242438", titleBar: "#11111b",
      activityBar: "#11111b", input: "#313244", hover: "#313244", border: "#313244",
      borderStrong: "#45475a", foreground: "#cdd6f4", foregroundSecondary: "#bac2de",
      foregroundMuted: "#7f849c", foregroundEmphasis: "#f5e0dc", primary: "#89b4fa",
      primaryHover: "#b4befe", primaryMuted: "#89b4fa22", primaryForeground: "#11111b",
      success: "#a6e3a1", successMuted: "#a6e3a11c", warning: "#f9e2af",
      error: "#f38ba8", errorMuted: "#f38ba81a", focus: "#89b4fa",
      selection: "#45475a", shadow: "#00000077", scrollbar: "#6c708666",
    },
    monaco: {
      base: "vs-dark", inherit: true,
      rules: [
        { token: "comment", foreground: "6C7086" }, { token: "keyword", foreground: "CBA6F7" },
        { token: "string", foreground: "A6E3A1" }, { token: "number", foreground: "FAB387" },
        { token: "type", foreground: "89DCEB" }, { token: "function", foreground: "89B4FA" },
      ],
      colors: { "editor.background": "#1e1e2e", "editor.foreground": "#cdd6f4", "editorGutter.background": "#1e1e2e", "editor.selectionBackground": "#45475a", "editor.lineHighlightBackground": "#242438", "editorCursor.foreground": "#f5e0dc" },
    },
    terminal: { background: "#1e1e2e", foreground: "#cdd6f4", cursor: "#f5e0dc", selectionBackground: "#45475a" },
  },
};

export const WORKBENCH_THEMES = definitions;
export const WORKBENCH_THEME_OPTIONS = WORKBENCH_THEME_IDS.map((id) => definitions[id]);

export function themeDescriptor(id: WorkbenchTheme): WorkbenchThemeDescriptor {
  return definitions[id];
}

export function monacoThemeName(id: WorkbenchTheme): `aone-${WorkbenchTheme}` {
  return `aone-${id}`;
}

export function registerMonacoThemes(monaco: { editor: { defineTheme: (name: string, data: editor.IStandaloneThemeData) => void } }): void {
  for (const definition of WORKBENCH_THEME_OPTIONS) {
    monaco.editor.defineTheme(monacoThemeName(definition.id), definition.monaco);
  }
}

export function applyWorkbenchTheme(id: WorkbenchTheme, root = document.documentElement): void {
  const definition = themeDescriptor(id);
  root.dataset.theme = definition.id;
  root.style.colorScheme = definition.dark ? "dark" : "light";
  for (const key of Object.keys(cssVariableNames) as Array<keyof ThemeTokens>) {
    root.style.setProperty(cssVariableNames[key], definition.tokens[key]);
  }
}
