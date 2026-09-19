import { describe, expect, it } from "vitest";
import type { GraphNode } from "../../types";
import { changedNewLinesFromPatch, relationshipNodeForGitPath } from "./relationshipSelection";

function node(id: string, relativePath: string, startLine: number, endLine: number): GraphNode {
  return {
    id,
    kind: "function",
    label: id,
    source: { relativePath, startLine, startColumn: 1, endLine, endColumn: 1 },
    evidence: "resolved",
    metadata: {},
  };
}

describe("Git relationship selection", () => {
  it("extracts only added and modified new-file lines from a unified patch", () => {
    expect(changedNewLinesFromPatch([
      "@@ -9,4 +9,5 @@ function checkout() {",
      " unchanged();",
      "-oldCall();",
      "+newCall();",
      "+audit();",
      " remaining();",
    ].join("\n"))).toEqual([10, 11]);
  });

  it("selects the smallest exact-path node intersecting the changed lines", () => {
    const nodes = [
      node("file-scope", "src/checkout.ts", 1, 100),
      node("checkout", "src/checkout.ts", 8, 30),
      node("new-call", "src/checkout.ts", 10, 11),
      node("other-file", "src/other.ts", 10, 11),
    ];
    const patch = "@@ -9,2 +9,2 @@\n unchanged();\n+newCall();";

    expect(relationshipNodeForGitPath(nodes, "src/checkout.ts", patch)?.id).toBe("new-call");
  });

  it("uses a stable earliest-node fallback before the diff is loaded", () => {
    const nodes = [
      node("later", "src/checkout.ts", 40, 45),
      node("earlier", "./src/checkout.ts", 3, 10),
    ];

    expect(relationshipNodeForGitPath(nodes, "src/checkout.ts")?.id).toBe("earlier");
    expect(relationshipNodeForGitPath(nodes, "src/missing.ts")).toBeNull();
  });

  it("selects the file change marker until a matching changed symbol is known", () => {
    const marker = {
      ...node("git-change:src/checkout.ts", "src/checkout.ts", 1, 2),
      metadata: { gitChangeMarker: true, gitChangeKind: "modify" },
    };
    const symbol = node("checkout", "src/checkout.ts", 20, 30);

    expect(relationshipNodeForGitPath([symbol, marker], "src/checkout.ts")?.id).toBe(marker.id);
    expect(relationshipNodeForGitPath(
      [symbol, marker],
      "src/checkout.ts",
      "@@ -21,1 +21,2 @@\n unchanged();\n+changed();",
    )?.id).toBe(symbol.id);
  });
});
