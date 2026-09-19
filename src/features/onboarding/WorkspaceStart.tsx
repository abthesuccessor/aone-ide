import {
  ArrowLeft,
  CheckCircle,
  FolderOpen,
  GitBranch,
  Plus,
  SpinnerGap,
  Warning,
} from "@phosphor-icons/react";
import { useMemo, useRef, useState } from "react";
import type { WorkspaceSummary } from "../../types";
import type {
  CloneGithubRepositoryRequest,
  CreateDocumentsProjectRequest,
  WorkspaceActionOutcome,
} from "./model";
import { githubDestination, validProjectName } from "./model";

interface WorkspaceStartProps {
  workspace: WorkspaceSummary | null;
  busy: boolean;
  onOpenFolder: () => Promise<void>;
  onClone: (request: CloneGithubRepositoryRequest) => Promise<WorkspaceActionOutcome>;
  onCreate: (request: CreateDocumentsProjectRequest) => Promise<WorkspaceActionOutcome>;
  onOpenAi: () => void;
  onOpenGit: () => void;
  onOpenProjectAgent: () => void;
}

type WorkspaceMode = "choose" | "clone" | "create";

function outcomeError(outcome: WorkspaceActionOutcome): string | null {
  if (outcome.status === "error") return outcome.message;
  if (outcome.status === "stale") return "A newer workspace action replaced this result.";
  return null;
}

export function WorkspaceStart({
  workspace,
  busy,
  onOpenFolder,
  onClone,
  onCreate,
  onOpenAi,
  onOpenGit,
  onOpenProjectAgent,
}: WorkspaceStartProps) {
  const [mode, setMode] = useState<WorkspaceMode>("choose");
  const [repositoryUrl, setRepositoryUrl] = useState("");
  const [projectName, setProjectName] = useState("");
  const [initializeGit, setInitializeGit] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cancelled, setCancelled] = useState(false);
  const [success, setSuccess] = useState<{ action: "clone" | "create"; destination: string } | null>(null);
  const cloneButtonRef = useRef<HTMLButtonElement>(null);
  const createButtonRef = useRef<HTMLButtonElement>(null);
  const clonePreview = useMemo(() => {
    if (!repositoryUrl.trim()) return null;
    try {
      return githubDestination(repositoryUrl);
    } catch {
      return null;
    }
  }, [repositoryUrl]);

  const returnToChoices = () => {
    const priorMode = mode;
    setMode("choose");
    setError(null);
    setCancelled(false);
    window.requestAnimationFrame(() => {
      (priorMode === "clone" ? cloneButtonRef.current : createButtonRef.current)?.focus();
    });
  };

  const submitClone = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    setCancelled(false);
    let request: CloneGithubRepositoryRequest;
    try {
      request = githubDestination(repositoryUrl);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Enter a valid public GitHub URL");
      return;
    }
    setSubmitting(true);
    const outcome = await onClone(request);
    setSubmitting(false);
    const nextError = outcomeError(outcome);
    setError(nextError);
    setCancelled(outcome.status === "cancelled");
    if (outcome.status === "success") {
      setSuccess({ action: "clone", destination: outcome.result.destinationHint });
      setMode("choose");
    }
  };

  const submitCreate = async (event: React.FormEvent) => {
    event.preventDefault();
    setError(null);
    setCancelled(false);
    let name: string;
    try {
      name = validProjectName(projectName);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : "Enter a valid project name");
      return;
    }
    setSubmitting(true);
    const outcome = await onCreate({ projectName: name, initializeGit });
    setSubmitting(false);
    const nextError = outcomeError(outcome);
    setError(nextError);
    setCancelled(outcome.status === "cancelled");
    if (outcome.status === "success") {
      setSuccess({ action: "create", destination: outcome.result.destinationHint });
      setMode("choose");
    }
  };

  const actionBusy = busy || submitting;
  return (
    <section
      className="onboarding-section workspace-start"
      aria-labelledby="onboarding-workspace-title"
      onKeyDown={(event) => {
        if (event.key === "Escape" && mode !== "choose" && !submitting) {
          event.preventDefault();
          event.stopPropagation();
          returnToChoices();
        }
      }}
    >
      <header className="onboarding-section-heading">
        <span className="onboarding-kicker">Workspace</span>
        <h2 id="onboarding-workspace-title">Start from code you control</h2>
        <p>Open a folder, clone a public GitHub repository, or create a clean local project in Documents.</p>
      </header>

      {success && (
        <div className="onboarding-success" role="status">
          <CheckCircle size={17} weight="fill" />
          <div>
            <strong>{success.action === "clone" ? "Repository cloned and opened" : "Project created and opened"}</strong>
            <code>{success.destination}</code>
          </div>
          <button type="button" onClick={() => setSuccess(null)} aria-label="Dismiss workspace success">Dismiss</button>
        </div>
      )}
      {cancelled && <div className="onboarding-neutral" role="status">Workspace action cancelled. Nothing was changed in this view.</div>}

      {mode === "choose" ? (
        <div className="workspace-actions" aria-label="Workspace choices">
          <button type="button" onClick={() => void onOpenFolder()} disabled={actionBusy}>
            <span className="workspace-action-icon"><FolderOpen size={18} /></span>
            <span><strong>Open existing folder</strong><small>Choose any local project folder with the native picker.</small></span>
            <span className="workspace-action-key">⌘O</span>
          </button>
          <button ref={cloneButtonRef} type="button" onClick={() => { setMode("clone"); setError(null); setCancelled(false); }} disabled={actionBusy}>
            <span className="workspace-action-icon"><GitBranch size={18} /></span>
            <span><strong>Clone public GitHub repository</strong><small>Clone an HTTPS github.com URL into your Documents folder.</small></span>
            <span>Configure</span>
          </button>
          <button ref={createButtonRef} type="button" onClick={() => { setMode("create"); setError(null); setCancelled(false); }} disabled={actionBusy}>
            <span className="workspace-action-icon"><Plus size={18} /></span>
            <span><strong>Create new project</strong><small>Create a folder in Documents with optional Git initialization.</small></span>
            <span>Configure</span>
          </button>
        </div>
      ) : mode === "clone" ? (
        <form className="workspace-form" onSubmit={(event) => void submitClone(event)}>
          <button type="button" className="onboarding-back" onClick={returnToChoices} disabled={submitting}>
            <ArrowLeft size={13} /> Workspace choices
          </button>
          <label htmlFor="onboarding-github-url">Public GitHub repository URL</label>
          <input
            id="onboarding-github-url"
            type="url"
            required
            autoFocus
            autoComplete="off"
            spellCheck={false}
            value={repositoryUrl}
            placeholder="https://github.com/owner/repository"
            onChange={(event) => { setRepositoryUrl(event.target.value); setError(null); }}
            aria-describedby="onboarding-clone-help"
          />
          <p id="onboarding-clone-help">HTTPS github.com URLs only. Credentials, query text, and fragments are rejected.</p>
          <div className="workspace-destination">
            <span>Destination</span>
            <code>{clonePreview ? `Documents/${clonePreview.destinationName}` : "Derived after a valid URL"}</code>
          </div>
          {error && <div className="onboarding-error" role="alert"><Warning size={14} />{error}</div>}
          <div className="workspace-form-actions">
            <button type="button" className="secondary-button" onClick={returnToChoices} disabled={submitting}>Cancel</button>
            <button type="submit" className="primary-button" disabled={actionBusy || !clonePreview}>
              {submitting ? <SpinnerGap className="spin" size={14} /> : <GitBranch size={14} />}
              {submitting ? "Cloning and indexing" : "Clone to Documents"}
            </button>
          </div>
        </form>
      ) : (
        <form className="workspace-form" onSubmit={(event) => void submitCreate(event)}>
          <button type="button" className="onboarding-back" onClick={returnToChoices} disabled={submitting}>
            <ArrowLeft size={13} /> Workspace choices
          </button>
          <label htmlFor="onboarding-project-name">Project name</label>
          <input
            id="onboarding-project-name"
            required
            autoFocus
            autoComplete="off"
            spellCheck={false}
            value={projectName}
            placeholder="my-project"
            onChange={(event) => { setProjectName(event.target.value); setError(null); }}
            aria-describedby="onboarding-create-help"
          />
          <p id="onboarding-create-help">The native command validates the final folder name and destination.</p>
          <label className="onboarding-checkbox">
            <input type="checkbox" checked={initializeGit} onChange={(event) => setInitializeGit(event.target.checked)} />
            <span><strong>Initialize Git repository</strong><small>Runs Git initialization only after you create the project.</small></span>
          </label>
          <div className="workspace-destination"><span>Destination</span><code>{projectName.trim() ? `Documents/${projectName.trim()}` : "Documents/project-name"}</code></div>
          {error && <div className="onboarding-error" role="alert"><Warning size={14} />{error}</div>}
          <div className="workspace-form-actions">
            <button type="button" className="secondary-button" onClick={returnToChoices} disabled={submitting}>Cancel</button>
            <button type="submit" className="primary-button" disabled={actionBusy || !projectName.trim()}>
              {submitting ? <SpinnerGap className="spin" size={14} /> : <Plus size={14} />}
              {submitting ? "Creating and indexing" : "Create in Documents"}
            </button>
          </div>
        </form>
      )}

      {workspace && mode === "choose" && (
        <div className="onboarding-next-actions">
          <span>Workspace ready: <strong>{workspace.name}</strong></span>
          <div>
            <button type="button" onClick={onOpenAi}>Configure AI provider</button>
            <button type="button" onClick={onOpenGit}>Review Git workflow</button>
            <button type="button" onClick={onOpenProjectAgent}>Open Project Agent</button>
          </div>
        </div>
      )}
    </section>
  );
}
