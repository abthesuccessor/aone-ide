import {
  ArrowClockwise,
  ArrowRight,
  CheckCircle,
  ChatCenteredDots,
  FolderOpen,
  Robot,
  ShieldCheck,
  SpinnerGap,
  UserCircle,
  Warning,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "../../components/WorkbenchSidebarChrome";
import {
  askProjectAgent,
  getProjectEnvironmentReport,
  inspectProjectEnvironment,
  type ProjectAgentResponse,
} from "../../lib/projectEnvironmentBridge";
import type { AiConfigurationStatus, EvidenceReference } from "../../types";
import type { ProjectEnvironmentReport } from "../project-environment/model";
import {
  buildAgentSetupPlan,
  countApprovableAgentSteps,
  type AgentRunProfileSummary,
  type AgentSetupPlanStep,
} from "./model";

interface AgentSetupPanelProps {
  workspaceId?: string;
  workspaceGeneration: number;
  disabled?: boolean;
  aiStatus: AiConfigurationStatus | null;
  runProfiles: AgentRunProfileSummary[];
  activeRunProfileId: string;
  onUseRunProfile: (profileId: string) => void;
  onConfigureAi?: () => void;
}

type InspectionPhase = "restoring" | "idle" | "inspecting" | "cancelled" | "ready";
type ChatPhase = "idle" | "thinking" | "error";

interface ChatMessage {
  id: number;
  role: "engineer" | "agent";
  body: string;
  model?: string;
  evidence?: EvidenceReference[];
}

interface ChatFailure {
  message: string;
  retryable: boolean;
}

const QUICK_QUESTIONS = [
  "How should I configure this project's .env files?",
  "What must run before I can test this service?",
  "Does this project need Docker or other services?",
];

function safeFailureMessage(failure: unknown, fallback: string): string {
  if (failure instanceof Error && failure.message.trim()) return failure.message;
  if (typeof failure === "string" && failure.trim()) return failure;
  return fallback;
}

function agentLabel(status: AiConfigurationStatus | null): string {
  if (!status?.inferenceAvailable) return "No AI connected";
  if (status.provider === "codex") return "Codex CLI";
  if (status.provider === "ollama") return `Ollama · ${status.model}`;
  if (status.provider === "anthropic") return `Anthropic · ${status.model}`;
  return `OpenAI · ${status.model}`;
}

function stepStateLabel(step: AgentSetupPlanStep): string {
  if (step.state === "awaiting-approval") return "You approve";
  if (step.state === "already-applied") return "Applied in IDE";
  return "Engineer action";
}

function completedResponse(response: ProjectAgentResponse): response is Extract<ProjectAgentResponse, { status: "completed" }> {
  return response.status === "completed";
}

export function AgentSetupPanel({
  workspaceId,
  workspaceGeneration,
  disabled = false,
  aiStatus,
  runProfiles,
  activeRunProfileId,
  onUseRunProfile,
  onConfigureAi,
}: AgentSetupPanelProps) {
  const [inspectionPhase, setInspectionPhase] = useState<InspectionPhase>(workspaceId ? "restoring" : "idle");
  const [report, setReport] = useState<ProjectEnvironmentReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [approvalStepId, setApprovalStepId] = useState<string | null>(null);
  const [lastActivity, setLastActivity] = useState<string | null>(null);
  const [chatPhase, setChatPhase] = useState<ChatPhase>("idle");
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatDraft, setChatDraft] = useState("");
  const [chatFailure, setChatFailure] = useState<ChatFailure | null>(null);
  const [lastQuestion, setLastQuestion] = useState<string | null>(null);
  const inspectionRequest = useRef(0);
  const chatRequest = useRef(0);
  const messageId = useRef(0);
  const workspaceRef = useRef({ workspaceId, workspaceGeneration });
  workspaceRef.current = { workspaceId, workspaceGeneration };

  useEffect(() => {
    const request = ++inspectionRequest.current;
    chatRequest.current += 1;
    setInspectionPhase(workspaceId ? "restoring" : "idle");
    setReport(null);
    setError(null);
    setApprovalStepId(null);
    setLastActivity(null);
    setChatPhase("idle");
    setChatMessages([]);
    setChatDraft("");
    setChatFailure(null);
    setLastQuestion(null);
    if (!workspaceId) return;

    const target = { workspaceId, workspaceGeneration };
    void getProjectEnvironmentReport(workspaceId).then((cached) => {
      const current = workspaceRef.current;
      if (request !== inspectionRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration) return;
      if (cached && cached.workspaceId !== workspaceId) {
        throw new Error("Saved project evidence did not match the active workspace");
      }
      setReport(cached);
      setInspectionPhase(cached ? "ready" : "idle");
      if (cached) setLastActivity("Reused the approved inspection from this Aone session; no new permission was requested");
    }).catch((failure: unknown) => {
      const current = workspaceRef.current;
      if (request !== inspectionRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration) return;
      setInspectionPhase("idle");
      setError(safeFailureMessage(failure, "Could not restore the saved project inspection"));
    });
  }, [workspaceGeneration, workspaceId]);

  const plan = useMemo(
    () => report ? buildAgentSetupPlan(report, runProfiles, activeRunProfileId) : null,
    [activeRunProfileId, report, runProfiles],
  );
  const approvableCount = plan ? countApprovableAgentSteps(plan) : 0;
  const aiReady = aiStatus?.inferenceAvailable === true;

  const inspect = async () => {
    if (!workspaceId || disabled || inspectionPhase === "inspecting") return;
    const request = ++inspectionRequest.current;
    const target = { workspaceId, workspaceGeneration };
    const retainedReport = report;
    chatRequest.current += 1;
    setChatPhase("idle");
    setChatFailure(null);
    setInspectionPhase("inspecting");
    setError(null);
    setApprovalStepId(null);
    setLastActivity("Inspection requested; waiting for native permission");
    try {
      const next = await inspectProjectEnvironment(workspaceId);
      const current = workspaceRef.current;
      if (request !== inspectionRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration) return;
      if (!next) {
        setInspectionPhase(retainedReport ? "ready" : "cancelled");
        setLastActivity(retainedReport
          ? "Re-inspection cancelled; the previous approved report remains available"
          : "Inspection cancelled; no project metadata was retained");
        return;
      }
      if (next.workspaceId !== workspaceId) {
        throw new Error("Native setup evidence did not match the active workspace");
      }
      setReport(next);
      setInspectionPhase("ready");
      setChatMessages([]);
      setChatFailure(null);
      setLastQuestion(null);
      setLastActivity(`Inspection ready: ${next.stacks.length} stacks, ${next.tools.length} local tools`);
    } catch (failure) {
      const current = workspaceRef.current;
      if (request === inspectionRequest.current
        && current.workspaceId === target.workspaceId
        && current.workspaceGeneration === target.workspaceGeneration) {
        setInspectionPhase(retainedReport ? "ready" : "idle");
        setError(safeFailureMessage(failure, "Could not inspect this workspace"));
      }
    }
  };

  const ask = async (rawQuestion: string, appendEngineerMessage = true) => {
    const question = rawQuestion.trim();
    if (!workspaceId || !report || !aiReady || disabled || chatPhase === "thinking" || !question) return;
    const request = ++chatRequest.current;
    const target = { workspaceId, workspaceGeneration, reportId: report.reportId };
    if (appendEngineerMessage) {
      setChatMessages((messages) => [...messages, { id: ++messageId.current, role: "engineer", body: question }]);
    }
    setLastQuestion(question);
    setChatPhase("thinking");
    setChatFailure(null);
    setLastActivity("Project question ready; waiting for provider consent");
    try {
      const response = await askProjectAgent({ workspaceId, reportId: report.reportId, question });
      const current = workspaceRef.current;
      if (request !== chatRequest.current
        || current.workspaceId !== target.workspaceId
        || current.workspaceGeneration !== target.workspaceGeneration
        || report.reportId !== target.reportId) return;
      if (!completedResponse(response)) {
        setChatPhase("error");
        setChatFailure({ message: response.message, retryable: response.retryable });
        setLastActivity(response.code === "cancelled" ? "Provider request cancelled; nothing was sent" : "Project Agent request did not complete");
        return;
      }
      setChatMessages((messages) => [...messages, {
        id: ++messageId.current,
        role: "agent",
        body: response.answer,
        model: response.model,
        evidence: response.evidence,
      }]);
      setChatDraft("");
      setChatPhase("idle");
      setLastActivity("Project Agent answered from the approved project report");
    } catch (failure) {
      const current = workspaceRef.current;
      if (request === chatRequest.current
        && current.workspaceId === target.workspaceId
        && current.workspaceGeneration === target.workspaceGeneration) {
        setChatPhase("error");
        setChatFailure({
          message: safeFailureMessage(failure, "The connected agent could not answer this project question"),
          retryable: true,
        });
        setLastActivity("Project Agent request failed; your question is still available");
      }
    }
  };

  const approveProfile = (step: AgentSetupPlanStep) => {
    const action = step.action;
    if (!action || action.kind !== "select-run-profile") return;
    if (!runProfiles.some((profile) => profile.id === action.profileId)) {
      setError("That run profile is no longer registered. Inspect the workspace again.");
      return;
    }
    onUseRunProfile(action.profileId);
    setApprovalStepId(null);
    setLastActivity(`Profile selected: ${action.profileName}. Run or Debug is still your explicit start decision.`);
  };

  if (!workspaceId) {
    return (
      <aside className="side-feature-panel agent-setup-panel" aria-label="Project Agent">
        <WorkbenchSidebarHeader title="Project Agent" />
        <WorkbenchSidebarSection title="Readiness · plan · chat" detail="No workspace" />
        <div className="side-feature-empty"><FolderOpen size={22} /><strong>Open a folder first</strong><span>Project Agent scopes every report, plan, and question to the active workspace.</span></div>
      </aside>
    );
  }

  return (
    <aside className="side-feature-panel agent-setup-panel" aria-label="Project Agent">
      <WorkbenchSidebarHeader title="Project Agent" />
      <WorkbenchSidebarSection
        title="Readiness · plan · chat"
        detail={inspectionPhase === "restoring" ? "Restoring" : inspectionPhase === "inspecting" ? "Inspecting" : report ? "Ready" : "Needs inspection"}
        count={plan?.steps.length}
      />

      <div className="agent-setup-scroll">
        <section className="agent-setup-connection" aria-labelledby="agent-setup-connection-title">
          <Robot size={17} weight="duotone" aria-hidden="true" />
          <div>
            <strong id="agent-setup-connection-title">{agentLabel(aiStatus)}</strong>
            <span>Senior engineering guidance · provider consent for every AI request</span>
          </div>
          {aiReady
            ? <span className="agent-connection-state is-ready">Connected</span>
            : <button type="button" className="agent-connection-action" onClick={onConfigureAi}>Configure AI</button>}
        </section>

        <section className="agent-responsibility-grid" aria-label="Project Agent responsibility boundary">
          <div><strong>Aone can</strong><span>Restore approved evidence, explain setup, and prepare a registered IDE profile.</span></div>
          <div><strong>You decide</strong><span>Environment files, installs, Docker, services, and the final Run or Debug.</span></div>
        </section>

        {inspectionPhase === "restoring" && !report ? (
          <section className="agent-setup-start is-restoring" role="status" aria-busy="true">
            <SpinnerGap className="spin" size={20} aria-hidden="true" />
            <strong>Checking this session</strong>
            <p>Aone is looking for an inspection you already approved. This check does not open a permission dialog.</p>
          </section>
        ) : !report ? (
          <section className="agent-setup-start">
            <ShieldCheck size={24} weight="duotone" aria-hidden="true" />
            <strong>Inspect once, then keep working</strong>
            <p>Approve metadata inspection and fixed version probes once. Closing and reopening Project Agent reuses that report; only <strong>Inspect again</strong> requests permission again.</p>
            <ol className="agent-readiness-timeline" aria-label="Project readiness">
              <li className="is-complete"><CheckCircle size={13} weight="fill" aria-hidden="true" /><span><strong>Folder opened</strong><small>Indexed project context is available.</small></span></li>
              <li><span className="agent-timeline-index">2</span><span><strong>Local readiness</strong><small>Tools and declared setup still need approval.</small></span></li>
              <li className={aiReady ? "is-complete" : ""}>{aiReady ? <CheckCircle size={13} weight="fill" aria-hidden="true" /> : <span className="agent-timeline-index">3</span>}<span><strong>Project guidance</strong><small>{aiReady ? `${agentLabel(aiStatus)} is ready.` : "Connect a supported provider first."}</small></span></li>
            </ol>
            {inspectionPhase === "cancelled" && <div className="agent-setup-notice" role="status">Inspection cancelled. Nothing was read or changed.</div>}
            {error && <div className="side-feature-error" role="alert">{error}</div>}
            <button type="button" className="primary-button" onClick={() => void inspect()} disabled={disabled || inspectionPhase === "inspecting"}>
              {inspectionPhase === "inspecting" ? <SpinnerGap className="spin" size={14} aria-hidden="true" /> : <ShieldCheck size={14} aria-hidden="true" />}
              {inspectionPhase === "inspecting" ? "Inspecting project…" : "Inspect project environment"}
            </button>
          </section>
        ) : (
          <>
            <section className="agent-setup-evidence" aria-labelledby="agent-setup-evidence-title">
              <header>
                <CheckCircle size={14} weight="fill" aria-hidden="true" />
                <strong id="agent-setup-evidence-title">Project readiness</strong>
                <span className="agent-report-state">Saved this session</span>
              </header>
              <div className="agent-evidence-metrics">
                <span>{plan?.declaredStackCount ?? 0}<small>stacks</small></span>
                <span>{plan?.detectedLocalToolCount ?? 0}<small>tools ready</small></span>
                <span>{report.versionProbeApproved ? "Yes" : "No"}<small>versions checked</small></span>
              </div>
              <div className="agent-stack-summary" aria-label="Detected project stacks">
                {report.stacks.slice(0, 4).map((stack) => <span key={stack.id}>{stack.label}<small>{stack.confidence}</small></span>)}
              </div>
              <button type="button" className="agent-inline-action" onClick={() => void inspect()} disabled={disabled || inspectionPhase === "inspecting"}>
                {inspectionPhase === "inspecting" ? <SpinnerGap className="spin" size={12} aria-hidden="true" /> : <ArrowClockwise size={12} aria-hidden="true" />}
                {inspectionPhase === "inspecting" ? "Waiting for permission…" : "Inspect again"}
              </button>
            </section>

            <section className="agent-setup-plan" aria-labelledby="agent-setup-plan-title">
              <header>
                <ArrowRight size={14} aria-hidden="true" />
                <strong id="agent-setup-plan-title">Recommended path</strong>
                <small>{approvableCount} IDE {approvableCount === 1 ? "change" : "changes"}</small>
              </header>
              {plan?.steps.length === 0 && <p className="agent-setup-empty-plan">No setup changes are currently recommended. Ask Project Agent about the next engineering task below.</p>}
              <ol className="agent-plan-list">
                {plan?.steps.map((step, index) => (
                  <li className={`agent-setup-step state-${step.state}`} key={step.id}>
                    <div className="agent-step-heading">
                      <span className="agent-step-index">{index + 1}</span>
                      <strong>{step.title}</strong>
                      <span>{stepStateLabel(step)}</span>
                    </div>
                    <p>{step.summary}</p>
                    {step.evidence.length > 0 && <div className="agent-step-evidence">{step.evidence.map((item) => <code key={item}>{item}</code>)}</div>}
                    {step.state === "awaiting-approval" && step.action && (
                      approvalStepId === step.id ? (
                        <div className="agent-step-approval" role="group" aria-label={`Approve ${step.title}`}>
                          <p>Select <strong>{step.action.profileName}</strong> in the IDE. No process, installer, service, or container starts.</p>
                          <div><button type="button" onClick={() => setApprovalStepId(null)}>Cancel</button><button type="button" className="primary-button" onClick={() => approveProfile(step)}>Approve profile</button></div>
                        </div>
                      ) : <button type="button" className="agent-inline-action" onClick={() => setApprovalStepId(step.id)}>Review IDE change</button>
                    )}
                  </li>
                ))}
              </ol>
            </section>

            <section className="agent-project-chat" aria-labelledby="agent-project-chat-title" aria-busy={chatPhase === "thinking"}>
              <header>
                <ChatCenteredDots size={14} aria-hidden="true" />
                <strong id="agent-project-chat-title">Ask about this project</strong>
                <small>{agentLabel(aiStatus)}</small>
              </header>
              <p className="agent-chat-contract">Each question uses this saved report. A native consent dialog shows the provider and bounded evidence before sending.</p>

              {chatMessages.length === 0 && chatPhase !== "thinking" && (
                <div className="agent-chat-empty">
                  <Robot size={18} weight="duotone" aria-hidden="true" />
                  <span>Ask for setup, environment, dependency, Docker, run, or debugging guidance.</span>
                </div>
              )}

              {chatMessages.length > 0 && (
                <ol className="agent-chat-log" aria-label="Project Agent conversation">
                  {chatMessages.map((message) => (
                    <li className={`is-${message.role}`} key={message.id}>
                      <div>{message.role === "engineer" ? <UserCircle size={13} aria-hidden="true" /> : <Robot size={13} aria-hidden="true" />}<strong>{message.role === "engineer" ? "You" : "Project Agent"}</strong>{message.model && <small>Inferred · {message.model}</small>}</div>
                      <p>{message.body}</p>
                      {message.evidence && message.evidence.length > 0 && (
                        <div className="agent-chat-evidence" aria-label="Answer evidence">
                          {message.evidence.slice(0, 4).map((item) => <span key={`${item.kind}:${item.id}`}>{item.label} · {item.evidence}</span>)}
                        </div>
                      )}
                    </li>
                  ))}
                </ol>
              )}

              {chatPhase === "thinking" && (
                <div className="agent-chat-thinking" role="status"><SpinnerGap className="spin" size={13} aria-hidden="true" /><span><strong>Project Agent is thinking</strong><small>Waiting for consent or provider response…</small></span></div>
              )}
              {chatFailure && (
                <div id="project-agent-chat-error" className="agent-chat-error" role="alert"><Warning size={13} aria-hidden="true" /><span>{chatFailure.message}</span>{chatFailure.retryable && lastQuestion && <button type="button" onClick={() => void ask(lastQuestion, false)}>Retry</button>}</div>
              )}

              <div className="agent-chat-prompts" aria-label="Suggested project questions">
                {QUICK_QUESTIONS.map((question) => <button type="button" key={question} onClick={() => void ask(question)} disabled={!aiReady || disabled || chatPhase === "thinking"}>{question}</button>)}
              </div>

              <form className="agent-chat-form" onSubmit={(event) => { event.preventDefault(); void ask(chatDraft); }}>
                <label className="visually-hidden" htmlFor="project-agent-question">Question for Project Agent</label>
                <textarea
                  id="project-agent-question"
                  name="projectAgentQuestion"
                  value={chatDraft}
                  maxLength={1200}
                  rows={2}
                  placeholder={aiReady ? "Ask how to run, configure, or debug this project…" : "Connect a supported AI provider to ask questions"}
                  aria-describedby={`project-agent-question-help${chatFailure ? " project-agent-chat-error" : ""}`}
                  aria-invalid={chatFailure ? true : undefined}
                  disabled={!aiReady || disabled || chatPhase === "thinking"}
                  onChange={(event) => setChatDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                      event.preventDefault();
                      void ask(chatDraft);
                    }
                  }}
                />
                <button type="submit" className="primary-button" disabled={!aiReady || disabled || chatPhase === "thinking" || !chatDraft.trim()}>
                  {chatPhase === "thinking" ? <SpinnerGap className="spin" size={13} aria-hidden="true" /> : <ArrowRight size={13} aria-hidden="true" />}
                  {chatPhase === "thinking" ? "Sending…" : "Ask"}
                </button>
                <small id="project-agent-question-help">⌘/Ctrl+Enter to send · 1,200 characters max · no secret values</small>
              </form>
            </section>
          </>
        )}

        {error && report && <div className="side-feature-error" role="alert">{error}</div>}
        {lastActivity && <div className="agent-setup-last-action" role="status"><strong>Latest</strong><span>{lastActivity}</span></div>}
        <p className="agent-setup-boundary"><ShieldCheck size={12} aria-hidden="true" /> AI guidance stays advisory. Aone currently applies only a registered run-profile selection after approval; it does not install packages, write environment values, start Docker, or launch dependent services from chat.</p>
      </div>
    </aside>
  );
}
