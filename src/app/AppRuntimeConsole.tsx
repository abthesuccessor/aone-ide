import { RuntimeApiConsole } from "../components/RuntimeApiConsole";
import type { useAppController } from "./useAppController";
import type { useDeveloperWorkbench } from "./useDeveloperWorkbench";

type Controller = ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;

export function AppRuntimeConsole({ controller }: { controller: Controller }) {
  return (
    <RuntimeApiConsole
      events={controller.runtimeEvents}
      graph={controller.relationshipGraph}
      isRunning={controller.activeRun !== null}
      collapsed={controller.consoleCollapsed}
      onToggleCollapsed={() => controller.setConsoleCollapsed((collapsed) => !collapsed)}
      onSelectRuntimeNode={(nodeId) => {
        controller.selectRelationshipNode(nodeId);
        controller.setMainView("graph");
      }}
      onSendRequest={controller.handleApiRequest}
      webSocketEvents={controller.webSocketEvents}
      webSocketEventSubscriptionReady={controller.webSocketEventSubscriptionReady}
      onWebSocketConnect={controller.handleWebSocketConnect}
      onWebSocketSend={controller.handleWebSocketSend}
      onWebSocketDisconnect={controller.handleWebSocketDisconnect}
      settings={controller.settings}
      debugActive={controller.activeRun?.mode === "debug"}
      workspaceScope={JSON.stringify([controller.workspace?.id ?? null, controller.workspaceGeneration])}
    />
  );
}
