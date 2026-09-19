import { CaretLeft, CaretRight } from "@phosphor-icons/react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import type { SourceLocation } from "../../types";
import {
  DEBUG_LANE_HEIGHT,
  DEBUG_LANE_TOP,
  DEBUG_LANES,
  debugConnectorPath,
  type DebugExecutionLayout,
  type DebugStepWindow,
  type PositionedDebugStep,
} from "./layout";
import { sourceLabel, stageIcon, stepTypeLabel } from "./DebuggerChrome";
import type { DebugExecutionStep } from "./contracts";

function stepKeyDown(
  event: ReactKeyboardEvent<SVGGElement>,
  node: PositionedDebugStep,
  nodes: PositionedDebugStep[],
  onSelect: (stepId: string) => void,
  onOpenSource: (source: SourceLocation) => void,
) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    onSelect(node.step.id);
  }
  if (event.key === "Enter" && (event.metaKey || event.ctrlKey) && node.step.source) {
    event.preventDefault();
    onOpenSource(node.step.source);
  }
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  event.preventDefault();
  const index = nodes.indexOf(node);
  const target = event.key === "Home"
    ? nodes[0]
    : event.key === "End"
      ? nodes[nodes.length - 1]
      : nodes[index + (event.key === "ArrowLeft" ? -1 : 1)];
  if (!target) return;
  onSelect(target.step.id);
  event.currentTarget.ownerSVGElement
    ?.querySelector<SVGGElement>(`[data-debug-step-id="${CSS.escape(target.step.id)}"]`)
    ?.focus();
}

interface DebuggerCanvasProps {
  svgRef: React.RefObject<SVGSVGElement | null>;
  layout: DebugExecutionLayout;
  stepWindow: DebugStepWindow;
  allSteps: DebugExecutionStep[];
  selectedStepId?: string;
  currentStepId?: string;
  replay?: boolean;
  onSelect: (stepId: string) => void;
  onOpenSource: (source: SourceLocation) => void;
  onEarlier: () => void;
  onLater: () => void;
}

export function DebuggerCanvas({
  svgRef,
  layout,
  stepWindow,
  allSteps,
  selectedStepId,
  currentStepId,
  replay = false,
  onSelect,
  onOpenSource,
  onEarlier,
  onLater,
}: DebuggerCanvasProps) {
  const nodeById = new Map(layout.nodes.map((node) => [node.step.id, node]));
  return (
    <div className="debugger-canvas-wrap">
      <svg
        ref={svgRef}
        className="debugger-canvas"
        role="group"
        aria-label="Process-reported safe-point workflow"
        data-testid="execution-debugger-canvas"
      >
        <defs>
          <marker id="debug-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M 0 0 L 8 4 L 0 8 z" />
          </marker>
        </defs>
        <g data-debug-viewport>
          <g className="debugger-lanes" aria-hidden="true">
            {DEBUG_LANES.map((lane, index) => {
              const y = DEBUG_LANE_TOP + index * DEBUG_LANE_HEIGHT;
              return (
                <g key={lane.id} className="debugger-lane">
                  <rect x={0} y={y} width={layout.width} height={DEBUG_LANE_HEIGHT} />
                  <text x={14} y={y + 25} className="debugger-lane-label">{lane.label}</text>
                  <text x={14} y={y + 46} className="debugger-lane-description">{lane.description}</text>
                </g>
              );
            })}
          </g>
          <g className="debugger-connectors" aria-hidden="true">
            {layout.connectors.map((connector) => {
              const source = nodeById.get(connector.sourceId);
              const target = nodeById.get(connector.targetId);
              if (!source || !target) return null;
              const path = debugConnectorPath(source, target);
              const active = target.step.id === currentStepId;
              return (
                <g
                  key={connector.id}
                  className={`debugger-connector-group${active ? " is-active" : ""}`}
                >
                  <path
                    className={`debugger-connector is-${connector.kind}${active ? " is-active" : ""}`}
                    d={path}
                    markerEnd="url(#debug-arrow)"
                  />
                  {active && (
                    <circle className="debugger-execution-token" r={4}>
                      <animateMotion dur="850ms" repeatCount="indefinite" path={path} />
                    </circle>
                  )}
                </g>
              );
            })}
          </g>
          <g className="debugger-steps">
            {layout.nodes.map((node) => {
              const step = node.step;
              const isSelected = selectedStepId === step.id;
              const isPosition = currentStepId === step.id;
              const isCurrent = isPosition && !replay;
              const label = step.label.length > 36 ? `${step.label.slice(0, 35)}…` : step.label;
              const source = sourceLabel(step);
              return (
                <g
                  key={step.id}
                  className={`debugger-step kind-${step.kind}${isSelected ? " is-selected" : ""}${isCurrent ? " is-current" : ""}${isPosition && replay ? " is-replay-position" : ""}`}
                  transform={`translate(${node.x} ${node.y})`}
                  role="button"
                  tabIndex={isSelected ? 0 : -1}
                  aria-label={`${label}, ${stepTypeLabel(step)}, ${source}, ${step.evidence} evidence${isPosition ? replay ? ", replay position" : ", current safe point" : ""}`}
                  data-debug-step-id={step.id}
                  onClick={() => onSelect(step.id)}
                  onDoubleClick={() => step.source && onOpenSource(step.source)}
                  onKeyDown={(event) => stepKeyDown(event, node, layout.nodes, onSelect, onOpenSource)}
                >
                  <rect className="debugger-step-surface" width={node.width} height={node.height} rx={5} />
                  <rect className="debugger-step-accent" width={3} height={node.height} rx={2} />
                  <text x={14} y={21} className="debugger-step-kind">{stepTypeLabel(step)}</text>
                  <text x={node.width - 13} y={21} textAnchor="end" className="debugger-step-sequence">#{step.sequence}</text>
                  <text x={14} y={49} className="debugger-step-label">{label}</text>
                  <text x={14} y={75} className="debugger-step-source">{source}</text>
                  <text x={14} y={99} className="debugger-step-state">
                    {isPosition ? replay ? "REPLAY POSITION" : "CURRENT SAFE POINT" : step.branchOutcome ? `OUTCOME ${step.branchOutcome}` : step.state?.toUpperCase() ?? "OBSERVED"}
                  </text>
                  {step.repeatCount > 1 && <text x={node.width - 13} y={99} textAnchor="end" className="debugger-step-repeat">×{step.repeatCount}</text>}
                </g>
              );
            })}
          </g>
        </g>
      </svg>
      <div className="debugger-page-controls">
        <button type="button" onClick={onEarlier} disabled={stepWindow.omittedBefore === 0}>
          <CaretLeft size={12} /> Earlier
        </button>
        <span>{allSteps.length === 0 ? "0" : `${stepWindow.start + 1}-${stepWindow.end}`} of {allSteps.length}</span>
        <button type="button" onClick={onLater} disabled={stepWindow.omittedAfter === 0}>
          Later <CaretRight size={12} />
        </button>
      </div>
    </div>
  );
}

export function DebuggerHistory({
  steps,
  selectedStepId,
  onSelect,
}: {
  steps: DebugExecutionStep[];
  selectedStepId?: string;
  onSelect: (stepId: string) => void;
}) {
  return (
    <div className="debugger-history" aria-label="Retained process-reported safe-point history">
      <header><span>Retained reported history</span><small>Replay selection does not execute code</small></header>
      <div>
        {steps.map((step) => (
          <button
            key={step.id}
            type="button"
            className={selectedStepId === step.id ? "is-selected" : ""}
            onClick={() => onSelect(step.id)}
            title={`${step.label} | ${sourceLabel(step)}`}
          >
            <time>{new Date(step.timestamp).toLocaleTimeString([], { hour12: false, hour: "2-digit", minute: "2-digit", second: "2-digit" })}</time>
            <span>{stageIcon(step)}</span>
            <strong>{step.label}</strong>
            <small>{stepTypeLabel(step)}</small>
          </button>
        ))}
      </div>
    </div>
  );
}
