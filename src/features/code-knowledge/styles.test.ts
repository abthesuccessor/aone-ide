import { describe, expect, it } from "vitest";
import catalogTreeCss from "../../styles/api-catalog-tree.css?raw";
import knowledgeCss from "../../styles/code-knowledge.css?raw";

function declarations(source: string, selector: string): string {
  const matches: string[] = [];
  for (const match of source.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const selectors = match[1]?.split(",").map((value) => value.trim()) ?? [];
    if (selectors.includes(selector) && match[2]) matches.push(match[2]);
  }
  if (matches.length === 0) throw new Error(`No declarations found for ${selector}`);
  return matches.join("\n");
}

describe("code knowledge endpoint layout", () => {
  it("reserves a method track wider than the shared badge minimum", () => {
    const badge = declarations(catalogTreeCss, ".api-method");
    const row = declarations(knowledgeCss, ".knowledge-endpoint-button");
    const badgeMinimum = Number(badge.match(/min-width:\s*([0-9.]+)px/)?.[1]);
    const trackMinimum = Number(
      row.match(/grid-template-columns:\s*minmax\(([0-9.]+)px,\s*max-content\)/)?.[1],
    );

    expect(badgeMinimum).toBeGreaterThan(0);
    expect(trackMinimum).toBeGreaterThanOrEqual(badgeMinimum);
    expect(row).toMatch(/minmax\(0,\s*1fr\)/);
    expect(row).toMatch(/min-width:\s*0/);
  });

  it("keeps long route and handler text inside the shrinkable column", () => {
    for (const selector of [
      ".knowledge-endpoint-button strong",
      ".knowledge-endpoint-button small",
    ]) {
      const copy = declarations(knowledgeCss, selector);
      expect(copy).toMatch(/overflow:\s*hidden/);
      expect(copy).toMatch(/text-overflow:\s*ellipsis/);
      expect(copy).toMatch(/white-space:\s*nowrap/);
    }
  });
});
