import { Cpu, GitBranch, House, PlugsConnected, Sparkle, X } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import type { EnvLoadResult, WorkspaceSummary } from "../../types";
import { AiSetup } from "./AiSetup";
import { GitOnboarding } from "./GitOnboarding";
import type {
  CloneGithubRepositoryRequest,
  CreateDocumentsProjectRequest,
  OnboardingSection,
  WorkspaceActionOutcome,
} from "./model";
import { ToolsSetup } from "./ToolsSetup";
import { WorkspaceStart } from "./WorkspaceStart";

interface OnboardingPanelProps {
  workspace: WorkspaceSummary | null;
  workspaceGeneration: number;
  workspaceBusy: boolean;
  aiEnv: EnvLoadResult | null;
  aiEnvLoading: boolean;
  onOpenFolder: () => Promise<void>;
  onClone: (request: CloneGithubRepositoryRequest) => Promise<WorkspaceActionOutcome>;
  onCreate: (request: CreateDocumentsProjectRequest) => Promise<WorkspaceActionOutcome>;
  onPickAiEnv: () => Promise<void>;
  onOpenTools: () => void;
  onOpenProjectAgent: () => void;
  onExit: () => void;
}

const sections = [
  { id: "workspace" as const, label: "Workspace", icon: House },
  { id: "ai" as const, label: "AI Provider", icon: Sparkle },
  { id: "tools" as const, label: "AI Tools", icon: PlugsConnected },
  { id: "git" as const, label: "Git Workflow", icon: GitBranch },
];

export function OnboardingPanel(props: OnboardingPanelProps) {
  const [section, setSection] = useState<OnboardingSection>("workspace");
  const headingRef = useRef<HTMLHeadingElement>(null);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  useEffect(() => { headingRef.current?.focus(); }, []);

  const selectSection = (next: OnboardingSection) => setSection(next);
  const moveTab = (index: number) => {
    const next = (index + sections.length) % sections.length;
    selectSection(sections[next]!.id);
    window.requestAnimationFrame(() => tabRefs.current[next]?.focus());
  };

  return (
    <div
      className="onboarding-panel"
      aria-label="Getting Started"
      onKeyDown={(event) => {
        if (event.key === "Escape" && props.workspace && !event.defaultPrevented) {
          event.preventDefault();
          props.onExit();
        }
      }}
    >
      <header className="onboarding-header">
        <div>
          <span className="onboarding-kicker">Getting Started</span>
          <h1 ref={headingRef} tabIndex={-1}>Set up a local Aone workspace</h1>
          <p>Choose only the setup actions you need. Local inspection and AI access always wait for your request.</p>
        </div>
        {props.workspace && (
          <button type="button" className="onboarding-close" onClick={props.onExit} aria-label="Close Getting Started">
            <X size={15} />
          </button>
        )}
      </header>

      <div className="onboarding-body">
        <nav className="onboarding-nav" role="tablist" aria-label="Getting Started sections">
          {sections.map(({ id, label, icon: Icon }, index) => (
            <button
              key={id}
              ref={(node) => { tabRefs.current[index] = node; }}
              type="button"
              role="tab"
              id={`onboarding-tab-${id}`}
              aria-controls={`onboarding-panel-${id}`}
              aria-selected={section === id}
              tabIndex={section === id ? 0 : -1}
              className={section === id ? "is-active" : ""}
              onClick={() => selectSection(id)}
              onKeyDown={(event) => {
                if (event.key === "ArrowDown" || event.key === "ArrowRight") { event.preventDefault(); moveTab(index + 1); }
                if (event.key === "ArrowUp" || event.key === "ArrowLeft") { event.preventDefault(); moveTab(index - 1); }
                if (event.key === "Home") { event.preventDefault(); moveTab(0); }
                if (event.key === "End") { event.preventDefault(); moveTab(sections.length - 1); }
              }}
            >
              <Icon size={16} weight={section === id ? "fill" : "regular"} />
              <span>{label}</span>
              {id === "workspace" && props.workspace && <small>{props.workspace.name}</small>}
            </button>
          ))}
        </nav>

        <div
          className="onboarding-content"
          role="tabpanel"
          id={`onboarding-panel-${section}`}
          aria-labelledby={`onboarding-tab-${section}`}
        >
          {section === "workspace" ? (
            <WorkspaceStart
              workspace={props.workspace}
              busy={props.workspaceBusy}
              onOpenFolder={props.onOpenFolder}
              onClone={props.onClone}
              onCreate={props.onCreate}
              onOpenAi={() => selectSection("ai")}
              onOpenGit={() => selectSection("git")}
              onOpenProjectAgent={props.onOpenProjectAgent}
            />
          ) : section === "ai" ? (
            <AiSetup aiEnv={props.aiEnv} loading={props.aiEnvLoading} onPickEnv={props.onPickAiEnv} />
          ) : section === "tools" ? (
            <ToolsSetup onOpenTools={props.onOpenTools} onOpenProjectAgent={props.onOpenProjectAgent} />
          ) : (
            <GitOnboarding
              workspaceId={props.workspace?.id}
              workspaceGeneration={props.workspaceGeneration}
              disabled={props.workspaceBusy}
            />
          )}
        </div>
      </div>
      <footer className="onboarding-footer">
        <span><Cpu size={13} /> Local-first workspace setup</span>
        <span>Inspection starts only after a button press</span>
      </footer>
    </div>
  );
}
