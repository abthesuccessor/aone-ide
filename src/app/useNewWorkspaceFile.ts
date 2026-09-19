import { useCallback, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import type { ToastTone } from "../components/Toast";
import { failureMessage } from "../lib/errorMessage";
import { pickAndCreateWorkspaceFile } from "../lib/editorBridge";
import type { SourceFile, SourceLocation, WorkspaceSummary } from "../types";
import type { MainView } from "./model";

interface NewWorkspaceFileOptions {
  workspace: WorkspaceSummary | null;
  openingWorkspaceRef: MutableRefObject<boolean>;
  loadFilesAndGraph: () => Promise<void>;
  notify: (text: string, tone?: ToastTone) => void;
  setSource: Dispatch<SetStateAction<SourceFile | null>>;
  setSourceLocation: Dispatch<SetStateAction<SourceLocation | null>>;
  setSourceError: Dispatch<SetStateAction<string | null>>;
  setSourceLoading: Dispatch<SetStateAction<boolean>>;
  setMainView: Dispatch<SetStateAction<MainView>>;
}

export function useNewWorkspaceFile(options: NewWorkspaceFileOptions) {
  const {
    workspace, openingWorkspaceRef, loadFilesAndGraph, notify,
    setSource, setSourceLocation, setSourceError, setSourceLoading, setMainView,
  } = options;
  return useCallback(async () => {
    if (!workspace || openingWorkspaceRef.current) {
      notify("Open a workspace before creating a file", "warning");
      return;
    }
    try {
      const created = await pickAndCreateWorkspaceFile();
      if (!created) return;
      setSource(created);
      setSourceLocation(null);
      setSourceError(null);
      setSourceLoading(false);
      setMainView("graph");
      await loadFilesAndGraph();
      notify(`Created ${created.relativePath}`, "success");
    } catch (error) {
      notify(failureMessage(error, "Unable to create file"), "error");
    }
  }, [
    loadFilesAndGraph, notify, openingWorkspaceRef, setMainView, setSource,
    setSourceError, setSourceLoading, setSourceLocation, workspace,
  ]);
}
