import {
  BracketsCurly,
  CaretDown,
  CaretRight,
  Code,
  FileCode,
  GitBranch,
  MagnifyingGlass,
  Pulse,
  Warning,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { WorkspaceExplorer } from "../../components/WorkspaceExplorer";
import type { GraphNode, GraphSnapshot, SourceLocation, WorkspaceFile, WorkspaceSummary } from "../../types";
import { normalizedApiMethod, type ApiEndpointInventoryItem } from "../api-catalog/model";
import { useApiCatalog, type ApiCatalogPageLoader } from "../api-catalog/useApiCatalog";
import type { EditorOpenMode } from "../editor/model";
import { buildEndpointCallTree, type KnowledgeCallNode } from "./model";

interface KnowledgeNavigatorProps {
  workspace: WorkspaceSummary | null;
  workspaceGeneration: number;
  files: WorkspaceFile[];
  activePath?: string;
  graph: GraphSnapshot;
  selectedNodeId: string | null;
  loading?: boolean;
  disabled?: boolean;
  loadPage: ApiCatalogPageLoader;
  onOpenFolder: () => void;
  onOpenFile: (file: WorkspaceFile, mode?: EditorOpenMode) => void;
  onOpenSource: (location: SourceLocation, mode?: EditorOpenMode) => void;
  onSelectNode: (nodeId: string) => void;
  onTraceFlow: (endpoint: ApiEndpointInventoryItem, mode?: EditorOpenMode) => void;
}

function matchesEndpoint(endpoint: ApiEndpointInventoryItem, query: string): boolean {
  if (!query) return true;
  const text = [
    endpoint.method,
    endpoint.path,
    endpoint.label,
    endpoint.operationId,
    endpoint.handler?.label,
    endpoint.sourceFile,
  ].filter(Boolean).join(" ").toLowerCase();
  return text.includes(query.toLowerCase());
}

function sourceText(source?: SourceLocation): string {
  if (!source) return "No exact source";
  const file = source.relativePath.split("/").pop() ?? source.relativePath;
  return `${file}:${source.startLine}`;
}

function CallNode({
  node,
  path,
  depth,
  expanded,
  selectedNodeId,
  onToggle,
  onSelect,
}: {
  node: KnowledgeCallNode;
  path: string;
  depth: number;
  expanded: ReadonlySet<string>;
  selectedNodeId: string | null;
  onToggle: (path: string) => void;
  onSelect: (node: KnowledgeCallNode, mode?: EditorOpenMode) => void;
}) {
  const hasChildren = node.children.length > 0;
  const open = expanded.has(path);
  const unavailable = node.kind === "summary";
  return (
    <li className="knowledge-call-item">
      <div
        className={`knowledge-call-row${node.id === selectedNodeId ? " is-selected" : ""}${node.cycle ? " is-cycle" : ""}`}
        style={{ paddingLeft: `${8 + depth * 13}px` }}
      >
        <button
          type="button"
          className="knowledge-call-toggle"
          aria-label={hasChildren ? `${open ? "Collapse" : "Expand"} ${node.label}` : undefined}
          aria-expanded={hasChildren ? open : undefined}
          disabled={!hasChildren}
          onClick={() => hasChildren && onToggle(path)}
        >
          {hasChildren ? open ? <CaretDown size={11} /> : <CaretRight size={11} /> : <span />}
        </button>
        <button
          type="button"
          className="knowledge-call-target"
          disabled={unavailable}
          onClick={() => onSelect(node, "preview")}
          onDoubleClick={() => onSelect(node, "pinned")}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault();
              onSelect(node, "pinned");
            }
          }}
          aria-keyshortcuts={node.source ? "Meta+Enter Control+Enter" : undefined}
          title={`${node.relation ? `${node.relation} → ` : ""}${node.label}\n${sourceText(node.source)}\n${node.evidence} evidence`}
        >
          <span className="knowledge-call-icon">{node.source ? <FileCode size={12} /> : <BracketsCurly size={12} />}</span>
          <span className="knowledge-call-copy">
            <strong>{node.label}</strong>
            <small>
              {node.relation && <em>{node.relation}</em>}
              <span>{node.kind}</span>
              <span>{sourceText(node.source)}</span>
            </small>
          </span>
          <span className="knowledge-evidence" data-evidence={node.evidence}>{node.evidence}</span>
        </button>
      </div>
      {hasChildren && open && (
        <ul>
          {node.children.map((child, index) => (
            <CallNode
              key={`${path}:${child.id}:${index}`}
              node={child}
              path={`${path}/${child.id}:${index}`}
              depth={depth + 1}
              expanded={expanded}
              selectedNodeId={selectedNodeId}
              onToggle={onToggle}
              onSelect={onSelect}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export function KnowledgeNavigator({
  workspace,
  workspaceGeneration,
  files,
  activePath,
  graph,
  selectedNodeId,
  loading = false,
  disabled = false,
  loadPage,
  onOpenFolder,
  onOpenFile,
  onOpenSource,
  onSelectNode,
  onTraceFlow,
}: KnowledgeNavigatorProps) {
  const [view, setView] = useState<"calls" | "files">("calls");
  const [query, setQuery] = useState("");
  const [selectedEndpointId, setSelectedEndpointId] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const catalog = useApiCatalog(workspace?.id, workspaceGeneration, "", loadPage);
  const endpoints = useMemo(
    () => catalog.endpoints.filter((endpoint) => matchesEndpoint(endpoint, query.trim())),
    [catalog.endpoints, query],
  );
  const selectedEndpoint = catalog.endpoints.find((endpoint) => endpoint.id === selectedEndpointId);
  const selectedTree = useMemo(
    () => selectedEndpoint ? buildEndpointCallTree(selectedEndpoint, graph) : null,
    [graph, selectedEndpoint],
  );

  useEffect(() => {
    setSelectedEndpointId(null);
    setExpanded(new Set());
    setQuery("");
  }, [workspace?.id, workspaceGeneration]);

  useEffect(() => {
    if (!selectedTree) return;
    const next = new Set([`endpoint:${selectedTree.id}`]);
    const first = selectedTree.children[0];
    if (first) next.add(`endpoint:${selectedTree.id}/${first.id}:0`);
    setExpanded(next);
  }, [selectedTree?.id, graph]);

  const toggle = (path: string) => setExpanded((current) => {
    const next = new Set(current);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    return next;
  });
  const chooseEndpoint = (endpoint: ApiEndpointInventoryItem, mode: EditorOpenMode = "preview") => {
    setSelectedEndpointId(endpoint.id);
    onTraceFlow(endpoint, mode);
  };
  const chooseNode = (node: KnowledgeCallNode, mode: EditorOpenMode = "preview") => {
    onSelectNode(node.id);
    if (node.source) onOpenSource(node.source, mode);
  };

  return (
    <aside className="workspace-panel knowledge-navigator" aria-label="Indexed code knowledge">
      <div className="knowledge-heading">
        <span><GitBranch size={14} /> Code knowledge</span>
        <strong>{catalog.indexedTotal || graph.nodes.length}</strong>
      </div>
      <div className="knowledge-tabs" role="tablist" aria-label="Knowledge navigator view">
        <button type="button" role="tab" aria-selected={view === "calls"} onClick={() => setView("calls")}><Pulse size={12} /> API call paths</button>
        <button type="button" role="tab" aria-selected={view === "files"} onClick={() => setView("files")}><Code size={12} /> Indexed files</button>
      </div>
      {view === "files" ? (
        <WorkspaceExplorer
          embedded
          workspace={workspace}
          files={files}
          activePath={activePath}
          loading={loading}
          disabled={disabled}
          onOpenFile={onOpenFile}
          onOpenFolder={onOpenFolder}
        />
      ) : (
        <>
          <label className="knowledge-search">
            <MagnifyingGlass size={13} />
            <input value={query} onChange={(event) => setQuery(event.target.value.slice(0, 128))} placeholder="Find API, handler, function…" />
            {query && <button type="button" aria-label="Clear API call search" onClick={() => setQuery("")}><X size={11} /></button>}
          </label>
          <div className="knowledge-summary" role="status">
            <span>{endpoints.length.toLocaleString()} of {catalog.total.toLocaleString()} APIs</span>
            <span>{graph.nodes.length.toLocaleString()} loaded code nodes</span>
          </div>
          <div className="knowledge-scroll" aria-busy={catalog.loading || loading}>
            {catalog.error && <div className="knowledge-error" role="alert"><Warning size={14} />{catalog.error}<button type="button" onClick={catalog.retry}>Retry</button></div>}
            {!workspace ? (
              <div className="knowledge-empty">Open a folder to index its relationships.</div>
            ) : catalog.loading && catalog.endpoints.length === 0 ? (
              <div className="knowledge-empty">Loading the complete indexed API inventory…</div>
            ) : endpoints.length === 0 ? (
              <div className="knowledge-empty">No indexed APIs match this search.</div>
            ) : endpoints.map((endpoint) => {
              const selected = endpoint.id === selectedEndpointId;
              return (
                <article className={`knowledge-endpoint${selected ? " is-selected" : ""}`} key={endpoint.id}>
                  <button
                    type="button"
                    className="knowledge-endpoint-button"
                    onClick={() => chooseEndpoint(endpoint, "preview")}
                    onDoubleClick={() => chooseEndpoint(endpoint, "pinned")}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                        event.preventDefault();
                        chooseEndpoint(endpoint, "pinned");
                      }
                    }}
                    aria-keyshortcuts="Meta+Enter Control+Enter"
                    aria-expanded={selected}
                  >
                    <span className="api-method" data-method={normalizedApiMethod(endpoint)}>{normalizedApiMethod(endpoint)}</span>
                    <span><strong>{endpoint.path || endpoint.label}</strong><small>{endpoint.handler?.label ?? endpoint.operationId ?? "Handler unresolved"}</small></span>
                    <CaretRight size={11} className={selected ? "is-open" : ""} />
                  </button>
                  {selected && selectedTree && (
                    <ul className="knowledge-call-tree" aria-label={`${normalizedApiMethod(endpoint)} ${endpoint.path} nested code calls`}>
                      <CallNode
                        node={selectedTree}
                        path={`endpoint:${selectedTree.id}`}
                        depth={0}
                        expanded={expanded}
                        selectedNodeId={selectedNodeId}
                        onToggle={toggle}
                        onSelect={chooseNode}
                      />
                    </ul>
                  )}
                </article>
              );
            })}
          </div>
        </>
      )}
    </aside>
  );
}
