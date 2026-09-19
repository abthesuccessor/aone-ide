import type { DebugExecutionStep, DebugStage } from "./model";

export const DEBUG_VIEW_WIDTH = 1_800;
export const DEBUG_VIEW_HEIGHT = 1_100;
export const DEBUG_CARD_WIDTH = 244;
export const DEBUG_CARD_HEIGHT = 110;
export const DEBUG_STEP_GAP = 46;
export const DEBUG_LANE_HEIGHT = 144;
export const DEBUG_LANE_TOP = 66;
export const DEBUG_WORLD_LEFT = 150;
export const DEBUG_PAGE_LIMIT = 120;

export const DEBUG_LANES: Array<{
  id: DebugStage;
  label: string;
  description: string;
}> = [
  { id: "trigger", label: "Trigger / frontend", description: "API-client boundary or child-reported frontend entry" },
  { id: "api", label: "API", description: "Request handler and response boundary" },
  { id: "backend", label: "Backend", description: "Controller, trait, and module safe points" },
  { id: "service", label: "Service / agent", description: "Methods, branches, agents, and application logic" },
  { id: "data", label: "Data", description: "Database operations and result shapes" },
  { id: "external", label: "External", description: "Cloud, agent, webhook, and third-party calls" },
  { id: "response", label: "Response", description: "Process-reported workflow completion or failure" },
];

const LANE_INDEX = new Map(DEBUG_LANES.map((lane, index) => [lane.id, index]));

export interface PositionedDebugStep {
  step: DebugExecutionStep;
  x: number;
  y: number;
  width: number;
  height: number;
  windowIndex: number;
}

export interface DebugConnector {
  id: string;
  sourceId: string;
  targetId: string;
  kind: "sequence" | "parent";
}

export interface DebugExecutionLayout {
  nodes: PositionedDebugStep[];
  connectors: DebugConnector[];
  width: number;
  height: number;
}

export interface DebugStepWindow {
  steps: DebugExecutionStep[];
  start: number;
  end: number;
  omittedBefore: number;
  omittedAfter: number;
}

export function clampStepWindow(
  steps: DebugExecutionStep[],
  requestedStart: number,
  limit = DEBUG_PAGE_LIMIT,
): DebugStepWindow {
  const boundedLimit = Math.max(1, Math.min(limit, DEBUG_PAGE_LIMIT));
  const lastPageStart = steps.length === 0
    ? 0
    : Math.floor((steps.length - 1) / boundedLimit) * boundedLimit;
  const requestedPageStart = Math.floor(Math.max(0, requestedStart) / boundedLimit) * boundedLimit;
  const start = Math.min(requestedPageStart, lastPageStart);
  const end = Math.min(steps.length, start + boundedLimit);
  return {
    steps: steps.slice(start, end),
    start,
    end,
    omittedBefore: start,
    omittedAfter: Math.max(0, steps.length - end),
  };
}

export function latestStepWindow(
  steps: DebugExecutionStep[],
  limit = DEBUG_PAGE_LIMIT,
): DebugStepWindow {
  const boundedLimit = Math.max(1, Math.min(limit, DEBUG_PAGE_LIMIT));
  const start = Math.max(0, steps.length - boundedLimit);
  const end = steps.length;
  return {
    steps: steps.slice(start, end),
    start,
    end,
    omittedBefore: start,
    omittedAfter: 0,
  };
}

export function windowContainingStep(
  steps: DebugExecutionStep[],
  stepId: string,
  limit = DEBUG_PAGE_LIMIT,
): DebugStepWindow {
  const index = steps.findIndex((step) => step.id === stepId);
  if (index < 0) return latestStepWindow(steps, limit);
  return clampStepWindow(steps, Math.floor(index / limit) * limit, limit);
}

export function createDebugExecutionLayout(
  steps: DebugExecutionStep[],
  positionOffset = 0,
): DebugExecutionLayout {
  const nodes = steps.map<PositionedDebugStep>((step, index) => ({
    step,
    x: DEBUG_WORLD_LEFT + (positionOffset + index) * (DEBUG_CARD_WIDTH + DEBUG_STEP_GAP),
    y: DEBUG_LANE_TOP + (LANE_INDEX.get(step.stage) ?? 1) * DEBUG_LANE_HEIGHT + 18,
    width: DEBUG_CARD_WIDTH,
    height: DEBUG_CARD_HEIGHT,
    windowIndex: index,
  }));
  const visibleIds = new Set(nodes.map((node) => node.step.id));
  const nodeById = new Map(nodes.map((node) => [node.step.id, node]));
  const connectors: DebugConnector[] = [];
  const connectorKeys = new Set<string>();

  const addConnector = (sourceId: string, targetId: string, kind: DebugConnector["kind"]) => {
    if (sourceId === targetId || !visibleIds.has(sourceId) || !visibleIds.has(targetId)) return;
    const key = `${sourceId}:${targetId}:${kind}`;
    if (connectorKeys.has(key)) return;
    connectorKeys.add(key);
    connectors.push({ id: key, sourceId, targetId, kind });
  };

  for (let index = 1; index < nodes.length; index += 1) {
    const current = nodes[index]!;
    const previous = nodes[index - 1]!;
    if (
      previous.step.protocol === "AONE_DEBUG_V1"
      && current.step.protocol === "AONE_DEBUG_V1"
      && previous.step.workflowId
      && previous.step.workflowId === current.step.workflowId
    ) {
      addConnector(previous.step.id, current.step.id, "sequence");
    }
    const parent = current.step.parentStepId
      ? nodeById.get(current.step.parentStepId)
      : undefined;
    if (
      parent
      && current.step.parentStepId !== previous.step.id
      && parent.step.workflowId === current.step.workflowId
    ) {
      addConnector(parent.step.id, current.step.id, "parent");
    }
  }

  return {
    nodes,
    connectors,
    width: Math.max(
      DEBUG_VIEW_WIDTH,
      DEBUG_WORLD_LEFT * 2 + (positionOffset + nodes.length) * (DEBUG_CARD_WIDTH + DEBUG_STEP_GAP),
    ),
    height: DEBUG_LANE_TOP + DEBUG_LANES.length * DEBUG_LANE_HEIGHT + 50,
  };
}

export function debugConnectorPath(
  source: PositionedDebugStep,
  target: PositionedDebugStep,
): string {
  const sourceX = source.x + source.width;
  const sourceY = source.y + source.height / 2;
  const targetX = target.x;
  const targetY = target.y + target.height / 2;
  const midpoint = sourceX + Math.max(18, (targetX - sourceX) / 2);
  return `M ${sourceX} ${sourceY} H ${midpoint} V ${targetY} H ${targetX}`;
}
