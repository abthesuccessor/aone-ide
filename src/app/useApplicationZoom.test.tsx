import { fireEvent, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useApplicationZoom } from "./useApplicationZoom";

describe("application zoom shortcuts", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.body.style.zoom = "";
  });

  afterEach(() => {
    document.body.style.zoom = "";
  });

  it("zooms the full application from any focused panel and resets to standard size", () => {
    const { result } = renderHook(() => useApplicationZoom());
    expect(result.current).toBe(1);

    fireEvent.keyDown(window, { key: "=", metaKey: true });
    expect(result.current).toBe(1.1);
    expect(document.body.style.zoom).toBe("1.1");

    const graph = document.createElement("div");
    graph.className = "force-graph";
    const target = document.createElement("button");
    graph.append(target);
    document.body.append(graph);
    fireEvent.keyDown(target, { key: "-", metaKey: true });
    expect(result.current).toBe(1);
    graph.remove();

    fireEvent.keyDown(window, { key: "-", metaKey: true });
    expect(result.current).toBe(0.9);

    fireEvent.keyDown(window, { key: "0", metaKey: true });
    expect(result.current).toBe(1);
  });

  it("ignores legacy zoom keys so the VS Code density baseline starts at actual size", () => {
    window.localStorage.setItem("aone.applicationZoom", "0.6");
    window.localStorage.setItem("aone.applicationZoom.v2", "0.7");
    const { result } = renderHook(() => useApplicationZoom());
    expect(result.current).toBe(1);
  });
});
