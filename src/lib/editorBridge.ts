import type { SourceFile } from "../types";
import type {
  FormatDocumentRequest,
  FormatDocumentResult,
  FormatterCapability,
  WriteWorkspaceFileRequest,
} from "../features/editor/model";

function desktopRuntime() {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

function browserHash(content: string) {
  let hash = 2_166_136_261;
  for (const byte of new TextEncoder().encode(content)) {
    hash ^= byte;
    hash = Math.imul(hash, 16_777_619);
  }
  return `browser-demo-${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

export async function getFormatterCapabilities(): Promise<FormatterCapability[]> {
  if (desktopRuntime()) return invokeDesktop("get_formatter_capabilities");
  return [
    { language: "Text", formatter: "Aone whitespace normalizer", available: true, external: false },
    { language: "TypeScript", formatter: "Prettier", available: false, external: true },
    { language: "Rust", formatter: "rustfmt", available: false, external: true },
  ];
}

export async function formatDocument(request: FormatDocumentRequest): Promise<FormatDocumentResult> {
  if (desktopRuntime()) return invokeDesktop("format_document", { request });
  const content = request.content
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((line) => line.replace(/[\t ]+$/g, ""))
    .join("\n")
    .replace(/\n*$/, "\n");
  return {
    content,
    formatter: "Aone whitespace normalizer (browser demo)",
    changed: content !== request.content,
    usedExternalTool: false,
  };
}

export async function writeWorkspaceFile(request: WriteWorkspaceFileRequest): Promise<SourceFile> {
  if (desktopRuntime()) return invokeDesktop("write_workspace_file", { request });
  return {
    relativePath: request.relativePath,
    language: "Text",
    content: request.content,
    contentHash: browserHash(request.content),
  };
}

export async function pickAndCreateWorkspaceFile(): Promise<SourceFile | null> {
  if (desktopRuntime()) return invokeDesktop("pick_and_create_workspace_file");
  return {
    relativePath: "untitled.txt",
    language: "Text",
    content: "",
    contentHash: browserHash(""),
  };
}
