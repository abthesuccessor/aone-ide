import { describe, expect, it } from "vitest";
import { failureMessage, MAX_FAILURE_MESSAGE_CHARACTERS } from "./errorMessage";

describe("failureMessage", () => {
  it("normalizes native IPC strings and JavaScript errors", () => {
    expect(failureMessage("  native\nrejection  ", "fallback")).toBe("native rejection");
    expect(failureMessage(new Error("JavaScript failure"), "fallback")).toBe("JavaScript failure");
  });

  it("uses the fallback for unsafe values and bounds renderer-visible text", () => {
    expect(failureMessage({ secret: "not renderer copy" }, "Safe fallback")).toBe("Safe fallback");
    expect(failureMessage("x".repeat(2_000), "fallback")).toHaveLength(
      MAX_FAILURE_MESSAGE_CHARACTERS,
    );
    expect(failureMessage("x".repeat(2_000), "fallback")).toMatch(/…$/u);
  });
});
