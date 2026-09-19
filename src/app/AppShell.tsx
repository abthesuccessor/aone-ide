import { BracketsCurly, GitBranch, ShieldCheck, Warning } from "@phosphor-icons/react";
import { lazy, Suspense, useCallback } from "react";
import { ActivityBar } from "../components/ActivityBar";
import { EnvPopover } from "../components/EnvPopover";
import { Titlebar } from "../components/Titlebar";
import { Toast } from "../components/Toast";
import { isTauriRuntime } from "../lib/bridge";
import { SidebarLoading, StageLoading } from "./AppLoadingStates";
import type { useAppController } from "./useAppController";
import type { ActivityView, useDeveloperWorkbench } from "./useDeveloperWorkbench";
import { useRunIntentShortcuts } from "./useAppShortcuts";

const loadAppActivityPanel = () => import("./AppActivityPanel");

const LazyAppActivityPanel = lazy(async () => {
  const module = await loadAppActivityPanel();
  return { default: module.AppActivityPanel };
});

function preloadActivityPanel(view: ActivityView) {
  void loadAppActivityPanel()
    .then((module) => module.preloadActivityPanel(view))
    .catch(() => undefined);
}

const LazyAppWorkbench = lazy(async () => {
  const module = await import("./AppWorkbench");
  return { default: module.AppWorkbench };
});
const LazyAssistantSetupDialog = lazy(async () => {
  const module = await import("../components/AssistantSetupDialog");
  return { default: module.AssistantSetupDialog };
});
const LazyResizableIdeLayout = lazy(async () => {
  const module = await import("./ResizableIdeLayout");
  return { default: module.ResizableIdeLayout };
});
const LazyResizableWorkbenchBody = lazy(async () => {
  const module = await import("./ResizableWorkbenchBody");
  return { default: module.ResizableWorkbenchBody };
});
const LazyAppRuntimeConsole = lazy(async () => {
  const module = await import("./AppRuntimeConsole");
  return { default: module.AppRuntimeConsole };
});
const LazyWorkspaceContextRail = lazy(async () => {
  const module = await import("./WorkspaceContextRail");
  return { default: module.WorkspaceContextRail };
});
const LazySettingsPanel = lazy(async () => {
  const module = await import("../features/editor/SettingsPanel");
  return { default: module.SettingsPanel };
});

interface AppShellProps {
  controller: ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;
}

interface RunIntentController {
  activeProfileId: string;
  setPrimarySidebarVisible: (visible: boolean) => void;
  setActivityView: (view: "agent-setup") => void;
  setMainView: (view: "graph") => void;
  notify: (text: string, tone: "warning") => void;
  handleStart: (mode: "run" | "debug") => Promise<void>;
}

export function routeRunIntent(
  controller: RunIntentController,
  mode: "run" | "debug",
): void {
  if (controller.activeProfileId) {
    void controller.handleStart(mode);
    return;
  }

  controller.setPrimarySidebarVisible(true);
  controller.setActivityView("agent-setup");
  controller.setMainView("graph");
  controller.notify(
    "No runnable profile was detected. Review Project Agent before starting; Aone will not guess commands or start dependencies.",
    "warning",
  );
}

export function AppShell({ controller }: AppShellProps) {
  const handleRunIntent = useCallback(
    (mode: "run" | "debug") => routeRunIntent(controller, mode),
    [controller],
  );
  useRunIntentShortcuts(controller.activeRun, handleRunIntent);

  if (controller.phase === "error") {
    return (
      <main className="fatal-state">
        <div className="fatal-mark">A1</div>
        <Warning size={30} weight="duotone" />
        <h1>Aone could not start</h1>
        <p>{controller.appError}</p>
        <button type="button" className="primary-button" onClick={() => void controller.initialize()}>Try again</button>
      </main>
    );
  }

  const workspaceBusy = controller.phase === "loading"
    || controller.openingWorkspace
    || controller.scanningWorkspace;
  const configurationLocked = controller.aiConfiguration?.inferenceAvailable !== true
    && !controller.codeOnlyMode;
  const progressPercent = !controller.scanProgress || controller.scanProgress.total === 0
    ? undefined
    : Math.min(100, (controller.scanProgress.completed / Math.max(controller.scanProgress.total, 1)) * 100);
  const progressPhase = controller.scanProgress?.phase.toLowerCase() === "committing"
    ? "committing graph and search index"
    : controller.scanProgress?.phase ?? "workspace";
  const progressLabel = controller.openingWorkspace
    ? `Opening workspace: ${progressPhase}`
    : `Indexing workspace: ${progressPhase}`;

  const stage = (
    <Suspense fallback={<StageLoading kind="graph" />}>
      <LazyAppWorkbench controller={controller} />
    </Suspense>
  );
  const debuggerFocused = controller.mainView === "debugger";
  const workbench = controller.workspace && !debuggerFocused ? (
    <Suspense fallback={<div className="ide-resizable-single">{stage}</div>}>
      <LazyResizableWorkbenchBody
        stage={stage}
        console={(
          <Suspense fallback={<div className="console-empty">Preparing local runtime tools...</div>}>
            <LazyAppRuntimeConsole controller={controller} />
          </Suspense>
        )}
        consoleCollapsed={controller.consoleCollapsed}
        onConsoleCollapsedChange={controller.setConsoleCollapsed}
      />
    </Suspense>
  ) : stage;
  // Keep the navigator keyed independently from the expensive workbench so an
  // activity change replaces only the sidebar content.
  const navigator = () => (
    <Suspense fallback={<SidebarLoading label="Activity view" />}>
      <LazyAppActivityPanel key={controller.activityView} controller={controller} />
    </Suspense>
  );
  const inspector = controller.workspace && controller.secondarySidebarVisible ? (
    <Suspense fallback={<div className="workspace-context-rail" aria-busy="true" />}>
      <LazyWorkspaceContextRail controller={controller} />
    </Suspense>
  ) : undefined;

  return (
    <div className={`app-shell${configurationLocked ? " is-configuration-locked" : ""}${controller.mainView === "debugger" ? " is-debugger" : ""}`} aria-busy={workspaceBusy}>
      <Titlebar
        workspace={configurationLocked ? null : controller.workspace}
        runProfiles={controller.runProfiles}
        activeProfileId={controller.activeProfileId}
        onProfileChange={controller.setActiveProfileId}
        onScan={() => void controller.handleScan()}
        onRun={() => routeRunIntent(controller, "run")}
        onDebug={() => routeRunIntent(controller, "debug")}
        onStop={() => void controller.handleStop()}
        onEnvToggle={() => controller.setEnvOpen((current) => !current)}
        scanProgress={controller.scanProgress}
        runMode={controller.activeRun?.mode ?? null}
        stopping={controller.activeRun?.stopping ?? false}
        aiEnvCount={controller.aiEnv?.names.length ?? 0}
        runEnvCount={controller.runEnv?.names.length ?? 0}
        workspaceBusy={workspaceBusy}
        locked={configurationLocked}
        primarySidebarVisible={controller.primarySidebarVisible}
        secondarySidebarVisible={controller.secondarySidebarVisible}
        onCommandCenter={() => {
          if (controller.workspace) controller.focusQuickFile();
          else void controller.handleOpenFolder();
        }}
        onTogglePrimarySidebar={() => controller.setPrimarySidebarVisible((visible) => !visible)}
        onToggleSecondarySidebar={() => controller.setSecondarySidebarVisible((visible) => !visible)}
      />

      {(controller.openingWorkspace || controller.scanningWorkspace || controller.scanProgress) && (
        <div
          className={`scan-progress${progressPercent === undefined ? " is-indeterminate" : ""}`}
          role="progressbar"
          aria-label={progressLabel}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={progressPercent === undefined ? undefined : Math.round(progressPercent)}
          aria-valuetext={controller.scanProgress
            ? controller.scanProgress.total === 0
              ? progressPhase
              : `${controller.scanProgress.completed} of ${controller.scanProgress.total}`
            : "Waiting for folder selection"}
        >
          <i aria-hidden="true" style={{ width: progressPercent === undefined ? "32%" : `${progressPercent}%` }} />
        </div>
      )}

      <EnvPopover
        open={!configurationLocked && controller.envOpen}
        aiEnv={controller.aiEnv}
        runEnv={controller.runEnv}
        aiLoading={controller.aiEnvLoading}
        runLoading={controller.runEnvLoading}
        onPickAi={() => void controller.handlePickAiEnv()}
        onPickRun={() => void controller.handlePickRunEnv()}
        onClose={() => controller.setEnvOpen(false)}
      />

      <div className="ide-grid" inert={configurationLocked ? true : undefined} aria-hidden={configurationLocked || undefined}>
        <ActivityBar
          active={controller.activityView}
          onIntent={preloadActivityPanel}
          onChange={(view) => {
            controller.setPrimarySidebarVisible(true);
            if (view === "graph") {
              controller.setActivityView("graph");
              controller.setRelationshipView("workspace");
              controller.setMainView("graph");
              return;
            }
            if (view === "debugger") {
              controller.setActivityView("debugger");
              controller.setMainView("debugger");
              controller.setConsoleCollapsed(true);
              return;
            }
            controller.setActivityView(view);
            if (view === "source-control") {
              controller.setRelationshipView("changes");
              controller.setMainView("graph");
            } else if (view === "explorer") {
              controller.setRelationshipView("workspace");
              controller.setMainView("source");
            } else {
              controller.setMainView("graph");
            }
          }}
          onSettings={() => controller.setSettingsOpen(true)}
        />
        <Suspense fallback={<div className="ide-resizable-single">{workbench}</div>}>
          <LazyResizableIdeLayout
            navigator={controller.primarySidebarVisible ? navigator : undefined}
            navigatorKey={controller.activityView}
            workbench={workbench}
            inspector={inspector}
            focus={debuggerFocused}
          />
        </Suspense>
      </div>

      <footer className="statusbar">
        <div className="statusbar-group" aria-label="Workspace status">
          <span className="statusbar-item status-connection"><i />{isTauriRuntime() ? "Desktop IPC" : "Browser demo"}</span>
          {controller.gitStatus?.isRepository && (
            <span className="statusbar-item" title="Current Git branch">
              <GitBranch size={13} aria-hidden="true" />
              {controller.gitStatus.branch ?? "Git repository"}
            </span>
          )}
          <span className="statusbar-item statusbar-workspace" title={controller.workspace?.rootPath}>
            {configurationLocked
              ? "Choose AI or code-only mode"
              : controller.workspace?.rootPath ?? "No workspace open"}
          </span>
        </div>
        <div className="statusbar-group statusbar-group-right" aria-label="Editor status">
          <span className="statusbar-item"><ShieldCheck size={13} aria-hidden="true" />Local only</span>
          <span className="statusbar-item">
            <BracketsCurly size={13} aria-hidden="true" />
            {controller.editor.activeDocument
              ? `${controller.editor.activeDocument.language} · ${controller.settings.wordWrap === "off" ? "No wrap" : `Wrap ${controller.settings.wordWrapColumn}`}`
              : controller.selectedNode?.source
                ? `${controller.selectedNode.source.relativePath}:${controller.selectedNode.source.startLine}`
                : "No selection"}
          </span>
        </div>
      </footer>

      {configurationLocked && (
        <Suspense fallback={null}>
          <LazyAssistantSetupDialog
            status={controller.aiConfiguration}
            loading={controller.aiEnvLoading}
            error={controller.aiConfigurationError}
            onConfigureHosted={controller.configureHostedAi}
            onImportEnvironment={controller.handlePickAiEnv}
            onConfigureOllama={controller.configureOllama}
            cliAdapters={controller.cliAdapters.length > 0 ? controller.cliAdapters : undefined}
            onRefreshCliAdapters={controller.refreshCliAdapters}
            onConnectCli={controller.configureCli}
            onContinueCodeOnly={() => controller.setCodeOnlyMode(true)}
          />
        </Suspense>
      )}

      {!configurationLocked && controller.settingsOpen && (
        <Suspense fallback={null}>
          <LazySettingsPanel open settings={controller.settings} onChange={controller.updateSettings} onReset={controller.resetSettings} onClose={() => controller.setSettingsOpen(false)} />
        </Suspense>
      )}
      <Toast message={controller.toast} onDismiss={() => controller.setToast(null)} />
    </div>
  );
}
