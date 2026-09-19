import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { MainView } from "./model";
import { useGraphFocus } from "./useGraphFocus";

describe("graph focus mode", () => {
  it("opens for the API catalog and Escape restores the invoking control", () => {
    const { result } = renderHook(() => useGraphFocus("api-data", "graph"));
    const button = document.createElement("button");
    document.body.append(button);
    act(() => { result.current.buttonRef.current = button; });
    expect(result.current.active).toBe(true);
    act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    expect(result.current.active).toBe(false);
    expect(button).toHaveFocus();
    button.remove();
  });

  it("exits when source replaces the graph", () => {
    const { result, rerender } = renderHook<ReturnType<typeof useGraphFocus>, { view: MainView }>(
      ({ view }) => useGraphFocus("system", view),
      { initialProps: { view: "graph" } },
    );
    act(() => result.current.toggle());
    expect(result.current.active).toBe(true);
    rerender({ view: "source" as const });
    expect(result.current.active).toBe(false);
  });
});
