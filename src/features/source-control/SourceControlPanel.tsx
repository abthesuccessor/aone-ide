import { ArrowClockwise, Check, FileCode, GitBranch, Minus, Plus } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "../../components/WorkbenchSidebarChrome";
import { Button } from "../../components/ui/button";
import {
  getGitDiff,
  getGitStatus,
  gitCommit,
  gitInitialize,
  gitStage,
  gitUnstage,
} from "../../lib/devtoolsBridge";
import type { GitDiffResult, GitFileStatus, GitStatusResult } from "./model";
import type { EditorOpenMode } from "../editor/model";

interface SourceControlPanelProps {
  workspaceId?: string;
  initialStatus?: GitStatusResult | null;
  selectedPath?: string | null;
  onOpenFile: (relativePath: string, mode?: EditorOpenMode) => void;
  onStatusChange?: (status: GitStatusResult | null) => void;
  onOpenDiff?: (diff: GitDiffResult | null) => void;
  refreshToken?: number;
  disabled?: boolean;
}

const INITIAL_VISIBLE_CHANGES = 160;
const VISIBLE_CHANGE_INCREMENT = 160;

function statusCode(file: GitFileStatus) {
  if (file.conflicted) return "!";
  if (file.indexStatus === "?" || file.workingTreeStatus === "?") return "U";
  return file.staged ? file.indexStatus.trim() || "S" : file.workingTreeStatus.trim() || "M";
}

export function gitDisplayEntries(file: GitFileStatus): GitFileStatus[] {
  if (file.conflicted) return [{ ...file, staged: false }];
  const indexChanged = Boolean(file.indexStatus.trim()) && file.indexStatus !== "?";
  const workingTreeChanged = Boolean(file.workingTreeStatus.trim()) || file.indexStatus === "?";
  const entries: GitFileStatus[] = [];
  if (indexChanged) entries.push({ ...file, staged: true, workingTreeStatus: " " });
  if (workingTreeChanged) entries.push({ ...file, staged: false, indexStatus: " " });
  return entries.length > 0 ? entries : [{ ...file }];
}

export function SourceControlPanel({
  workspaceId,
  initialStatus = null,
  selectedPath = null,
  onOpenFile,
  onStatusChange,
  onOpenDiff,
  refreshToken = 0,
  disabled = false,
}: SourceControlPanelProps) {
  const [status, setStatus] = useState<GitStatusResult | null>(initialStatus);
  const [diff, setDiff] = useState<GitDiffResult | null>(null);
  const [selectedEntryKey, setSelectedEntryKey] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [visibleLimit, setVisibleLimit] = useState(INITIAL_VISIBLE_CHANGES);
  const [error, setError] = useState<string | null>(null);
  const workspaceRef = useRef(workspaceId);
  const previousWorkspaceRef = useRef(workspaceId);
  const disabledRef = useRef(disabled);
  const statusGeneration = useRef(0);
  const diffGeneration = useRef(0);
  const mutationGeneration = useRef(0);
  workspaceRef.current = workspaceId;
  disabledRef.current = disabled;

  const refresh = useCallback(async () => {
    const targetWorkspace = workspaceId;
    const generation = ++statusGeneration.current;
    if (!workspaceId) {
      setStatus(null);
      onStatusChange?.(null);
      return;
    }
    setRefreshing(true);
    setError(null);
    try {
      const nextStatus = await getGitStatus();
      if (generation === statusGeneration.current && workspaceRef.current === targetWorkspace) {
        setStatus(nextStatus);
        onStatusChange?.(nextStatus);
      }
    } catch (failure) {
      if (generation === statusGeneration.current && workspaceRef.current === targetWorkspace) {
        setError(failure instanceof Error ? failure.message : "Could not read Git status");
      }
    } finally {
      if (generation === statusGeneration.current && workspaceRef.current === targetWorkspace) {
        setRefreshing(false);
      }
    }
  }, [onStatusChange, workspaceId]);

  useEffect(() => {
    const workspaceChanged = previousWorkspaceRef.current !== workspaceId;
    previousWorkspaceRef.current = workspaceId;
    statusGeneration.current += 1;
    diffGeneration.current += 1;
    mutationGeneration.current += 1;
    if (workspaceChanged) setStatus(null);
    setDiff(null);
    setSelectedEntryKey(null);
    setMessage("");
    setError(null);
    setBusy(false);
    setVisibleLimit(INITIAL_VISIBLE_CHANGES);
    void refresh();
  }, [refresh, workspaceId]);

  const lastRefreshToken = useRef(refreshToken);
  useEffect(() => {
    if (lastRefreshToken.current === refreshToken) return;
    lastRefreshToken.current = refreshToken;
    const timer = window.setTimeout(() => void refresh(), 120);
    return () => window.clearTimeout(timer);
  }, [refresh, refreshToken]);

  const mutate = async (action: () => Promise<unknown>, onSuccess?: () => void) => {
    if (disabledRef.current) return;
    const targetWorkspace = workspaceId;
    const generation = ++mutationGeneration.current;
    setBusy(true);
    setError(null);
    try {
      await action();
      if (generation !== mutationGeneration.current || workspaceRef.current !== targetWorkspace) return;
      onSuccess?.();
      diffGeneration.current += 1;
      setDiff(null);
      setSelectedEntryKey(null);
      onOpenDiff?.(null);
      await refresh();
    } catch (failure) {
      if (generation === mutationGeneration.current && workspaceRef.current === targetWorkspace) {
        setError(failure instanceof Error ? failure.message : "Git action failed");
      }
    } finally {
      if (generation === mutationGeneration.current && workspaceRef.current === targetWorkspace) {
        setBusy(false);
      }
    }
  };

  const showDiff = async (file: GitFileStatus) => {
    const targetWorkspace = workspaceId;
    const generation = ++diffGeneration.current;
    setError(null);
    try {
      const nextDiff = await getGitDiff({ relativePath: file.relativePath, staged: file.staged });
      if (generation === diffGeneration.current && workspaceRef.current === targetWorkspace) {
        setDiff(nextDiff);
        onOpenDiff?.(nextDiff);
      }
    } catch (failure) {
      if (generation === diffGeneration.current && workspaceRef.current === targetWorkspace) {
        setError(failure instanceof Error ? failure.message : "Could not read diff");
      }
    }
  };

  const displayFiles = useMemo(
    () => status?.files.flatMap(gitDisplayEntries) ?? [],
    [status?.files],
  );
  const visibleFiles = displayFiles.slice(0, visibleLimit);
  const remainingFiles = displayFiles.length - visibleFiles.length;
  useEffect(() => {
    if (!selectedPath) return;
    const selectedIndex = displayFiles.findIndex((file) => file.relativePath === selectedPath);
    if (selectedIndex < visibleLimit) return;
    setVisibleLimit(Math.ceil((selectedIndex + 1) / VISIBLE_CHANGE_INCREMENT) * VISIBLE_CHANGE_INCREMENT);
  }, [displayFiles, selectedPath, visibleLimit]);

  return (
    <aside className="side-feature-panel source-control-panel" aria-label="Source control" aria-busy={refreshing}>
      <WorkbenchSidebarHeader
        title="Source Control"
        actions={<Button variant="ghost" size="icon-xs" type="button" onClick={() => void refresh()} disabled={!workspaceId || busy || refreshing || disabled} aria-label="Refresh Git status"><ArrowClockwise size={14} className={refreshing ? "spin" : undefined} /></Button>}
      />
      {!workspaceId ? (
        <div className="side-feature-empty"><GitBranch size={24} weight="thin" /><span>Open a workspace to use Git.</span></div>
      ) : (
        <>
          {error && <div className="side-feature-error" role="alert">{error}</div>}
          {!status ? (
            !error && <div className="side-feature-empty"><span>Reading local Git status…</span></div>
          ) : !status.isRepository ? (
            <div className="side-feature-empty"><GitBranch size={24} weight="thin" /><span>This workspace is not a Git repository.</span><Button type="button" variant="outline" size="sm" className="secondary-button" onClick={() => void mutate(gitInitialize)} disabled={busy || disabled}>Initialize repository</Button></div>
          ) : (
          <>
            <WorkbenchSidebarSection title="Changes" detail={status.branch ?? "Git"} count={displayFiles.length} />
            <div className="commit-box">
              <textarea value={message} onChange={(event) => setMessage(event.target.value)} placeholder="Commit message" aria-label="Commit message" maxLength={512} disabled={disabled} />
              <Button type="button" size="sm" onClick={() => void mutate(() => gitCommit({ message }), () => setMessage(""))} disabled={busy || disabled || !message.trim() || !displayFiles.some((file) => file.staged)}><Check size={14} weight="bold" /> Commit staged</Button>
            </div>
            <div className="git-files">
            {displayFiles.length === 0 ? <div className="side-feature-empty"><Check size={20} /><span>No local changes.</span></div> : visibleFiles.map((file) => {
              const entryKey = `${file.relativePath}-${file.staged}`;
              const selected = selectedPath
                ? selectedPath === file.relativePath
                : selectedEntryKey === entryKey;
              return (
              <div className={`git-file${selected ? " is-active" : ""}`} key={entryKey}>
                <Button
                  variant="ghost"
                  size="sm"
                  type="button"
                  className="git-file-name"
                  title={file.relativePath}
                  aria-label={`Open ${file.staged ? "staged" : "working tree"} diff for ${file.relativePath}`}
                  aria-current={selected ? "true" : undefined}
                  aria-keyshortcuts="Meta+Enter Control+Enter"
                  onClick={() => { setSelectedEntryKey(entryKey); onOpenFile(file.relativePath, "preview"); void showDiff(file); }}
                  onDoubleClick={() => onOpenFile(file.relativePath, "pinned")}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                      event.preventDefault();
                      onOpenFile(file.relativePath, "pinned");
                    }
                  }}
                ><FileCode size={13} /><span>{file.relativePath}</span><b aria-hidden="true" className={file.conflicted ? "is-conflict" : ""}>{statusCode(file)}</b></Button>
                <Button variant="ghost" size="icon-xs" type="button" title={file.staged ? "Unstage" : "Stage"} aria-label={`${file.staged ? "Unstage" : "Stage"} ${file.relativePath}`} disabled={busy || disabled || file.conflicted} onClick={() => void mutate(() => file.staged ? gitUnstage({ paths: [file.relativePath] }) : gitStage({ paths: [file.relativePath] }))}>{file.staged ? <Minus size={12} /> : <Plus size={12} />}</Button>
              </div>
              );
            })}
            {remainingFiles > 0 && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="git-load-more"
                onClick={() => setVisibleLimit((current) => current + VISIBLE_CHANGE_INCREMENT)}
              >Show {Math.min(VISIBLE_CHANGE_INCREMENT, remainingFiles)} more changes</Button>
            )}
            </div>
          </>
          )}
        </>
      )}
    </aside>
  );
}
