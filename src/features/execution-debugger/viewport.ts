import { zoomIdentity, type ZoomTransform } from "d3-zoom";
import {
  DEBUG_VIEW_HEIGHT,
  DEBUG_VIEW_WIDTH,
  type DebugExecutionLayout,
  type PositionedDebugStep,
} from "./layout";

export function centerTransform(
  node: PositionedDebugStep,
  svg?: SVGSVGElement | null,
): ZoomTransform {
  const scale = 0.92;
  const viewportWidth = svg?.clientWidth || DEBUG_VIEW_WIDTH;
  const viewportHeight = svg?.clientHeight || DEBUG_VIEW_HEIGHT;
  return zoomIdentity
    .translate(
      viewportWidth / 2 - (node.x + node.width / 2) * scale,
      viewportHeight / 2 - (node.y + node.height / 2) * scale,
    )
    .scale(scale);
}

export function initialTransform(
  layout: DebugExecutionLayout,
  svg?: SVGSVGElement | null,
): ZoomTransform {
  const last = layout.nodes[layout.nodes.length - 1];
  return last ? centerTransform(last, svg) : zoomIdentity;
}

export function getStepById(
  layout: DebugExecutionLayout,
  stepId?: string,
): PositionedDebugStep | undefined {
  return stepId ? layout.nodes.find((node) => node.step.id === stepId) : undefined;
}

export function stepIsVisible(
  node: PositionedDebugStep,
  transform: ZoomTransform,
  svg?: SVGSVGElement | null,
): boolean {
  const viewportWidth = svg?.clientWidth || DEBUG_VIEW_WIDTH;
  const viewportHeight = svg?.clientHeight || DEBUG_VIEW_HEIGHT;
  const margin = 48;
  const left = transform.applyX(node.x);
  const right = transform.applyX(node.x + node.width);
  const top = transform.applyY(node.y);
  const bottom = transform.applyY(node.y + node.height);
  return right >= margin
    && left <= viewportWidth - margin
    && bottom >= margin
    && top <= viewportHeight - margin;
}
