import { ArrowClockwise, Code, FilePlus, PlugsConnected, ShieldCheck } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "../../components/WorkbenchSidebarChrome";
import {
  inspectToolConfigurations,
  listToolConfigurations,
  openToolConfiguration,
} from "../../lib/devtoolsBridge";
import type { ToolConfiguration, ToolInspectionState } from "./model";

interface ToolConfigPanelProps {
  workspaceId?: string;
  disabled?: boolean;
}

function inspectionLabel(state: ToolInspectionState, subject: "configuration" | "CLI") {
  if (state === "notInspected") return `${subject === "CLI" ? "CLI" : "File"} not inspected`;
  if (state === "found") return subject === "CLI" ? "CLI found" : "Configured";
  return subject === "CLI" ? "CLI not found" : "Not created";
}

function failureMessage(failure: unknown, fallback: string) {
  return failure instanceof Error && failure.message.trim()
    ? failure.message
    : typeof failure === "string" && failure.trim() ? failure : fallback;
}

export function ToolConfigPanel({ workspaceId, disabled = false }: ToolConfigPanelProps) {
  const [tools, setTools] = useState<ToolConfiguration[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [catalogReady, setCatalogReady] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const workspaceRef = useRef(workspaceId);
  const disabledRef = useRef(disabled);
  const inspectingRef = useRef(false);
  const openingRef = useRef(false);
  const catalogGeneration = useRef(0);
  const inspectionGeneration = useRef(0);
  const openGeneration = useRef(0);
  workspaceRef.current = workspaceId;
  disabledRef.current = disabled;

  useEffect(() => {
    const targetWorkspace = workspaceId;
    const generation = ++catalogGeneration.current;
    inspectionGeneration.current += 1;
    openGeneration.current += 1;
    setTools([]);
    setCatalogReady(false);
    setInspecting(inspectingRef.current);
    setError(null);
    void listToolConfigurations()
      .then((catalog) => {
        if (generation === catalogGeneration.current && workspaceRef.current === targetWorkspace) {
          setTools(catalog);
          setCatalogReady(true);
        }
      })
      .catch((failure) => {
        if (generation === catalogGeneration.current && workspaceRef.current === targetWorkspace) {
          setError(failureMessage(failure, "Could not load the static tool catalog"));
        }
      });
  }, [workspaceId]);

  const inspect = async () => {
    if (disabledRef.current || inspectingRef.current || !catalogReady) return;
    const targetWorkspace = workspaceId;
    const generation = ++inspectionGeneration.current;
    inspectingRef.current = true;
    setInspecting(true);
    setError(null);
    try {
      const inspected = await inspectToolConfigurations();
      if (generation === inspectionGeneration.current && workspaceRef.current === targetWorkspace) {
        setTools(inspected);
      }
    } catch (failure) {
      if (generation === inspectionGeneration.current && workspaceRef.current === targetWorkspace) {
        setError(failureMessage(failure, "Could not inspect AI tools"));
      }
    } finally {
      inspectingRef.current = false;
      setInspecting(false);
    }
  };

  const open = async (tool: ToolConfiguration) => {
    const workspaceUnavailable = tool.scope === "workspace" && !workspaceId;
    if (disabledRef.current || openingRef.current || workspaceUnavailable
      || tool.configurationState === "notInspected") return;
    const targetWorkspace = workspaceId;
    const generation = ++openGeneration.current;
    openingRef.current = true;
    setBusyId(tool.id);
    setError(null);
    try {
      const result = await openToolConfiguration({
        toolId: tool.id,
        createIfMissing: tool.configurationState === "notFound",
      });
      if (generation !== openGeneration.current || workspaceRef.current !== targetWorkspace) return;
      if (result.opened) {
        setTools((current) => current.map((item) => item.id === tool.id
          ? { ...item, configurationState: "found" }
          : item));
      }
    } catch (failure) {
      if (generation === openGeneration.current && workspaceRef.current === targetWorkspace) {
        setError(failureMessage(failure, "Could not open configuration"));
      }
    } finally {
      openingRef.current = false;
      setBusyId(null);
    }
  };

  const inspected = tools.length > 0
    && tools.every((tool) => tool.configurationState !== "notInspected");

  return (
    <aside className="side-feature-panel tool-config-panel" aria-label="AI tool configuration">
      <WorkbenchSidebarHeader
        title="MCP and CLI"
        actions={<button type="button" onClick={() => void inspect()} aria-label={inspected ? "Inspect tool metadata again" : "Inspect tool metadata"} disabled={disabled || inspecting || !catalogReady}>
          {inspected ? <ArrowClockwise size={14} /> : <ShieldCheck size={14} />}
        </button>}
      />
      <WorkbenchSidebarSection
        title="Developer tools"
        detail={inspecting ? "Inspecting" : inspected ? "Inspected" : "Local configuration"}
        count={tools.length || undefined}
      />
      <div className="tool-config-consent">
        <p>Files, PATH, and installed apps are not inspected automatically. Contents and secrets never enter Aone.</p>
        <button type="button" className="secondary-button" onClick={() => void inspect()} disabled={disabled || inspecting || !catalogReady}>
          <ShieldCheck size={13} />{inspecting ? "Awaiting permission…" : inspected ? "Inspect again" : "Inspect AI tools"}
        </button>
      </div>
      {error && <div className="side-feature-error" role="alert">{error}</div>}
      <div className="tool-config-list" aria-busy={inspecting}>
        {tools.map((tool) => {
          const found = tool.configurationState === "found";
          const workspaceUnavailable = tool.scope === "workspace" && !workspaceId;
          const needsInspection = tool.configurationState === "notInspected";
          const actionLabel = workspaceUnavailable
            ? "Open a folder first"
            : needsInspection ? "Inspect first" : found ? "Open config" : "Create template";
          const ariaLabel = workspaceUnavailable
            ? `Open a folder before configuring ${tool.label}`
            : needsInspection
              ? `Inspect before configuring ${tool.label}`
              : `${found ? "Open config for" : "Create template for"} ${tool.label}`;
          return (
            <article key={tool.id} className="tool-config-card">
              <div className="tool-config-icon">{tool.kind === "mcp" ? <PlugsConnected size={17} /> : <Code size={17} />}</div>
              <div>
                <strong>{tool.label}</strong>
                <span>{tool.company} · {tool.scope}</span>
                <code title={tool.pathHint}>{tool.pathHint}</code>
              </div>
              <span className={`tool-config-kind kind-${tool.kind}`}>{tool.kind}</span>
              <div className="tool-config-state">
                <i className={found ? "is-ready" : needsInspection ? "is-unknown" : ""} />
                {inspectionLabel(tool.configurationState, "configuration")}
                <small>{inspectionLabel(tool.cliState, "CLI")}</small>
              </div>
              <button type="button" className="secondary-button" aria-label={ariaLabel} disabled={disabled || busyId !== null || workspaceUnavailable || needsInspection} onClick={() => void open(tool)}>
                {found ? <Code size={13} /> : <FilePlus size={13} />}
                {busyId === tool.id ? "Opening" : actionLabel}
              </button>
            </article>
          );
        })}
      </div>
    </aside>
  );
}
