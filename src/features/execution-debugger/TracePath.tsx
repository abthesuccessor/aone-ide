import {
  ArrowBendDownRight,
  BracketsCurly,
  CaretLeft,
  CaretRight,
  Database,
  Globe,
  Robot,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, type KeyboardEvent, type ReactNode } from "react";
import type { GraphNode, GraphSnapshot, SourceLocation } from "../../types";
import type { EditorOpenMode } from "../editor/model";
import { flowStageForNode } from "../graph/flowModel";
import type { DebugExecutionStep } from "./contracts";
import type { DebugStepWindow } from "./layout";
import { sourceLabel, stepTypeLabel } from "./DebuggerChrome";

interface StaticPathNode {
  node: GraphNode;
  relation?: string;
}

const MAX_STATIC_PATH_NODES = 160;

export function orderedStaticPath(graph: GraphSnapshot, preferredRootId?: string): StaticPathNode[] {
  if (graph.nodes.length === 0) return [];
  const byId = new Map(graph.nodes.map((node) => [node.id, node]));
  const incoming = new Map<string, number>();
  const outgoing = new Map<string, typeof graph.edges>();
  for (const edge of graph.edges) {
    if (!byId.has(edge.source) || !byId.has(edge.target)) continue;
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + 1);
    outgoing.set(edge.source, [...(outgoing.get(edge.source) ?? []), edge]);
  }
  const preferredRoot = preferredRootId ? byId.get(preferredRootId) : undefined;
  const roots = preferredRoot
    ? [preferredRoot]
    : graph.nodes.filter((node) => node.metadata.flowRoot === true);
  if (roots.length === 0) roots.push(...graph.nodes.filter((node) => !incoming.has(node.id)));
  if (roots.length === 0 && graph.nodes[0]) roots.push(graph.nodes[0]);

  const result: StaticPathNode[] = [];
  const seen = new Set<string>();
  const queue = roots.map((node) => ({ node, relation: undefined as string | undefined }));
  while (queue.length > 0 && result.length < MAX_STATIC_PATH_NODES) {
    const entry = queue.shift();
    if (!entry || seen.has(entry.node.id)) continue;
    seen.add(entry.node.id);
    result.push(entry);
    const next = [...(outgoing.get(entry.node.id) ?? [])]
      .sort((left, right) => left.kind.localeCompare(right.kind) || left.target.localeCompare(right.target));
    for (const edge of next) {
      const node = byId.get(edge.target);
      if (node && !seen.has(node.id)) queue.push({ node, relation: edge.kind });
    }
  }
  if (!preferredRoot && !graph.nodes.some((node) => node.metadata.flowRoot === true)) {
    for (const node of graph.nodes) {
      if (result.length >= MAX_STATIC_PATH_NODES) break;
      if (!seen.has(node.id)) result.push({ node });
    }
  }
  return result;
}

function stepIcon(step: DebugExecutionStep): ReactNode {
  if (step.kind === "database") return <Database size={14} weight="duotone" />;
  if (step.kind === "external") return <Globe size={14} weight="duotone" />;
  if (step.kind === "agent") return <Robot size={14} weight="duotone" />;
  if (step.kind === "branch") return <ArrowBendDownRight size={14} weight="duotone" />;
  return <BracketsCurly size={14} weight="duotone" />;
}

function nodeIcon(node: GraphNode): ReactNode {
  const stage = flowStageForNode(node);
  if (stage === "data") return <Database size={14} weight="duotone" />;
  if (stage === "external") return <Globe size={14} weight="duotone" />;
  return <BracketsCurly size={14} weight="duotone" />;
}

function moveFocus(
  event: KeyboardEvent<HTMLButtonElement>,
  selector: string,
  onSelect: (id: string) => void,
) {
  if (!["ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
  const nodes = Array.from(event.currentTarget.closest("ol")?.querySelectorAll<HTMLButtonElement>(selector) ?? []);
  const index = nodes.indexOf(event.currentTarget);
  const next = event.key === "Home"
    ? nodes[0]
    : event.key === "End"
      ? nodes[nodes.length - 1]
      : nodes[index + (event.key === "ArrowUp" ? -1 : 1)];
  if (!next) return;
  event.preventDefault();
  next.focus();
  const id = next.dataset.traceNodeId;
  if (id) onSelect(id);
}

interface TracePathProps {
  allSteps: DebugExecutionStep[];
  stepWindow: DebugStepWindow;
  selectedStepId?: string;
  currentStepId?: string;
  replay: boolean;
  indexedGraph?: GraphSnapshot;
  indexedRootId?: string;
  indexedGraphLoading?: boolean;
  error?: string | null;
  onSelect: (stepId: string) => void;
  onOpenSource: (source: SourceLocation, mode?: EditorOpenMode) => void;
  onEarlier: () => void;
  onLater: () => void;
}

export function TracePath({
  allSteps,
  stepWindow,
  selectedStepId,
  currentStepId,
  replay,
  indexedGraph,
  indexedRootId,
  indexedGraphLoading = false,
  error,
  onSelect,
  onOpenSource,
  onEarlier,
  onLater,
}: TracePathProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const staticPath = useMemo(() => orderedStaticPath(indexedGraph ?? {
    nodes: [], edges: [], truncated: false,
  }, indexedRootId), [indexedGraph, indexedRootId]);
  const focusId = selectedStepId ?? currentStepId;

  useEffect(() => {
    if (!focusId) return;
    const escapedFocusId = typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(focusId)
      : focusId.replace(/["\\]/g, "\\$&");
    const target = scrollRef.current?.querySelector<HTMLElement>(`[data-trace-node-id="${escapedFocusId}"]`);
    target?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [focusId, stepWindow.start]);

  return (
    <section className="trace-path-pane" aria-label="API execution path">
      <header className="trace-pane-heading">
        <span>Execution path</span>
        <small>{allSteps.length > 0 ? `${allSteps.length} reported` : `${staticPath.length} indexed`}</small>
      </header>
      <div className="trace-path-scroll" ref={scrollRef}>
        {error && allSteps.length === 0 ? (
          <div className="trace-pane-state" role="alert"><strong>Trace unavailable</strong><span>{error}</span></div>
        ) : indexedGraphLoading && allSteps.length === 0 ? (
          <div className="trace-pane-state" role="status"><span className="trace-path-loader" /><span>Loading the indexed API path…</span></div>
        ) : allSteps.length > 0 ? (
          <ol className="trace-path-list" aria-label="Reported safe points">
            {stepWindow.steps.map((step) => {
              const selected = step.id === selectedStepId;
              const position = step.id === currentStepId;
              const current = position && !replay;
              return (
                <li key={step.id}>
                  <button
                    type="button"
                    className={`trace-path-node${selected ? " is-selected" : ""}${current ? " is-current" : ""}${position && replay ? " is-replay-position" : ""}`}
                    data-trace-node-id={step.id}
                    data-debug-step-id={step.id}
                    aria-current={current ? "step" : undefined}
                    aria-label={`${step.label}, ${stepTypeLabel(step)}, ${sourceLabel(step)}, reported safe point ${step.sequence}`}
                    onClick={() => onSelect(step.id)}
                    onDoubleClick={() => step.source && onOpenSource(step.source, "pinned")}
                    onKeyDown={(event) => moveFocus(event, ".trace-path-node", onSelect)}
                  >
                    <span className="trace-path-icon" aria-hidden="true">{stepIcon(step)}</span>
                    <span className="trace-path-copy">
                      <small>{stepTypeLabel(step)}</small>
                      <strong>{step.label}</strong>
                      <span>{sourceLabel(step)}</span>
                    </span>
                    <span className="trace-path-sequence">#{step.sequence}</span>
                  </button>
                </li>
              );
            })}
          </ol>
        ) : staticPath.length > 0 ? (
          <>
            <p className="trace-indexed-note">Indexed path · lights appear only for reported runtime safe points.</p>
            <ol className="trace-path-list is-indexed" aria-label="Indexed API code path">
              {staticPath.map(({ node, relation }) => (
                <li key={node.id}>
                  <button
                    type="button"
                    className="trace-path-node is-indexed"
                    data-trace-node-id={node.id}
                    aria-label={`${node.label}, ${node.kind}, ${node.evidence} indexed evidence`}
                    onClick={() => node.source && onOpenSource(node.source)}
                    onDoubleClick={() => node.source && onOpenSource(node.source, "pinned")}
                    onKeyDown={(event) => moveFocus(event, ".trace-path-node", (nodeId) => {
                      const nextNode = staticPath.find((entry) => entry.node.id === nodeId)?.node;
                      if (nextNode?.source) onOpenSource(nextNode.source);
                    })}
                  >
                    <span className="trace-path-icon" aria-hidden="true">{nodeIcon(node)}</span>
                    <span className="trace-path-copy">
                      <small>{relation ?? flowStageForNode(node)}</small>
                      <strong>{node.label}</strong>
                      <span>{node.source ? `${node.source.relativePath}:${node.source.startLine}` : node.kind}</span>
                    </span>
                    <span className={`trace-evidence evidence-${node.evidence}`}>{node.evidence}</span>
                  </button>
                </li>
              ))}
            </ol>
          </>
        ) : (
          <div className="trace-pane-state">
            <BracketsCurly size={24} weight="thin" aria-hidden="true" />
            <strong>Send an API request</strong>
            <span>Reported code, service, branch, database, and response safe points will appear here in order.</span>
          </div>
        )}
      </div>
      {allSteps.length > 0 && (
        <footer className="trace-path-paging">
          <button type="button" onClick={onEarlier} disabled={stepWindow.omittedBefore === 0} aria-label="Earlier safe points">
            <CaretLeft size={12} aria-hidden="true" /> Earlier
          </button>
          <span>{stepWindow.start + 1}–{stepWindow.end} / {allSteps.length}</span>
          <button type="button" onClick={onLater} disabled={stepWindow.omittedAfter === 0} aria-label="Later safe points">
            Later <CaretRight size={12} aria-hidden="true" />
          </button>
        </footer>
      )}
    </section>
  );
}
