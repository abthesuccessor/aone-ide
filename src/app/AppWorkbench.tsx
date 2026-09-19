import { Bug, Code, FileArrowUp, FolderOpen, GitBranch, Graph } from "@phosphor-icons/react";
import { lazy, Suspense } from "react";
import { WorkbenchTabs } from "../features/editor/WorkbenchTabs";
import type { useAppController } from "./useAppController";
import type { useDeveloperWorkbench } from "./useDeveloperWorkbench";
import { StageLoading } from "./AppLoadingStates";

const LazySourceView = lazy(async () => {
  const module = await import("../components/SourceView");
  return { default: module.SourceView };
});
const LazyCodeMapWorkbench = lazy(async () => {
  const module = await import("../features/code-knowledge/CodeMapWorkbench");
  return { default: module.CodeMapWorkbench };
});
const LazyExecutionDebugger = lazy(async () => {
  const module = await import("../features/execution-debugger/ExecutionDebugger");
  return { default: module.ExecutionDebugger };
});
const LazySourceComparisonView = lazy(async () => {
  const module = await import("../features/source-control/SourceComparisonView");
  return { default: module.SourceComparisonView };
});

type Controller = ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;

function EmptyWorkbenchHome({ controller }: { controller: Controller }) {
  const openView = (view: "graph" | "debugger" | "source-control") => {
    controller.setActivityView(view);
    controller.setPrimarySidebarVisible(true);
    controller.setMainView(view === "debugger" ? "debugger" : "graph");
  };

  return (
    <div className="welcome-editor" aria-label="No workspace open">
      <div className="welcome-editor-tabs" role="tablist" aria-label="Open editors">
        <div role="tab" aria-selected="true"><Code size={14} weight="duotone" /><span>Welcome</span></div>
      </div>
      <div className="welcome-editor-body">
        <div className="welcome-layout">
          <section className="welcome-start">
            <h1>Aone IDE</h1>
            <p>Code relationships, evolved</p>
            <h2>Start</h2>
            <button type="button" onClick={() => void controller.handleOpenFile()}><FileArrowUp size={18} />New or Open File...</button>
            <button type="button" onClick={() => void controller.handleOpenFolder()}><FolderOpen size={18} />Open Folder...</button>
            <h2>Workspace</h2>
            <span>Open a local project to index source, render relationships, and inspect evidence.</span>
          </section>
          <section className="welcome-walkthroughs">
            <h2>Enhanced Workbench</h2>
            <button type="button" onClick={() => openView("graph")}>
              <Graph size={18} />
              <span><strong>Explore the Code Graph</strong><small>Map source, dependencies, API paths, data, and Git changes.</small></span>
            </button>
            <button type="button" onClick={() => openView("debugger")}>
              <Bug size={18} />
              <span><strong>Trace Cooperative Debugging</strong><small>Replay evidence-backed safe points beside exact source.</small></span>
            </button>
            <button type="button" onClick={() => openView("source-control")}>
              <GitBranch size={18} />
              <span><strong>Review Local Changes</strong><small>Connect changed files to their upstream code relationships.</small></span>
            </button>
          </section>
        </div>
      </div>
    </div>
  );
}

function SourceEditor({
  controller,
  mapMode = false,
  focusOnReveal = true,
}: {
  controller: Controller;
  mapMode?: boolean;
  focusOnReveal?: boolean;
}) {
  if (controller.gitDiff) {
    return (
      <LazySourceComparisonView
        diff={controller.gitDiff}
        language={controller.editor.activeDocument?.language ?? "text"}
        settings={controller.settings}
        onClose={() => controller.setGitDiff(null)}
      />
    );
  }
  const location = controller.sourceLocation
    && controller.sourceLocation.relativePath === controller.editor.activeDocument?.relativePath
    ? controller.sourceLocation
    : controller.selectedSource?.relativePath === controller.editor.activeDocument?.relativePath
      ? (controller.selectedSource ?? undefined)
      : undefined;
  return (
    <LazySourceView
      source={controller.editor.activeDocument}
      location={location}
      loading={controller.sourceLoading}
      error={controller.sourceError}
      editable
      workspaceBusy={controller.openingWorkspace}
      dirty={controller.editor.activeDocument?.dirty ?? false}
      saving={controller.editor.savingPath === controller.editor.activePath}
      formatting={controller.editor.formattingPath === controller.editor.activePath}
      settings={controller.settings}
      onChange={controller.editor.updateContent}
      onSave={() => void controller.editor.saveDocument()}
      onFormat={() => void controller.editor.formatActiveDocument()}
      compact={mapMode}
      focusOnReveal={focusOnReveal}
    />
  );
}

export function AppWorkbench({ controller }: { controller: Controller }) {
  const indexingLabel = controller.scanProgress
    ? `${controller.scanProgress.phase}${controller.scanProgress.total > 0
      ? ` ${controller.scanProgress.completed} of ${controller.scanProgress.total}`
      : ""}`
    : "Waiting for file or folder selection";
  const stage = (
    <div className="workbench-stage">
      {!controller.workspace ? (
        controller.openingWorkspace ? (
          <div className="empty-workbench indexing-workbench" role="status" aria-live="polite">
            <div className="graph-loader" aria-hidden="true" />
            <strong>{controller.scanProgress ? "Indexing workspace" : "Open a file or folder"}</strong>
            <span>{indexingLabel}</span>
            {controller.scanProgress?.currentPath && <small>{controller.scanProgress.currentPath}</small>}
          </div>
        ) : (
          <EmptyWorkbenchHome controller={controller} />
        )
      ) : controller.mainView === "debugger" ? (
        <section className="debugger-workbench-frame" aria-label="API trace workbench">
          <Suspense fallback={<StageLoading kind="debugger" />}>
            <LazyExecutionDebugger
              events={controller.runtimeEvents}
              runId={controller.debuggerSession.runId}
              observeActive={controller.activeRun?.mode === "debug"}
              processActive={controller.activeRun?.id === controller.debuggerSession.runId}
              session={controller.debuggerSession.session}
              loading={controller.debuggerSession.loading}
              error={controller.debuggerSession.error}
              onControl={controller.handleDebugControl}
              workspaceId={controller.workspace?.id}
              workspaceGeneration={controller.workspaceGeneration}
              onRuntimeEventsChanged={controller.refreshRuntimeEvents}
              onOpenSource={controller.openSourceLocationInDebugger}
              onSendRequest={controller.handleApiRequest}
              target={controller.apiTraceTarget}
              indexedGraph={controller.apiTraceTarget ? controller.relationshipGraph : undefined}
              indexedRootId={controller.flowRootId ?? undefined}
              indexedGraphLoading={controller.flowLoading}
              sourcePane={(
                <section className="trace-source-pane" aria-label="Trace source editor">
                  <WorkbenchTabs
                    documents={controller.editor.documents}
                    activePath={controller.editor.activePath}
                    onDocument={(relativePath) => {
                      controller.setGitDiff(null);
                      controller.editor.setActivePath(relativePath);
                      controller.setMainView("debugger");
                    }}
                    onPin={controller.editor.pinDocument}
                    onClose={(relativePath) => controller.editor.closeDocument(relativePath)}
                    closeDisabled={controller.openingWorkspace}
                  />
                  <Suspense fallback={<StageLoading kind="source" />}>
                    <SourceEditor controller={controller} mapMode focusOnReveal={false} />
                  </Suspense>
                </section>
              )}
            />
          </Suspense>
        </section>
      ) : (
        <Suspense fallback={<StageLoading kind="graph" />}>
          <LazyCodeMapWorkbench
            source={(
              <div className="source-pane-stack">
                <WorkbenchTabs
                  documents={controller.editor.documents}
                  activePath={controller.editor.activePath}
                  onDocument={(relativePath) => {
                    controller.setGitDiff(null);
                    controller.editor.setActivePath(relativePath);
                    controller.setMainView("graph");
                  }}
                  onPin={controller.editor.pinDocument}
                  onClose={(relativePath) => controller.editor.closeDocument(relativePath)}
                  closeDisabled={controller.openingWorkspace}
                />
                <Suspense fallback={<StageLoading kind="source" />}>
                  <SourceEditor controller={controller} mapMode />
                </Suspense>
              </div>
            )}
            graph={controller.relationshipGraph}
            selectedNodeId={controller.selectedNodeId}
            loading={controller.graphLoading || controller.scanningWorkspace || controller.phase === "loading"}
            error={controller.graphError}
            loadingMore={controller.flowLoading}
            relationshipView={controller.relationshipView}
            onRelationshipViewChange={(view) => {
              controller.setRelationshipView(view);
              if (view === "codepath" && controller.selectedNodeId) {
                void controller.traceNode(controller.selectedNodeId);
              }
            }}
            onSelectNode={controller.selectRelationshipNode}
            onOpenSource={controller.openRelationshipNode}
            onTraceNode={(nodeId) => {
              controller.setRelationshipView("codepath");
              void controller.traceNode(nodeId);
            }}
            onLoadMore={() => void controller.loadNextExecutionFlow()}
            graphSearchQuery={controller.graphSearchQuery}
            graphSearchLoading={controller.graphSearchLoading}
            onGraphSearch={controller.searchGraph}
            workspaceName={controller.workspace.name}
            applicationZoom={controller.applicationZoom}
          />
        </Suspense>
      )}
    </div>
  );

  return (
    <main className={`workbench${controller.workspace ? "" : " is-empty"}`}>{stage}</main>
  );
}
