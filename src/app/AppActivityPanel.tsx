import { lazy, Suspense } from "react";
import { EmptyExplorer } from "../components/EmptyExplorer";
import { WorkspaceExplorer } from "../components/WorkspaceExplorer";
import { listApiEndpointsPage } from "../features/api-catalog/bridge";
import { SourceControlPanel } from "../features/source-control/SourceControlPanel";
import type { useAppController } from "./useAppController";
import type { ActivityView, useDeveloperWorkbench } from "./useDeveloperWorkbench";
import { SidebarLoading } from "./AppLoadingStates";

const loadSearchPanel = () => import("../features/search/SearchPanel");
const loadKnowledgeNavigator = () => import("../features/code-knowledge/KnowledgeNavigator");
const loadAgentSetupPanel = () => import("../features/agent-setup/AgentSetupPanel");
const loadToolConfigPanel = () => import("../features/tool-config/ToolConfigPanel");

const LazySearchPanel = lazy(async () => {
  const module = await loadSearchPanel();
  return { default: module.SearchPanel };
});
const LazyKnowledgeNavigator = lazy(async () => {
  const module = await loadKnowledgeNavigator();
  return { default: module.KnowledgeNavigator };
});
const LazyAgentSetupPanel = lazy(async () => {
  const module = await loadAgentSetupPanel();
  return { default: module.AgentSetupPanel };
});
const LazyToolConfigPanel = lazy(async () => {
  const module = await loadToolConfigPanel();
  return { default: module.ToolConfigPanel };
});

type Controller = ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;

export function preloadActivityPanel(view: ActivityView) {
  if (view === "search") void loadSearchPanel().catch(() => undefined);
  if (view === "graph") void loadKnowledgeNavigator().catch(() => undefined);
  if (view === "agent-setup") void loadAgentSetupPanel().catch(() => undefined);
  if (view === "tools") void loadToolConfigPanel().catch(() => undefined);
}

export function AppActivityPanel({ controller }: { controller: Controller }) {
  const workspaceBusy = controller.phase === "loading"
    || controller.openingWorkspace
    || controller.scanningWorkspace;
  if (controller.activityView === "graph") {
    return (
      <Suspense fallback={<SidebarLoading label="Code knowledge" />}>
        <LazyKnowledgeNavigator
          workspace={controller.workspace}
          workspaceGeneration={controller.workspaceGeneration}
          files={controller.files}
          activePath={controller.editor.activePath ?? undefined}
          graph={controller.relationshipGraph}
          selectedNodeId={controller.selectedNodeId}
          loading={controller.filesLoading || controller.graphLoading || workspaceBusy}
          disabled={workspaceBusy}
          loadPage={listApiEndpointsPage}
          onOpenFolder={() => void controller.handleOpenFolder()}
          onOpenFile={(file, mode = "preview") => {
            if (mode === "pinned") controller.editor.pinDocument(file.relativePath);
            void controller.openSource(file.relativePath, undefined, undefined, "graph");
          }}
          onOpenSource={controller.openSourceLocationInGraph}
          onSelectNode={controller.selectRelationshipNode}
          onTraceFlow={controller.openApiTrace}
        />
      </Suspense>
    );
  }
  if (controller.activityView === "explorer" || controller.activityView === "debugger") {
    const sidebarTitle = controller.activityView === "debugger" ? "Run and Debug" : "Explorer";
    const sectionTitle = controller.activityView === "debugger" ? "Workspace source" : undefined;
    const sectionDetail = controller.activityView === "debugger" ? "Safe-point sources" : undefined;
    if (!controller.workspace) {
      return (
        <EmptyExplorer
          busy={workspaceBusy}
          title={sidebarTitle}
          sectionTitle={sectionTitle ?? "No folder opened"}
          onOpenFile={() => void controller.handleOpenFile()}
          onOpenFolder={() => void controller.handleOpenFolder()}
        />
      );
    }
    return <WorkspaceExplorer
      workspace={controller.workspace}
      files={controller.files}
      activePath={controller.editor.activePath ?? undefined}
      loading={controller.filesLoading || controller.phase === "loading"}
      disabled={controller.openingWorkspace || controller.phase === "loading"}
      title={sidebarTitle}
      sectionTitle={sectionTitle}
      sectionDetail={sectionDetail}
      onOpenFolder={() => void controller.handleOpenFolder()}
      onOpenFile={(file, mode = "preview") => {
        if (mode === "pinned") controller.editor.pinDocument(file.relativePath);
        void controller.openSource(file.relativePath, undefined, undefined, "source");
      }}
    />;
  }
  if (controller.activityView === "search") {
    return (
      <Suspense fallback={<SidebarLoading label="Search" />}>
        <LazySearchPanel
          workspaceId={controller.workspace?.id}
          workspaceName={controller.workspace?.name}
          workspaceGeneration={controller.workspaceGeneration}
          disabled={workspaceBusy}
          onOpenMatch={controller.openSearchMatch}
          onOpenFolder={() => void controller.handleOpenFolder()}
        />
      </Suspense>
    );
  }
  if (controller.activityView === "source-control") {
    return (
      <SourceControlPanel
        workspaceId={controller.workspace?.id}
        disabled={controller.openingWorkspace}
        initialStatus={controller.gitStatus}
        selectedPath={controller.selectedGitPath}
        refreshToken={controller.catalogGeneration}
        onStatusChange={controller.setGitStatus}
        onOpenDiff={controller.openGitDiff}
        onOpenFile={controller.openGitChangedFile}
      />
    );
  }
  if (controller.activityView === "agent-setup") {
    return (
      <Suspense fallback={<SidebarLoading label="Project Agent" />}>
        <LazyAgentSetupPanel
          workspaceId={controller.workspace?.id}
          workspaceGeneration={controller.workspaceGeneration}
          disabled={controller.openingWorkspace}
          aiStatus={controller.aiConfiguration}
          runProfiles={controller.runProfiles.map(({ id, name }) => ({ id, name }))}
          activeRunProfileId={controller.activeProfileId}
          onConfigureAi={() => controller.setCodeOnlyMode(false)}
          onUseRunProfile={(profileId) => {
            const profile = controller.runProfiles.find((candidate) => candidate.id === profileId);
            if (!profile) return;
            controller.setActiveProfileId(profileId);
            controller.notify(`Project Agent selected ${profile.name}; no process was started`, "success");
          }}
        />
      </Suspense>
    );
  }
  return (
    <Suspense fallback={<SidebarLoading label="MCP and CLI configuration" />}>
      <LazyToolConfigPanel workspaceId={controller.workspace?.id} disabled={controller.openingWorkspace} />
    </Suspense>
  );
}
