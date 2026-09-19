# Styles

The application stylesheet is split into ordered, feature-focused files. The
single entry point is `index.css`; `src/main.tsx` must import only that file.

## Import order and responsibilities

1. `tokens.css` defines shared motion and visual custom properties.
2. `base.css` defines the document reset, typed-theme fallbacks and aliases,
   typography, focus, and scrollbars. Runtime values come from `editor/themes.ts`.
3. `chrome.css` styles the app grid, title bar, workspace explorer, and shared
   shell controls. It also reserves native macOS traffic-light space and owns the
   two-pixel, text-free indexing indicator directly below the title bar.
4. `activity.css` styles the compact Explorer/Source Control/MCP/settings rail.
5. `search.css` styles grouped indexed matches, match-kind labels, and search states.
6. `devtools.css` styles local Git and MCP/CLI configuration panels.
7. `project-environment.css` styles consent, toolchain evidence, recommendations,
   and inferred AI setup summaries.
8. `onboarding.css` and `onboarding-git.css` style the full-workbench setup,
   private AI env templates, sanitized Git report, and manual Git guidance.
9. `workbench.css` styles workbench navigation, empty states, and shared stage
   controls.
10. `graph.css` styles shared graph states, edges, evidence, and zoom controls.
11. `execution-flow.css` styles the static six-stage Flow/Map cards, nested
    source groups, search, runtime disclosure, and bounded summary.
12. `api-catalog.css` and `api-catalog-tree.css` style the complete-inventory
    toolbar, role/service/route hierarchy, bounded rows, and loading/error states.
13. `source.css` styles source headers, loading state, and editor containment.
14. `editor.css` styles Monaco workbench tabs and editor actions.
15. `settings.css` styles bounded appearance/editor/terminal preferences.
16. `console.css` styles runtime rows, API/GraphQL request composition,
    responses, timelines, and headers.
17. `terminal.css` imports xterm.js CSS and styles the integrated PTY toolbar,
    host, and error state.
18. `realtime.css` styles WebSocket connection controls and transcripts.
19. `evidence.css` styles evidence inspection, relations, metadata, and AI output.
20. `overlays.css` styles environment selection, toast, status bar, and shared
    keyframes.
21. `motion.css` contains reusable panel, tabs, and toast recipes plus responsive
    and reduced-motion overrides.

The order is part of the UI contract. Add new selectors to the narrowest feature
file and do not import feature files directly from components.

## Conventions

- Keep each handwritten CSS file below 500 lines.
- Reuse tokens rather than introducing component-local copies of shared colors,
  durations, easing curves, radii, or typography.
- Keep selectors shallow and tied to semantic component classes.
- Preserve the evidence colors and visual distinction between static, observed,
  and inferred claims.
- Drive all three themes from semantic custom properties; avoid hard-coded dark
  colors in feature files when a token exists.
- Put responsive overrides after their base declarations unless a global media
  policy intentionally belongs in `motion.css`.

## Accessibility

Visible keyboard focus must remain high contrast. Disabled controls must retain
both semantic `disabled` state and a visual treatment. Animation additions need
a corresponding `prefers-reduced-motion` behavior; essential status information
must never depend on motion or color alone.

Async search/status/error text must use semantic live/status/alert roles rather
than animation alone. Modal styling must preserve the settings dialog's initial
focus, tested Tab/Shift+Tab containment, Escape-close behavior, capture-phase
workbench-shortcut blocking, and restored invoking focus.

Graph edge labels remain visual SVG presentation. The selected-node relation is
also mirrored into a screen-reader-only `aria-live` region with separate
incoming/outgoing lists; do not hide or restyle that region into duplicate
visible content. Search reaches every loaded node while collapsed groups keep
the mounted card count bounded. Fit remains the explicit whole-map shrink control.

The indexing line exposes progress through `role="progressbar"`, its accessible
name, and value text; it does not render a second visible completion banner.
Reduced motion converts indeterminate movement to a static full-width line.

## Verification

The App and component tests protect class-driven interaction states. The
production build validates CSS parsing and import resolution, and the size check
guards the complete emitted bundle.

```bash
npm test
npm run build
npm run size
```
