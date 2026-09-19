import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ToastTone } from "../../components/Toast";
import { formatDocument, writeWorkspaceFile } from "../../lib/editorBridge";
import { failureMessage } from "../../lib/errorMessage";
import type { SourceFile } from "../../types";
import type { EditorDocument, WorkbenchSettings } from "./model";

interface EditorControllerOptions {
  source: SourceFile | null;
  workspaceId?: string;
  workspaceGeneration?: number;
  openingWorkspace?: boolean;
  settings: WorkbenchSettings;
  notify: (text: string, tone?: ToastTone) => void;
}

const MAX_DIRTY_PATHS_IN_PROMPT = 5;
const MAX_PROMPT_PATH_CHARS = 120;

function promptSafePath(path: string) {
  return path.replace(/[\u0000-\u001f\u007f]/g, " ").slice(0, MAX_PROMPT_PATH_CHARS);
}

export function useEditorController({
  source,
  workspaceId,
  workspaceGeneration = 0,
  openingWorkspace = false,
  settings,
  notify,
}: EditorControllerOptions) {
  const [documents, setDocuments] = useState<EditorDocument[]>([]);
  const [activePath, setActivePath] = useState<string | null>(null);
  const [savingPath, setSavingPath] = useState<string | null>(null);
  const [formattingPath, setFormattingPath] = useState<string | null>(null);
  const editingLockedRef = useRef(openingWorkspace);
  const editorMutationCountRef = useRef(0);
  const pendingPinnedPathsRef = useRef(new Set<string>());
  const workspaceIdentityRef = useRef({ workspaceId, workspaceGeneration });
  editingLockedRef.current = openingWorkspace;
  if (
    workspaceIdentityRef.current.workspaceId !== workspaceId ||
    workspaceIdentityRef.current.workspaceGeneration !== workspaceGeneration
  ) {
    workspaceIdentityRef.current = { workspaceId, workspaceGeneration };
  }

  useEffect(() => {
    pendingPinnedPathsRef.current.clear();
    setDocuments([]);
    setActivePath(null);
    setSavingPath(null);
    setFormattingPath(null);
  }, [workspaceGeneration, workspaceId]);

  useEffect(() => {
    if (!source || openingWorkspace) return;
    setDocuments((current) => {
      const existing = current.find((item) => item.relativePath === source.relativePath);
      if (existing?.dirty) return current;
      const pinRequested = pendingPinnedPathsRef.current.delete(source.relativePath);
      const next: EditorDocument = {
        ...source,
        dirty: false,
        savedContent: source.content,
        preview: pinRequested ? false : (existing?.preview ?? true),
      };
      if (existing) {
        return current.map((item) => item.relativePath === source.relativePath ? next : item);
      }
      if (pinRequested) return [...current, next];
      const previewIndex = current.findIndex((item) => item.preview === true && !item.dirty);
      if (previewIndex < 0) return [...current, next];
      return current.map((item, index) => index === previewIndex ? next : item);
    });
    setActivePath(source.relativePath);
  }, [openingWorkspace, source]);

  const activeDocument = useMemo(
    () => documents.find((item) => item.relativePath === activePath) ?? null,
    [activePath, documents],
  );

  const updateContent = useCallback((content: string) => {
    if (!activePath || editingLockedRef.current) return;
    setDocuments((current) => current.map((item) => item.relativePath === activePath
      ? { ...item, content, dirty: content !== item.savedContent, preview: false }
      : item));
  }, [activePath]);

  const pinDocument = useCallback((relativePath: string) => {
    if (editingLockedRef.current) return;
    if (!documents.some((item) => item.relativePath === relativePath)) {
      pendingPinnedPathsRef.current.add(relativePath);
    }
    setDocuments((current) => current.map((item) => {
      if (item.relativePath !== relativePath) return item;
      return item.preview ? { ...item, preview: false } : item;
    }));
  }, [documents]);

  const saveDocument = useCallback(async (document = activeDocument) => {
    if (!document || !document.dirty || editingLockedRef.current) return document;
    if (!workspaceId) {
      notify("Open a workspace before saving", "error");
      return null;
    }
    const submittedContent = document.content;
    const requestWorkspace = { workspaceId, workspaceGeneration };
    const sameWorkspace = () =>
      workspaceIdentityRef.current.workspaceId === requestWorkspace.workspaceId &&
      workspaceIdentityRef.current.workspaceGeneration === requestWorkspace.workspaceGeneration;
    const canApplyResult = () => sameWorkspace() && !editingLockedRef.current;
    editorMutationCountRef.current += 1;
    setSavingPath(document.relativePath);
    try {
      let content = submittedContent;
      if (settings.formatOnSave) {
        const formatted = await formatDocument({
          workspaceId,
          relativePath: document.relativePath,
          content,
          expectedContentHash: document.contentHash,
          tabSize: settings.tabSize,
          insertSpaces: settings.insertSpaces,
          printWidth: settings.wordWrapColumn,
        });
        if (!canApplyResult()) return null;
        content = formatted.content;
      }
      const saved = await writeWorkspaceFile({
        workspaceId,
        relativePath: document.relativePath,
        content,
        expectedContentHash: document.contentHash,
      });
      if (!canApplyResult()) return null;
      let next: EditorDocument | null = null;
      setDocuments((current) => current.map((item) => {
        if (!canApplyResult()) return item;
        if (item.relativePath !== saved.relativePath) return item;
        const contentChangedDuringSave = item.content !== submittedContent;
        next = {
          ...saved,
          content: contentChangedDuringSave ? item.content : saved.content,
          dirty: contentChangedDuringSave ? item.content !== saved.content : false,
          savedContent: saved.content,
        };
        return next;
      }));
      if (canApplyResult()) notify(`Saved ${saved.relativePath}`, "success");
      return next;
    } catch (error) {
      if (canApplyResult()) {
        notify(failureMessage(error, "File save failed"), "error");
      }
      return null;
    } finally {
      editorMutationCountRef.current = Math.max(0, editorMutationCountRef.current - 1);
      if (sameWorkspace()) setSavingPath(null);
    }
  }, [activeDocument, notify, settings, workspaceGeneration, workspaceId]);

  const formatActiveDocument = useCallback(async () => {
    const document = activeDocument;
    if (!document || editingLockedRef.current) return;
    if (!workspaceId) {
      notify("Open a workspace before formatting", "error");
      return;
    }
    setDocuments((current) => current.map((item) => item.relativePath === document.relativePath
      ? { ...item, preview: false }
      : item));
    const submittedContent = document.content;
    const requestWorkspace = { workspaceId, workspaceGeneration };
    const sameWorkspace = () =>
      workspaceIdentityRef.current.workspaceId === requestWorkspace.workspaceId &&
      workspaceIdentityRef.current.workspaceGeneration === requestWorkspace.workspaceGeneration;
    const canApplyResult = () => sameWorkspace() && !editingLockedRef.current;
    editorMutationCountRef.current += 1;
    setFormattingPath(document.relativePath);
    try {
      const result = await formatDocument({
        workspaceId,
        relativePath: document.relativePath,
        content: document.content,
        expectedContentHash: document.contentHash,
        tabSize: settings.tabSize,
        insertSpaces: settings.insertSpaces,
        printWidth: settings.wordWrapColumn,
      });
      if (!canApplyResult()) return;
      setDocuments((current) => current.map((item) => {
        if (!canApplyResult()) return item;
        if (item.relativePath !== document.relativePath || item.content !== submittedContent) return item;
        return { ...item, content: result.content, dirty: result.content !== item.savedContent };
      }));
      if (canApplyResult()) {
        notify(`${result.formatter}${result.changed ? " formatted the document" : " found no changes"}`, result.changed ? "success" : "info");
      }
    } catch (error) {
      if (canApplyResult()) {
        notify(failureMessage(error, "Formatting failed"), "error");
      }
    } finally {
      editorMutationCountRef.current = Math.max(0, editorMutationCountRef.current - 1);
      if (sameWorkspace()) setFormattingPath(null);
    }
  }, [activeDocument, notify, settings, workspaceGeneration, workspaceId]);

  const closeDocument = useCallback((relativePath: string) => {
    if (editingLockedRef.current) return false;
    const document = documents.find((item) => item.relativePath === relativePath);
    if (document?.dirty && !window.confirm(`Close ${relativePath} without saving?`)) return false;
    const index = documents.findIndex((item) => item.relativePath === relativePath);
    const next = documents.filter((item) => item.relativePath !== relativePath);
    setDocuments(next);
    if (activePath === relativePath) {
      setActivePath(next[Math.max(0, index - 1)]?.relativePath ?? null);
    }
    return true;
  }, [activePath, documents]);

  const confirmCanLeaveWorkspace = useCallback(() => {
    if (editingLockedRef.current) return false;
    if (editorMutationCountRef.current > 0) {
      notify("Wait for the current save or format to finish before opening another folder", "warning");
      return false;
    }
    const dirtyDocuments = documents.filter((document) => document.dirty);
    if (dirtyDocuments.length === 0) return true;
    const visiblePaths = dirtyDocuments
      .slice(0, MAX_DIRTY_PATHS_IN_PROMPT)
      .map((document) => `• ${promptSafePath(document.relativePath)}`);
    const hiddenCount = dirtyDocuments.length - visiblePaths.length;
    if (hiddenCount > 0) visiblePaths.push(`• and ${hiddenCount} more`);
    return window.confirm(
      `Open another folder and discard unsaved changes in ${dirtyDocuments.length} file${dirtyDocuments.length === 1 ? "" : "s"}?\n\n${visiblePaths.join("\n")}`,
    );
  }, [documents, notify]);

  return {
    documents,
    activeDocument,
    activePath,
    setActivePath,
    pinDocument,
    updateContent,
    closeDocument,
    confirmCanLeaveWorkspace,
    saveDocument,
    formatActiveDocument,
    savingPath,
    formattingPath,
    openingWorkspace,
  };
}
