import {
  ArrowRight,
  Brain,
  CircleNotch,
  Code,
  FileCode,
  Link,
  Sparkle,
  Warning,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { groupNodeRelations, presentMetadata } from "../lib/graphPresentation";
import type { AiExplanation, GraphEdge, GraphNode, GraphSnapshot } from "../types";
import { EvidenceBadge } from "./EvidenceBadge";
import { SlidingTabs } from "./SlidingTabs";

type DetailTab = "sequence" | "ai";

interface EvidencePanelProps {
  graph: GraphSnapshot;
  selectedNode: GraphNode | null;
  explanation: AiExplanation | null;
  explaining: boolean;
  explanationError?: string | null;
  aiAvailable?: boolean;
  onExplain: (nodeId: string) => void;
  onConfigureAi?: () => void;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode) => void;
}

const evidenceBasisCopy: Record<GraphNode["evidence"], string> = {
  declared: "Declared by parsed source structure.",
  resolved: "Resolved through static symbols or configuration.",
  observed: "Captured from this local runtime session.",
  inferred: "Inferred from available evidence; not observed behavior.",
};
const VISIBLE_RELATIONS_PER_DIRECTION = 4;

interface RelationRowProps {
  edge: GraphEdge;
  currentId: string;
  nodes: Map<string, GraphNode>;
  onSelect: (nodeId: string) => void;
}

function RelationRow({ edge, currentId, nodes, onSelect }: RelationRowProps) {
  const outgoing = edge.source === currentId;
  const relatedId = outgoing ? edge.target : edge.source;
  const node = nodes.get(relatedId);
  if (!node) return null;
  return (
    <button type="button" className="relation-row" onClick={() => onSelect(relatedId)}>
      <span className={`relation-direction${outgoing ? "" : " is-incoming"}`}><ArrowRight size={11} /></span>
      <span className="relation-copy">
        <strong>{node.label}</strong>
        <small>{edge.kind}{edge.confidence !== undefined && edge.confidence < 1 ? ` / ${Math.round(edge.confidence * 100)}%` : ""}</small>
      </span>
      <EvidenceBadge evidence={edge.evidence} compact />
    </button>
  );
}

export function EvidencePanel({
  graph,
  selectedNode,
  explanation,
  explaining,
  explanationError,
  aiAvailable = true,
  onExplain,
  onConfigureAi,
  onSelectNode,
  onOpenSource,
}: EvidencePanelProps) {
  const [tab, setTab] = useState<DetailTab>("sequence");
  const nodes = useMemo(() => new Map(graph.nodes.map((node) => [node.id, node])), [graph.nodes]);
  const relations = useMemo(
    () => selectedNode
      ? graph.edges.filter((edge) => edge.source === selectedNode.id || edge.target === selectedNode.id)
      : [],
    [graph.edges, selectedNode],
  );
  const groupedRelations = useMemo(
    () => selectedNode
      ? groupNodeRelations(relations, selectedNode.id, nodes)
      : { incoming: [], outgoing: [] },
    [nodes, relations, selectedNode],
  );
  const metadata = useMemo(() => presentMetadata(selectedNode?.metadata), [selectedNode]);
  const visibleOutgoing = groupedRelations.outgoing.slice(0, VISIBLE_RELATIONS_PER_DIRECTION);
  const visibleIncoming = groupedRelations.incoming.slice(0, VISIBLE_RELATIONS_PER_DIRECTION);
  const hiddenRelations = relations.length - visibleOutgoing.length - visibleIncoming.length;

  useEffect(() => {
    if (!selectedNode) setTab("sequence");
  }, [selectedNode]);

  return (
    <aside className="evidence-panel" aria-label="Selected node evidence and relationships">
      <div className="panel-heading evidence-heading">
        <h2>Evidence</h2>
        {selectedNode && <EvidenceBadge evidence={selectedNode.evidence} compact />}
      </div>

      <SlidingTabs
        value={tab}
        onChange={setTab}
        label="Inspector view"
        options={[
          { value: "sequence", label: "Sequence", count: relations.length },
          { value: "ai", label: "AI Recommendation" },
        ]}
        className="inspector-tabs"
      />

      <div className="evidence-panel-clip">
        <div className="t-panel-slide evidence-content" data-open={selectedNode ? "true" : "false"}>
          {!selectedNode ? null : tab === "sequence" ? (
            <div className="inspector-scroll">
              <section className="node-summary-block">
                <span className="inspector-claim-label">Current step</span>
                <div className="node-kind-line">
                  <span>{selectedNode.kind}</span>
                  <span>{selectedNode.language ?? "unknown"}</span>
                </div>
                <h3>{selectedNode.label}</h3>
                {selectedNode.evidence === "inferred" && (
                  <div className="inference-warning"><Warning size={13} />This is a hypothesis, not observed behavior.</div>
                )}
                {selectedNode.source && (
                  <button type="button" className="source-jump" onClick={() => onOpenSource(selectedNode)}>
                    <span>{selectedNode.source.relativePath}</span>
                    <strong>L{selectedNode.source.startLine}{selectedNode.source.endLine !== selectedNode.source.startLine ? `-${selectedNode.source.endLine}` : ""}</strong>
                    <Code size={13} />
                  </button>
                )}
              </section>

              <section className="inspector-section relations-section">
                <h4><Link size={13} /> Relationships <span>{relations.length}</span></h4>
                {relations.length === 0 ? (
                  <p className="section-empty">No relationships in the current graph scope.</p>
                ) : (
                  <div className="relation-groups">
                    {groupedRelations.outgoing.length > 0 && (
                      <section className="relation-group" aria-labelledby="outgoing-relations-heading">
                        <h5 id="outgoing-relations-heading">Outgoing <span>{groupedRelations.outgoing.length}</span></h5>
                        {visibleOutgoing.map((edge) => (
                          <RelationRow key={edge.id} edge={edge} currentId={selectedNode.id} nodes={nodes} onSelect={onSelectNode} />
                        ))}
                      </section>
                    )}
                    {groupedRelations.incoming.length > 0 && (
                      <section className="relation-group" aria-labelledby="incoming-relations-heading">
                        <h5 id="incoming-relations-heading">Incoming <span>{groupedRelations.incoming.length}</span></h5>
                        {visibleIncoming.map((edge) => (
                          <RelationRow key={edge.id} edge={edge} currentId={selectedNode.id} nodes={nodes} onSelect={onSelectNode} />
                        ))}
                      </section>
                    )}
                    {hiddenRelations > 0 && <p className="relation-overflow">{hiddenRelations} more relationships</p>}
                  </div>
                )}
              </section>

              <details className="inspector-details">
                <summary><FileCode size={13} /> Evidence details</summary>
                <section className="inspector-section evidence-basis-section">
                  <h4>Evidence basis</h4>
                  <p className="evidence-basis-copy">{evidenceBasisCopy[selectedNode.evidence]}</p>
                  {metadata.keyFacts.length > 0 && (
                    <>
                      <h4>Key facts</h4>
                      <dl className="metadata-grid">
                        {metadata.keyFacts.map((entry) => (
                          <div key={entry.key}><dt>{entry.label}</dt><dd>{entry.value}</dd></div>
                        ))}
                      </dl>
                    </>
                  )}
                  {metadata.rawEvidence.length > 0 && (
                    <details className="raw-evidence-disclosure">
                      <summary>
                        <span>Raw evidence</span>
                        <strong>{metadata.rawEvidence.length} {metadata.rawEvidence.length === 1 ? "field" : "fields"}</strong>
                      </summary>
                      <dl className="metadata-grid raw-metadata-grid">
                        {metadata.rawEvidence.map((entry) => (
                          <div key={entry.key}><dt>{entry.label}</dt><dd>{entry.value}</dd></div>
                        ))}
                      </dl>
                    </details>
                  )}
                </section>
              </details>
            </div>
          ) : (
            <div className="inspector-scroll ai-panel-content">
              <div className="ai-origin">
                <Sparkle size={14} weight="bold" />
                <div><strong>AI output is inferred</strong><span>It cannot become observed evidence.</span></div>
              </div>
              {!aiAvailable ? (
                <div className="ai-empty">
                  <Brain size={28} weight="thin" />
                  <p>Code-only mode keeps indexing, search, graph export, and source navigation local. Connect AI only when you want an inferred explanation.</p>
                  {onConfigureAi && (
                    <button type="button" className="primary-button" onClick={onConfigureAi}>
                      Configure optional AI
                    </button>
                  )}
                </div>
              ) : !explanation && !explaining && !explanationError && (
                <div className="ai-empty">
                  <Brain size={28} weight="thin" />
                  <p>Explain this selection using its bounded two-hop graph corridor. Sentence-level support, contradiction, and causality remain inferred.</p>
                  <button type="button" className="primary-button" onClick={() => onExplain(selectedNode.id)}>
                    <Sparkle size={13} weight="bold" /> Explain selection
                  </button>
                </div>
              )}
              {aiAvailable && explaining && (
                <div className="ai-loading" aria-live="polite">
                  <CircleNotch className="spin" size={18} />
                  <span>Building an evidence-bounded explanation…</span>
                </div>
              )}
              {aiAvailable && explanationError && (
                <div className="ai-error" role="alert"><Warning size={15} />{explanationError}</div>
              )}
              {aiAvailable && explanation && !explaining && (
                <div className="ai-explanation">
                  <p className="ai-summary">{explanation.answer}</p>
                  <h4>Evidence used</h4>
                  <div className="ai-related">
                    <span>{explanation.model}</span>
                    {explanation.evidence.map((reference) => {
                      const node = nodes.get(reference.id);
                      return (
                        <button
                          type="button"
                          key={`${reference.kind}:${reference.id}`}
                          onClick={() => node && onSelectNode(node.id)}
                          disabled={!node}
                          title={`${reference.kind} / ${reference.evidence}`}
                        >
                          {reference.label}<EvidenceBadge evidence={reference.evidence} compact />
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {!selectedNode && (
          <div className="panel-empty inspector-empty">
            <Link size={26} weight="thin" />
            <p>Select a graph node or source file to inspect its evidence.</p>
            <span>⌘↵ opens the selected source range.</span>
          </div>
        )}
      </div>
    </aside>
  );
}
