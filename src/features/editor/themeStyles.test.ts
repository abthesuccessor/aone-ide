import { describe, expect, it } from "vitest";
import consoleStyles from "../../styles/console.css?raw";
import evidenceStyles from "../../styles/evidence.css?raw";
import graphStyles from "../../styles/graph.css?raw";
import overlayStyles from "../../styles/overlays.css?raw";
import realtimeStyles from "../../styles/realtime.css?raw";

const THEMED_STYLES = {
  "console.css": consoleStyles,
  "evidence.css": evidenceStyles,
  "graph.css": graphStyles,
  "overlays.css": overlayStyles,
  "realtime.css": realtimeStyles,
} as const;

type ThemedStyleName = keyof typeof THEMED_STYLES;

describe("theme-aware workbench feature styles", () => {
  it.each(Object.keys(THEMED_STYLES) as ThemedStyleName[])(
    "keeps %s free of fixed color literals",
    (fileName) => {
      const source = THEMED_STYLES[fileName];

      expect(source).not.toMatch(/#[\da-f]{3,8}\b/iu);
      expect(source).not.toMatch(/\b(?:rgb|hsl)a?\(/iu);
      expect(source).toContain("var(--");
    },
  );

  it("keeps evidence states on their semantic theme tokens", () => {
    const source = `${evidenceStyles}\n${graphStyles}`;

    expect(source).toContain("var(--observed)");
    expect(source).toContain("var(--inferred)");
    expect(source).toContain("var(--danger)");
  });
});
