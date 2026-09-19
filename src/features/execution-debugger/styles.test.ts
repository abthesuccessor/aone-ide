import { describe, expect, it } from "vitest";
import debuggerCss from "../../styles/execution-debugger.css?raw";
import debuggerStateCss from "../../styles/execution-debugger-states.css?raw";
import debuggerSource from "./ExecutionDebugger.tsx?raw";

function declarations(selector: string): string {
  const matches: string[] = [];
  for (const match of debuggerCss.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = match[1]?.split(",").map((value) => value.trim()) ?? [];
    if (selectors.includes(selector) && match[2]) matches.push(match[2]);
  }
  if (matches.length === 0) throw new Error(`No declarations found for ${selector}`);
  return matches.join("\n");
}

function fontSize(selector: string): number {
  const rules = declarations(selector);
  const explicit = rules.match(/font-size:\s*([0-9.]+)px/);
  const shorthand = rules.match(/font:\s*([0-9.]+)px(?:\/[0-9.]+)?\s/);
  const value = explicit?.[1] ?? shorthand?.[1];
  if (!value) throw new Error(`No font size found for ${selector}`);
  return Number(value);
}

describe("execution debugger visual tokens", () => {
  it("uses only workbench color tokens", () => {
    const combined = `${debuggerCss}\n${debuggerStateCss}`;
    expect(combined).not.toMatch(/#[0-9a-f]{3,8}\b/i);
    expect(combined).not.toMatch(/\brgba?\(/i);
    expect(combined).toMatch(/var\(--observed\)/);
    expect(combined).toMatch(/color-mix\(in srgb, var\(--observed\)/);
  });

  it("contains all four resizable panes and their scroll surfaces", () => {
    expect(debuggerSource).toMatch(/id="trace-api-panel"[^>]*minSize="200px"/s);
    expect(debuggerSource).toMatch(/id="trace-source-panel"[^>]*minSize="300px"/s);
    expect(debuggerSource).toMatch(/id="trace-path-panel"[^>]*minSize="230px"/s);
    expect(debuggerSource).toMatch(/id="trace-data-panel"[^>]*minSize="200px"/s);

    for (const selector of [
      ".trace-api-pane",
      ".trace-source-pane",
      ".trace-path-pane",
      ".trace-data-pane",
    ]) {
      const rules = declarations(selector);
      expect(rules).toMatch(/min-width:\s*0/);
      expect(rules).toMatch(/min-height:\s*0/);
      expect(rules).toMatch(/overflow:\s*hidden/);
    }
    for (const selector of [".trace-api-scroll", ".trace-path-scroll", ".trace-data-scroll"]) {
      const rules = declarations(selector);
      expect(rules).toMatch(/min-width:\s*0/);
      expect(rules).toMatch(/min-height:\s*0/);
      expect(rules).toMatch(/overflow:\s*auto/);
    }
  });

  it("lights only the current trace node and keeps replay visually distinct", () => {
    const current = declarations(".trace-path-node.is-current");
    expect(current).toMatch(/border-color:\s*var\(--observed\)/);
    expect(current).toMatch(/background:\s*color-mix\(in srgb, var\(--observed\)/);
    expect(current).toMatch(/box-shadow:[^;]*var\(--observed\)/);
    expect(declarations(".trace-path-node.is-current .trace-path-icon"))
      .toMatch(/animation:\s*trace-node-pulse/);
    expect(declarations(".trace-path-node.is-replay-position"))
      .toMatch(/border-color:\s*var\(--amber\)/);
  });

  it("keeps source-line and structured-data text readable", () => {
    expect(fontSize(".trace-path-copy strong")).toBeGreaterThanOrEqual(10);
    expect(fontSize(".trace-path-copy span")).toBeGreaterThanOrEqual(8);
    expect(fontSize(".trace-step-title strong")).toBeGreaterThanOrEqual(11);
    expect(fontSize(".json-tree")).toBeGreaterThanOrEqual(9);
    expect(fontSize(".trace-shape-fields table")).toBeGreaterThanOrEqual(8);
    expect(declarations(".json-tree")).toMatch(/font:\s*9px\/1\.55/);
  });

  it("removes active-node animation when reduced motion is requested", () => {
    expect(debuggerCss).toMatch(/@media \(prefers-reduced-motion: reduce\)/);
    expect(debuggerCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.trace-path-node\.is-current \.trace-path-icon,[\s\S]*\.trace-path-loader\s*\{\s*animation:\s*none;/,
    );
  });
});
