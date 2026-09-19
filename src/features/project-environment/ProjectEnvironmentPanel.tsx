import {
  ArrowClockwise,
  CheckCircle,
  Cpu,
  FolderOpen,
  Play,
  Robot,
  ShieldCheck,
  Stack,
  Warning,
  XCircle,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "../../components/WorkbenchSidebarChrome";
import {
  explainProjectEnvironment,
  inspectProjectEnvironment,
} from "../../lib/projectEnvironmentBridge";
import type { AiExplanation } from "../../types";
import type {
  ProjectEnvironmentReport,
  ProjectTool,
} from "./model";

interface ProjectEnvironmentPanelProps {
  workspaceId?: string;
  workspaceGeneration: number;
  disabled?: boolean;
  runProfileIds: string[];
  activeRunProfileId: string;
  onUseRunProfile: (profileId: string) => void;
}

type InspectionState = "idle" | "inspecting" | "cancelled" | "ready";

function failureMessage(failure: unknown, fallback: string) {
  if (failure instanceof Error && failure.message.trim()) return failure.message;
  if (typeof failure === "string" && failure.trim()) return failure;
  return fallback;
}

function statusIcon(tool: ProjectTool) {
  if (tool.status === "available") return <CheckCircle size={13} weight="fill" />;
  if (tool.status === "missing") return <XCircle size={13} weight="fill" />;
  return <Warning size={13} weight="fill" />;
}

function visibleTools(report: ProjectEnvironmentReport, showAll: boolean) {
  if (showAll) return report.tools;
  return report.tools.filter(
    (tool) => tool.required || tool.usedBy.length > 0 || tool.status === "available",
  );
}

export function ProjectEnvironmentPanel({
  workspaceId,
  workspaceGeneration,
  disabled = false,
  runProfileIds,
  activeRunProfileId,
  onUseRunProfile,
}: ProjectEnvironmentPanelProps) {
  const [state, setState] = useState<InspectionState>("idle");
  const [report, setReport] = useState<ProjectEnvironmentReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAllTools, setShowAllTools] = useState(false);
  const [explanation, setExplanation] = useState<AiExplanation | null>(null);
  const [explanationError, setExplanationError] = useState<string | null>(null);
  const [explaining, setExplaining] = useState(false);
  const inspectionRequest = useRef(0);
  const explanationRequest = useRef(0);
  const workspaceRef = useRef({ workspaceId, workspaceGeneration });
  workspaceRef.current = { workspaceId, workspaceGeneration };

  useEffect(() => {
    inspectionRequest.current += 1;
    explanationRequest.current += 1;
    setState("idle");
    setReport(null);
    setError(null);
    setShowAllTools(false);
    setExplanation(null);
    setExplanationError(null);
    setExplaining(false);
  }, [workspaceGeneration, workspaceId]);

  useEffect(() => {
    if (!disabled) return;
    inspectionRequest.current += 1;
    explanationRequest.current += 1;
    setState("idle");
    setReport(null);
    setError(null);
    setShowAllTools(false);
    setExplanation(null);
    setExplanationError(null);
    setExplaining(false);
  }, [disabled]);

  const tools = useMemo(
    () => report ? visibleTools(report, showAllTools) : [],
    [report, showAllTools],
  );

  const inspect = async () => {
    if (!workspaceId || disabled || state === "inspecting") return;
    const target = { workspaceId, workspaceGeneration };
    const request = ++inspectionRequest.current;
    explanationRequest.current += 1;
    setState("inspecting");
    setError(null);
    setExplanation(null);
    setExplanationError(null);
    try {
      const next = await inspectProjectEnvironment(workspaceId);
      const current = workspaceRef.current;
      if (request !== inspectionRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration) return;
      if (!next) {
        setReport(null);
        setState("cancelled");
        return;
      }
      if (next.workspaceId !== workspaceId) throw new Error("Native environment report did not match the active workspace");
      setReport(next);
      setState("ready");
    } catch (failure) {
      const current = workspaceRef.current;
      if (request === inspectionRequest.current
        && current.workspaceId === target.workspaceId
        && current.workspaceGeneration === target.workspaceGeneration) {
        setState("idle");
        setError(failureMessage(failure, "Could not inspect the project environment"));
      }
    }
  };

  const explain = async () => {
    if (!workspaceId || !report || disabled || explaining) return;
    const target = { workspaceId, workspaceGeneration, reportId: report.reportId };
    const request = ++explanationRequest.current;
    setExplaining(true);
    setExplanation(null);
    setExplanationError(null);
    try {
      const next = await explainProjectEnvironment(workspaceId, report.reportId);
      const current = workspaceRef.current;
      if (request !== explanationRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration
        || report.reportId !== target.reportId) return;
      setExplanation(next);
    } catch (failure) {
      const current = workspaceRef.current;
      if (request === explanationRequest.current
        && current.workspaceId === target.workspaceId
        && current.workspaceGeneration === target.workspaceGeneration) {
        setExplanationError(failureMessage(failure, "AI could not explain this setup"));
      }
    } finally {
      if (request === explanationRequest.current) setExplaining(false);
    }
  };

  return (
    <aside className="side-feature-panel project-environment-panel" aria-label="Project setup">
      <WorkbenchSidebarHeader
        title="Project Setup"
        actions={report && <button type="button" onClick={() => void inspect()} aria-label="Inspect project environment again" disabled={disabled || state === "inspecting"}><ArrowClockwise size={14} /></button>}
      />
      <WorkbenchSidebarSection
        title="Local runtime"
        detail={!workspaceId ? "No folder" : state === "inspecting" ? "Inspecting" : report ? "Inspection ready" : "Not inspected"}
        count={report?.tools.length}
      />

      {!workspaceId ? (
        <div className="side-feature-empty"><FolderOpen size={22} /><strong>Open a folder first</strong><span>Aone needs a workspace before it can recommend a local runtime.</span></div>
      ) : !report ? (
        <div className="environment-consent-intro">
          <ShieldCheck size={25} weight="duotone" />
          <strong>Inspection only starts when you ask</strong>
          <p>The first native dialog asks before Aone reads bounded build/config sources and executable paths. A second dialog asks before fixed, read-only version probes run.</p>
          <ul><li>Names-only environment and dependency hints · never values or proven requirements</li><li>No project env files, shell startup files, installers, or automatic configuration</li><li>Declining either step is safe</li></ul>
          {state === "cancelled" && <div className="environment-cancelled" role="status">Inspection cancelled. No report was retained.</div>}
          {error && <div className="side-feature-error" role="alert">{error}</div>}
          <button type="button" className="primary-button" onClick={() => void inspect()} disabled={disabled || state === "inspecting"}>
            <Cpu size={14} />{state === "inspecting" ? "Waiting for permission…" : "Inspect project environment"}
          </button>
        </div>
      ) : (
        <div className="environment-report" aria-live="polite">
          <div className={`environment-approval ${report.versionProbeApproved ? "is-approved" : "is-partial"}`}>
            <ShieldCheck size={14} weight="fill" />
            <div><strong>Paths inspected with permission</strong><span>{report.versionProbeApproved ? "Fixed version probes approved" : "Version probes skipped by user"}</span></div>
          </div>

          <section className="environment-section" aria-labelledby="environment-stacks-title">
            <header><Stack size={13} /><strong id="environment-stacks-title">Detected stack</strong><span>{report.stacks.length}</span></header>
            {report.stacks.map((stack) => (
              <article className="environment-stack" key={stack.id}>
                <div><strong>{stack.label}</strong><span className={`confidence-${stack.confidence}`}>{stack.confidence}</span></div>
                {stack.evidence.map((item) => <code key={`${item.relativePath}:${item.detail}`} title={item.detail}>{item.relativePath}</code>)}
              </article>
            ))}
          </section>

          <section className="environment-section" aria-labelledby="environment-tools-title">
            <header><Cpu size={13} /><strong id="environment-tools-title">Local tools</strong><span>{report.tools.length}</span></header>
            <div className="environment-tools">
              {tools.map((tool) => (
                <article className={`environment-tool status-${tool.status}`} key={tool.id}>
                  <div className="environment-tool-state">{statusIcon(tool)}<strong>{tool.label}</strong>{tool.required && <small>required</small>}</div>
                  <span>{tool.status === "available" ? tool.version ?? "Available" : tool.status === "unverified" ? "Path found · version not run" : "Not found"}</span>
                  {tool.canonicalPath && <code title={tool.canonicalPath}>{tool.canonicalPath}</code>}
                  {tool.alternateCanonicalPaths.length > 0 && <small>{tool.alternateCanonicalPaths.length} alternate {tool.alternateCanonicalPaths.length === 1 ? "path" : "paths"}</small>}
                  {tool.probeError && <small className="tool-probe-error" title={tool.probeError}>Version unavailable</small>}
                </article>
              ))}
            </div>
            {tools.length < report.tools.length && <button type="button" className="environment-show-all" onClick={() => setShowAllTools(true)}>Show all {report.tools.length} catalog tools</button>}
            {showAllTools && <button type="button" className="environment-show-all" onClick={() => setShowAllTools(false)}>Show relevant tools only</button>}
          </section>

          <section className="environment-section" aria-labelledby="environment-recommendations-title">
            <header><Play size={13} /><strong id="environment-recommendations-title">Recommendations</strong><span>{report.recommendations.length}</span></header>
            {report.recommendations.map((recommendation) => {
              const profileAvailable = Boolean(recommendation.runProfileId && runProfileIds.includes(recommendation.runProfileId));
              const profileActive = recommendation.runProfileId === activeRunProfileId;
              return (
                <article className={`environment-recommendation severity-${recommendation.severity}`} key={recommendation.id}>
                  <div><strong>{recommendation.title}</strong><span>{recommendation.severity}</span></div>
                  <p>{recommendation.summary}</p>
                  {recommendation.evidence.map((item) => <code key={`${item.relativePath}:${item.detail}`} title={item.detail}>{item.relativePath}</code>)}
                  {recommendation.runProfileId && <button type="button" className="secondary-button" disabled={disabled || !profileAvailable || profileActive} onClick={() => onUseRunProfile(recommendation.runProfileId!)}>{profileActive ? "Profile selected" : profileAvailable ? "Use run profile" : "Profile unavailable"}</button>}
                </article>
              );
            })}
          </section>

          <section className="environment-ai" aria-labelledby="environment-ai-title">
            <div><Robot size={14} /><strong id="environment-ai-title">AI explanation</strong></div>
            <p>Another native dialog asks before bounded project facts are sent to the configured provider. Absolute tool paths stay local.</p>
            <button type="button" className="secondary-button" onClick={() => void explain()} disabled={disabled || explaining}>{explaining ? "Waiting for permission…" : "Ask AI to explain setup"}</button>
            {explanationError && <div className="side-feature-error" role="alert">{explanationError}</div>}
            {explanation && <div className="environment-ai-answer"><span>AI output is inferred · {explanation.model}</span><p>{explanation.answer}</p><div>{explanation.evidence.map((item) => <small key={`${item.kind}:${item.id}`}>{item.label} · {item.evidence}</small>)}</div></div>}
          </section>
        </div>
      )}
    </aside>
  );
}
