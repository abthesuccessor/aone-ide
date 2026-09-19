import { describe, expect, it } from "vitest";
import { githubDestination, validProjectName } from "./model";

describe("onboarding input validation", () => {
  it("normalizes a public GitHub URL and derives its Documents folder", () => {
    expect(githubDestination(" https://github.com/crynta/terax-ai.git ")).toEqual({
      repositoryUrl: "https://github.com/crynta/terax-ai",
      destinationName: "terax-ai",
    });
  });

  it.each([
    "git@github.com:owner/repo.git",
    "https://user:secret@github.com/owner/repo",
    "https://github.com/owner/repo?token=secret",
    "https://gitlab.com/owner/repo",
    "https://github.com/owner/repo/extra",
  ])("rejects a clone target outside the public GitHub HTTPS contract: %s", (value) => {
    expect(() => githubDestination(value)).toThrow();
  });

  it("accepts a readable project name and rejects path traversal", () => {
    expect(validProjectName("  design-notes  ")).toBe("design-notes");
    expect(() => validProjectName("../private")).toThrow();
    expect(() => validProjectName("design notes")).toThrow();
    expect(() => validProjectName(".hidden")).toThrow();
    expect(() => validProjectName("trailing.")).toThrow();
    expect(() => validProjectName(" ")).toThrow();
  });
});
