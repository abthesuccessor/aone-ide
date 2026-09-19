import { describe, expect, it } from "vitest";
import type { DebugExecutionStep } from "./contracts";
import {
  clampStepWindow,
  createDebugExecutionLayout,
  DEBUG_PAGE_LIMIT,
  latestStepWindow,
} from "./layout";

function step(index: number, overrides: Partial<DebugExecutionStep> = {}): DebugExecutionStep {
  return {
    id: `step-${index}`,
    reportedStepId: `step-${index}`,
    eventIds: [`event-${index}`],
    workflowId: "workflow-1",
    sequence: index + 1,
    kind: "method",
    stage: "service",
    label: `Step ${index}`,
    timestamp: `2026-08-17T02:00:${String(index % 60).padStart(2, "0")}Z`,
    evidence: "observed",
    repeatCount: 1,
    protocol: "AONE_DEBUG_V1",
    ...overrides,
  };
}

describe("deterministic execution debugger layout", () => {
  it("caps each rendered page at 120 cards", () => {
    const steps = Array.from({ length: 400 }, (_, index) => step(index));
    expect(clampStepWindow(steps, 0).steps).toHaveLength(DEBUG_PAGE_LIMIT);
    expect(createDebugExecutionLayout(clampStepWindow(steps, 0).steps).nodes).toHaveLength(120);
  });

  it("uses stable absolute positions while the latest 120-card window advances", () => {
    const before = Array.from({ length: 121 }, (_, index) => step(index));
    const after = [...before, step(121)];
    const beforeWindow = latestStepWindow(before);
    const afterWindow = latestStepWindow(after);
    expect(beforeWindow.start).toBe(1);
    expect(afterWindow.start).toBe(2);
    const beforeLayout = createDebugExecutionLayout(beforeWindow.steps, beforeWindow.start);
    const afterLayout = createDebugExecutionLayout(afterWindow.steps, afterWindow.start);
    const sharedBefore = beforeLayout.nodes.find((node) => node.step.id === "step-2");
    const sharedAfter = afterLayout.nodes.find((node) => node.step.id === "step-2");
    expect(sharedBefore?.x).toBe(sharedAfter?.x);
    expect(beforeWindow.steps).toHaveLength(120);
    expect(afterWindow.steps).toHaveLength(120);
  });

  it("draws sequence arrows only inside an explicit instrumented workflow", () => {
    const debugLayout = createDebugExecutionLayout([
      step(0),
      step(1),
      step(2, { workflowId: "workflow-2" }),
    ]);
    expect(debugLayout.connectors.filter((connector) => connector.kind === "sequence")).toHaveLength(1);

    const replayLayout = createDebugExecutionLayout([
      step(0, { protocol: "AONE_TRACE_V1", kind: "trace" }),
      step(1, { protocol: "AONE_TRACE_V1", kind: "trace" }),
      step(2, { workflowId: undefined, protocol: "boundary", kind: "boundary" }),
      step(3, { workflowId: undefined, protocol: "boundary", kind: "boundary" }),
    ]);
    expect(replayLayout.connectors.filter((connector) => connector.kind === "sequence")).toHaveLength(0);
  });

  it("keeps card coordinates deterministic with no force simulation or overlap", () => {
    const layout = createDebugExecutionLayout([step(0), step(1), step(2, { stage: "data" })]);
    expect(layout.nodes.map(({ x }) => x)).toEqual([...layout.nodes.map(({ x }) => x)].sort((a, b) => a - b));
    for (let index = 1; index < layout.nodes.length; index += 1) {
      expect(layout.nodes[index]!.x).toBeGreaterThan(layout.nodes[index - 1]!.x + layout.nodes[index - 1]!.width);
    }
  });
});
