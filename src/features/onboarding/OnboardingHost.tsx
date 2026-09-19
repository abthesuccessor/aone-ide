import type { useAppController } from "../../app/useAppController";
import type { useDeveloperWorkbench } from "../../app/useDeveloperWorkbench";
import { OnboardingPanel } from "./OnboardingPanel";

interface OnboardingHostProps {
  controller: ReturnType<typeof useAppController> & ReturnType<typeof useDeveloperWorkbench>;
  onExit: () => void;
}

export function OnboardingHost({ controller, onExit }: OnboardingHostProps) {
  return (
    <OnboardingPanel
      workspace={controller.workspace}
      workspaceGeneration={controller.workspaceGeneration}
      workspaceBusy={controller.phase === "loading" || controller.openingWorkspace || controller.scanningWorkspace}
      aiEnv={controller.aiEnv}
      aiEnvLoading={controller.aiEnvLoading}
      onOpenFolder={controller.handleOnboardingOpenFolder}
      onClone={controller.handleCloneRepository}
      onCreate={controller.handleCreateProject}
      onPickAiEnv={controller.handlePickAiEnv}
      onOpenTools={() => controller.setActivityView("tools")}
      onOpenProjectAgent={() => controller.setActivityView("agent-setup")}
      onExit={onExit}
    />
  );
}
