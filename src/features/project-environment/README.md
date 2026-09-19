# Project environment

This feature presents deterministic, permission-gated project setup guidance.

- `ProjectEnvironmentPanel.tsx` never inspects on mount. The user must start the inspection.
- Rust owns two sequential native consent dialogs: executable-path discovery first, then bounded fixed version probes.
- `model.ts` mirrors the native camel-case DTO. Secret values, arbitrary command text, and executable arguments are not accepted from React.
- Workspace identity and generation guard every async result so an old folder cannot populate the new folder's panel.
- A run recommendation can select an existing backend-owned run profile; it cannot create or execute a command.

The browser build returns deterministic sample data and performs no local-machine inspection.
