# Onboarding

The onboarding feature owns the full-workbench Getting Started experience.

- `OnboardingPanel.tsx` provides the accessible section navigation.
- `WorkspaceStart.tsx` validates public GitHub HTTPS clone and direct-Documents-child create inputs, then delegates every workspace mutation to the shared app controller path.
- `AiSetup.tsx` shows OpenAI/Anthropic env-file templates without accepting or storing secret values.
- `ToolsSetup.tsx` routes to existing opt-in tool and project inspection panels.
- `GitOnboarding.tsx` starts sanitized Git/public-key-fingerprint inspection only after user action, guards late results by workspace generation, and keeps fork/branch/remote/push work as a manual checklist.
- `model.ts` contains handwritten feature types for the native onboarding contract.
- `workspaceMutation.ts` serializes Open, Clone, and Create adoption with the existing workspace-operation guard.

No component inspects the host, filesystem, Git setup, AI tools, or provider configuration on mount.
