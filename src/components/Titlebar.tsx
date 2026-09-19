import {
  ArrowClockwise,
  CaretDown,
  Code,
  Key,
  MagnifyingGlass,
  Play,
  SidebarSimple,
  Stop,
} from "@phosphor-icons/react";
import type { RunProfile, ScanProgress, WorkspaceSummary } from "../types";

interface TitlebarProps {
  workspace: WorkspaceSummary | null;
  runProfiles: RunProfile[];
  activeProfileId: string;
  onProfileChange: (profileId: string) => void;
  onScan: () => void;
  onRun: () => void;
  onDebug: () => void;
  onStop: () => void;
  onEnvToggle: () => void;
  scanProgress: ScanProgress | null;
  runMode: "run" | "observe" | "debug" | null;
  stopping?: boolean;
  aiEnvCount: number;
  runEnvCount: number;
  workspaceBusy?: boolean;
  locked?: boolean;
  primarySidebarVisible: boolean;
  secondarySidebarVisible: boolean;
  onCommandCenter: () => void;
  onTogglePrimarySidebar: () => void;
  onToggleSecondarySidebar: () => void;
}

interface ToolbarButtonProps {
  label: string;
  title?: string;
  icon: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  active?: boolean;
  tone?: "normal" | "accent" | "danger";
}

function ToolbarButton({ label, title, icon, onClick, disabled, active, tone = "normal" }: ToolbarButtonProps) {
  return (
    <button
      type="button"
      className={`toolbar-button toolbar-${tone}${active ? " is-active" : ""}`}
      onClick={onClick}
      disabled={disabled}
      title={title ?? label}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}

export function Titlebar({
  workspace,
  runProfiles,
  activeProfileId,
  onProfileChange,
  onScan,
  onRun,
  onDebug,
  onStop,
  onEnvToggle,
  scanProgress,
  runMode,
  stopping = false,
  aiEnvCount,
  runEnvCount,
  workspaceBusy = false,
  locked = false,
  primarySidebarVisible,
  secondarySidebarVisible,
  onCommandCenter,
  onTogglePrimarySidebar,
  onToggleSecondarySidebar,
}: TitlebarProps) {
  const scanning = scanProgress !== null;
  const running = runMode !== null;
  const desktop = typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;

  return (
    <header
      className={`titlebar${locked ? " is-locked" : ""}${workspace ? "" : " is-empty"}`}
      data-tauri-drag-region
    >
      <div
        className={`traffic-lights${desktop ? " is-native" : ""}`}
        aria-hidden="true"
        data-tauri-drag-region
      >
        {!desktop && (
          <>
            <span className="traffic-light traffic-red" />
            <span className="traffic-light traffic-yellow" />
            <span className="traffic-light traffic-green" />
          </>
        )}
      </div>

      {locked ? (
        <div className="titlebar-command-center is-locked" data-tauri-drag-region>
          <MagnifyingGlass size={13} />
          <span>Configure AI to continue</span>
        </div>
      ) : (
        <button
          type="button"
          className="titlebar-command-center"
          onClick={onCommandCenter}
          aria-label={workspace ? `Quick open files in ${workspace.name}` : "Open a file or folder"}
          title={workspace ? "Quick open files (Command P)" : "Open a file or folder"}
        >
          <MagnifyingGlass size={13} aria-hidden="true" />
          <span>{workspace?.name ?? "Open a file or folder"}</span>
          {workspace && <kbd aria-hidden="true">⌘P</kbd>}
        </button>
      )}

      {!locked && workspace && (
        <>
          <nav className="toolbar-actions" aria-label="Workspace actions">
            <ToolbarButton
              label="Scan"
              title={scanning
                ? scanProgress.total === 0
                  ? scanProgress.phase.toLowerCase() === "committing"
                    ? "Committing graph and search index"
                    : `Indexing ${scanProgress.phase}`
                  : `Indexing ${scanProgress.phase}: ${scanProgress.completed}/${scanProgress.total}`
                : "Scan workspace"}
              icon={<ArrowClockwise className={scanning ? "spin" : ""} size={14} weight="bold" />}
              onClick={onScan}
              disabled={scanning || workspaceBusy}
            />
          </nav>

          <div className="profile-select-wrap">
            <select
              aria-label="Run profile"
              value={activeProfileId}
              onChange={(event) => onProfileChange(event.target.value)}
              disabled={running || workspaceBusy || runProfiles.length === 0}
            >
              {runProfiles.map((profile) => (
                <option key={profile.id} value={profile.id}>{profile.name}</option>
              ))}
            </select>
            <CaretDown size={11} aria-hidden="true" />
          </div>

          <div className="run-actions" aria-label="Execution controls">
            <ToolbarButton
              label="Run"
              title="Run selected profile (Command R)"
              icon={<Play size={14} weight="fill" />}
              onClick={onRun}
              disabled={running || workspaceBusy}
              active={runMode === "run"}
              tone="accent"
            />
            <ToolbarButton
              label="Debug"
              title="Start cooperative AONE_DEBUG_V1 safe-point debugging. The child application must be instrumented."
              icon={<Code size={14} weight="bold" />}
              onClick={onDebug}
              disabled={running || workspaceBusy}
              active={runMode === "debug"}
            />
            <ToolbarButton
              label={stopping ? "Stopping" : "Stop"}
              title={stopping ? "Waiting for managed process exit" : "Request managed process stop"}
              icon={<Stop size={14} weight="fill" />}
              onClick={onStop}
              disabled={!running || stopping}
              tone="danger"
            />
          </div>

          <button
            type="button"
            className="env-trigger"
            onClick={onEnvToggle}
            disabled={workspaceBusy}
            aria-label={`Environment settings (${aiEnvCount} AI, ${runEnvCount} run)`}
          >
            <Key size={14} weight="bold" />
            <span>Env</span>
            <span className="env-count">{aiEnvCount + runEnvCount}</span>
          </button>

          <nav className="titlebar-layout-controls" aria-label="Workbench layout">
            <button
              type="button"
              className="layout-control"
              onClick={onTogglePrimarySidebar}
              aria-label="Toggle primary sidebar"
              aria-pressed={primarySidebarVisible}
              title="Toggle primary sidebar (Command B)"
            >
              <SidebarSimple size={15} weight={primarySidebarVisible ? "fill" : "regular"} aria-hidden="true" />
            </button>
            <button
              type="button"
              className="layout-control is-secondary"
              onClick={onToggleSecondarySidebar}
              aria-label="Toggle secondary sidebar"
              aria-pressed={secondarySidebarVisible}
              title="Toggle secondary sidebar"
            >
              <SidebarSimple size={15} weight={secondarySidebarVisible ? "fill" : "regular"} aria-hidden="true" />
            </button>
          </nav>
        </>
      )}
    </header>
  );
}
