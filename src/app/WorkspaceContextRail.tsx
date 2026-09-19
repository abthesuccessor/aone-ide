import { EvidencePanel } from "../components/EvidencePanel";
import type { useAppController } from "./useAppController";
import type { useDeveloperWorkbench } from "./useDeveloperWorkbench";

type Controller = ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;

export function WorkspaceContextRail({ controller }: { controller: Controller }) {
  return (
    <div className="workspace-context-rail" aria-label="Evidence inspector">
      <EvidencePanel
        graph={controller.relationshipGraph}
        selectedNode={controller.selectedNode}
        explanation={controller.explanation}
        explaining={controller.explaining}
        explanationError={controller.explanationError}
        aiAvailable={controller.aiConfiguration?.inferenceAvailable === true}
        onExplain={(nodeId) => void controller.handleExplain(nodeId)}
        onConfigureAi={() => controller.setCodeOnlyMode(false)}
        onSelectNode={controller.selectRelationshipNode}
        onOpenSource={controller.openRelationshipNode}
      />
    </div>
  );
}
