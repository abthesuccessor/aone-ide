import {
  ArrowClockwise,
  CheckCircle,
  GitBranch,
  Key,
  ShieldCheck,
  SpinnerGap,
  Warning,
  X,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { inspectGitOnboarding } from "../../lib/onboardingBridge";
import type { GitOnboardingReport } from "./model";

interface GitOnboardingProps {
  workspaceId?: string;
  workspaceGeneration: number;
  disabled?: boolean;
}

type InspectionState = "idle" | "inspecting" | "cancelled" | "ready";

const workflow = [
  { title: "Fork or choose the source repository", detail: "Create a fork in the account that should own your contribution, or confirm you can push to the source repository." },
  { title: "Choose the remote and account", detail: "Review the sanitized remote below. Confirm the host and owner before changing origin or adding upstream." },
  { title: "Create a branch", detail: "Use a short topic branch, for example git switch -c feature/onboarding." },
  { title: "Review and commit", detail: "Inspect git diff and git status, then create a focused commit with your own message." },
  { title: "Push the branch", detail: "Run git push -u origin <branch> only after confirming the remote and account." },
  { title: "Open a pull request", detail: "Review the final diff on the Git host and submit the pull request yourself." },
];

function failureText(failure: unknown): string {
  if (failure instanceof Error && failure.message.trim()) return failure.message;
  if (typeof failure === "string" && failure.trim()) return failure;
  return "Could not inspect Git setup";
}

export function GitOnboarding({ workspaceId, workspaceGeneration, disabled = false }: GitOnboardingProps) {
  const [state, setState] = useState<InspectionState>("idle");
  const [report, setReport] = useState<GitOnboardingReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const targetRef = useRef({ workspaceId, workspaceGeneration });
  const inspectButtonRef = useRef<HTMLButtonElement>(null);
  targetRef.current = { workspaceId, workspaceGeneration };

  useEffect(() => {
    requestRef.current += 1;
    setState("idle");
    setReport(null);
    setError(null);
  }, [workspaceGeneration, workspaceId]);

  useEffect(() => {
    if (state === "cancelled") inspectButtonRef.current?.focus();
  }, [state]);

  const inspect = async () => {
    if (!workspaceId || disabled || state === "inspecting") return;
    const target = { workspaceId, workspaceGeneration };
    const request = ++requestRef.current;
    setState("inspecting");
    setReport(null);
    setError(null);
    try {
      const next = await inspectGitOnboarding({ workspaceId });
      const current = targetRef.current;
      if (request !== requestRef.current || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration) return;
      if (!next) {
        setState("cancelled");
        return;
      }
      if (next.workspaceId !== target.workspaceId) throw new Error("Git report did not match the active workspace");
      setReport(next);
      setState("ready");
    } catch (failure) {
      const current = targetRef.current;
      if (request === requestRef.current && current.workspaceId === target.workspaceId
        && current.workspaceGeneration === target.workspaceGeneration) {
        setState("idle");
        setError(failureText(failure));
      }
    }
  };

  const cancel = () => {
    requestRef.current += 1;
    setState("cancelled");
    setReport(null);
    setError(null);
  };

  return (
    <section
      className="onboarding-section git-onboarding"
      aria-labelledby="onboarding-git-title"
      onKeyDown={(event) => {
        if (event.key === "Escape" && state === "inspecting") {
          event.preventDefault();
          event.stopPropagation();
          cancel();
        }
      }}
    >
      <header className="onboarding-section-heading">
        <span className="onboarding-kicker">Git workflow</span>
        <h2 id="onboarding-git-title">Inspect metadata, then choose each Git action</h2>
        <p>Aone can report sanitized Git setup metadata after permission. The checklist below is guidance and does not run commands.</p>
      </header>

      <details className="git-disclosure">
        <summary><ShieldCheck size={14} /> What inspection can read</summary>
        <div>
          <p>Repository state, current branch, configured identity labels, sanitized remote coordinates, and public key fingerprints.</p>
          <strong>Private key contents: never read</strong>
        </div>
      </details>

      <div className="git-inspection-actions">
        <button
          ref={inspectButtonRef}
          type="button"
          className="primary-button"
          onClick={() => void inspect()}
          disabled={!workspaceId || disabled || state === "inspecting"}
        >
          {state === "ready" ? <ArrowClockwise size={14} /> : <GitBranch size={14} />}
          {state === "ready" ? "Inspect Git setup again" : "Inspect Git setup"}
        </button>
        {state === "inspecting" && (
          <button type="button" className="secondary-button" onClick={cancel}>
            <X size={13} /> Cancel inspection
          </button>
        )}
        {state === "inspecting" && <span role="status"><SpinnerGap className="spin" size={13} /> Waiting for permission</span>}
      </div>

      {!workspaceId && <div className="onboarding-neutral"><Warning size={14} />Open a workspace before inspecting Git setup.</div>}
      {state === "cancelled" && <div className="onboarding-neutral" role="status">Inspection cancelled. No report was retained.</div>}
      {error && <div className="onboarding-error" role="alert"><Warning size={14} />{error}</div>}

      {report && (
        <div className="git-report" aria-label="Sanitized Git setup report">
          <header>
            <CheckCircle size={15} weight="fill" />
            <div><strong>{report.isRepository ? "Git repository detected" : "No Git repository detected"}</strong><span>{report.branch ? `Branch: ${report.branch}` : "No current branch reported"}</span></div>
          </header>
          <dl>
            <div><dt>Identity</dt><dd>{report.identity ? `${report.identity.name ?? "Name not reported"}${report.identity.email ? `, ${report.identity.email}` : ""} (${report.identity.source})` : "Not configured"}</dd></div>
            <div><dt>Remotes</dt><dd>{report.remotes.length}</dd></div>
            <div><dt>Public keys</dt><dd>{report.publicKeys.length}</dd></div>
          </dl>
          {report.remotes.map((remote) => (
            <div className="git-remote" key={`${remote.name}:${remote.displayUrl}`}>
              <strong>{remote.name}</strong><code>{remote.displayUrl}</code><span>{remote.transport}{remote.ownerRepo ? `, ${remote.ownerRepo}` : ""}</span>
            </div>
          ))}
          {report.publicKeys.map((key) => (
            <div className="git-public-key" key={key.fileName}>
              <Key size={13} /><strong>{key.fileName}</strong><span>{key.keyType}</span><code>{key.fingerprint}</code>
            </div>
          ))}
          <div className="git-private-boundary"><ShieldCheck size={14} /><strong>Private key contents: never read</strong></div>
        </div>
      )}

      <section className="git-guidance" aria-labelledby="git-guidance-title">
        <header><strong id="git-guidance-title">Guided manual workflow</strong><span>Aone has not executed these actions.</span></header>
        <ol>{workflow.map((item) => <li key={item.title}><strong>{item.title}</strong><p>{item.detail}</p></li>)}</ol>
      </section>
      <div className="git-approval-note">
        <ShieldCheck size={15} />
        <p><strong>Approval boundary</strong><span>Review the account, remote, branch, commit, push command, and pull request before you run or submit anything.</span></p>
      </div>
    </section>
  );
}
