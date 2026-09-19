import { Code, DownloadSimple, GitBranch, MagnifyingGlass, X } from "@phosphor-icons/react";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { GraphCanvas } from "../../components/GraphCanvas";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "../../components/ui/Resizable";
import { ToggleGroup, ToggleGroupItem } from "../../components/ui/toggle-group";
import type { GraphNode, GraphSnapshot } from "../../types";
import { createGraphArtifactBundle, downloadGraphArtifactBundle } from "../graph/artifacts";
import { RELATIONSHIP_VIEWS, type RelationshipView } from "../graph/relationshipView";
import type { EditorOpenMode } from "../editor/model";

interface CodeMapWorkbenchProps {
  source: ReactNode;
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  loading?: boolean;
  error?: string | null;
  loadingMore?: boolean;
  relationshipView: RelationshipView;
  onRelationshipViewChange: (view: RelationshipView) => void;
  onSelectNode: (nodeId: string) => void;
  onOpenSource: (node: GraphNode, mode?: EditorOpenMode) => void;
  onTraceNode: (nodeId: string) => void;
  onLoadMore: () => void;
  graphSearchQuery: string;
  graphSearchLoading?: boolean;
  onGraphSearch: (query: string) => Promise<void> | void;
  workspaceName: string;
  applicationZoom?: number;
}

const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/g;
const MAX_GRAPH_QUERY_CHARS = 256;

function boundedGraphQuery(value: string): string {
  return Array.from(value.replace(CONTROL_CHARACTERS, ""))
    .slice(0, MAX_GRAPH_QUERY_CHARS)
    .join("");
}

export function CodeMapWorkbench({
  source,
  graph,
  selectedNodeId,
  loading,
  error,
  loadingMore,
  relationshipView,
  onRelationshipViewChange,
  onSelectNode,
  onOpenSource,
  onTraceNode,
  onLoadMore,
  graphSearchQuery,
  graphSearchLoading = false,
  onGraphSearch,
  workspaceName,
  applicationZoom,
}: CodeMapWorkbenchProps) {
  const [query, setQuery] = useState(graphSearchQuery);
  useEffect(() => setQuery(graphSearchQuery), [graphSearchQuery]);
  const selectNode = (nodeId: string) => {
    const node = graph.nodes.find((candidate) => candidate.id === nodeId);
    if (node?.source) onOpenSource(node, "preview");
    else onSelectNode(nodeId);
  };
  const submitSearch = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void onGraphSearch(query);
  };
  const clearSearch = () => {
    setQuery("");
    void onGraphSearch("");
  };
  const exportArtifacts = () => {
    downloadGraphArtifactBundle(createGraphArtifactBundle({
      workspaceSafeName: workspaceName,
      generatedAt: new Date().toISOString(),
      graph,
    }));
  };
  return (
    <section className="code-map-workbench" aria-label="Indexed source and workspace relationships">
      <ResizablePanelGroup
        orientation="horizontal"
        id="aone-code-map"
        defaultLayout={{ "code-map-source": 48, "code-map-graph": 52 }}
      >
        <ResizablePanel id="code-map-source" minSize="26%" defaultSize="48%" className="code-map-pane">
          <header className="code-map-pane-label">
            <span><Code size={12} /> File</span>
          </header>
          <div className="code-map-source-host">{source}</div>
        </ResizablePanel>
        <ResizableHandle aria-label="Resize source and graph panels" />
        <ResizablePanel id="code-map-graph" minSize="34%" defaultSize="52%" className="code-map-pane">
          <header className="code-map-pane-label">
            <span className="relationship-pane-title"><GitBranch size={12} /><span>Relationships</span></span>
            <ToggleGroup
              className="relationship-view-switcher"
              aria-label="Relationship category"
              value={[relationshipView]}
              spacing={0}
              size="sm"
              onValueChange={(value) => value[0] && onRelationshipViewChange(value[0] as RelationshipView)}
            >
              {RELATIONSHIP_VIEWS.map((view) => (
                <ToggleGroupItem
                  key={view.id}
                  value={view.id}
                  className={relationshipView === view.id ? "is-active" : ""}
                  aria-pressed={relationshipView === view.id}
                  aria-label={view.label}
                >
                  <span className="relationship-label-wide" aria-hidden="true">{view.label}</span>
                  <span className="relationship-label-compact" aria-hidden="true">
                    {view.id === "codepath" ? "Path" : view.id === "changes" ? "Git" : view.label}
                  </span>
                </ToggleGroupItem>
              ))}
            </ToggleGroup>
            <button
              type="button"
              className="graph-export-button"
              onClick={exportArtifacts}
              aria-label="Export current graph view"
              title="Download graph.json, GRAPH_REPORT.md, and graph.html for this bounded view"
            >
              <DownloadSimple size={12} /><span>Export</span>
            </button>
            <small>{graph.nodes.length}</small>
          </header>
          <div className="code-map-graph-host">
            <form className="graph-query-bar" role="search" onSubmit={submitSearch}>
              <MagnifyingGlass size={13} aria-hidden="true" />
              <input
                value={query}
                onChange={(event) => setQuery(boundedGraphQuery(event.target.value))}
                placeholder="Find nodes, paths, headings, or sentences"
                aria-label="Search graph relationships"
                maxLength={MAX_GRAPH_QUERY_CHARS}
                spellCheck={false}
              />
              {query && (
                <button type="button" onClick={clearSearch} aria-label="Clear graph search">
                  <X size={12} />
                </button>
              )}
              <button type="submit" disabled={!query.trim() || graphSearchLoading}>
                {graphSearchLoading ? "Searching…" : "Map"}
              </button>
            </form>
            {graphSearchQuery && (
              <div className="graph-query-scope" role="status">
                Relationship neighborhood for “{graphSearchQuery}”
              </div>
            )}
            {graph.truncated && (
              <div className="graph-bounds-warning" role="status">
                Bounded view — refine the search or select a node to inspect omitted relationships.
              </div>
            )}
            <GraphCanvas
              graph={graph}
              lens="all"
              selectedNodeId={selectedNodeId}
              onSelectNode={selectNode}
              onOpenSource={onOpenSource}
              onTraceNode={onTraceNode}
              onLoadMore={onLoadMore}
              loadingMore={loadingMore}
              loading={loading}
              error={error}
              applicationZoom={applicationZoom}
            />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </section>
  );
}
