import {
  CaretDown,
  CaretRight,
  File,
  FileCode,
  Folder,
  FolderOpen,
  MagnifyingGlass,
  X,
} from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { getWorkspaceFiles } from "../lib/bridge";
import type { EditorOpenMode } from "../features/editor/model";
import type { WorkspaceFile, WorkspaceSummary } from "../types";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "./WorkbenchSidebarChrome";

const SEARCH_DEBOUNCE_MS = 180;
const SEARCH_RESULT_LIMIT = 200;
const MAX_SEARCH_QUERY_CHARS = 256;
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/g;

type SearchPhase = "idle" | "searching" | "ready" | "error";

interface SearchState {
  query: string;
  files: WorkspaceFile[];
  phase: SearchPhase;
  error?: string;
}

interface FileTreeNode {
  name: string;
  path: string;
  children: FileTreeNode[];
  file?: WorkspaceFile;
}

interface WorkspaceExplorerProps {
  workspace: WorkspaceSummary | null;
  files: WorkspaceFile[];
  activePath?: string;
  loading?: boolean;
  disabled?: boolean;
  onOpenFile: (file: WorkspaceFile, mode?: EditorOpenMode) => void;
  onOpenFolder: () => void;
  embedded?: boolean;
  title?: string;
  sectionTitle?: string;
  sectionDetail?: string;
}

export function buildTree(files: WorkspaceFile[]): FileTreeNode[] {
  const root: FileTreeNode = { name: "", path: "", children: [] };
  const nodesByPath = new Map<string, FileTreeNode>();

  for (const file of files) {
    const parts = file.relativePath.split("/").filter(Boolean);
    let parent = root;
    parts.forEach((part, index) => {
      const path = parts.slice(0, index + 1).join("/");
      let child = nodesByPath.get(path);
      if (!child) {
        child = { name: part, path, children: [] };
        nodesByPath.set(path, child);
        parent.children.push(child);
      }
      if (index === parts.length - 1) child.file = file;
      parent = child;
    });
  }

  const sort = (nodes: FileTreeNode[]): FileTreeNode[] => nodes
    .sort((a, b) => {
      if (Boolean(a.file) !== Boolean(b.file)) return a.file ? 1 : -1;
      return a.name.localeCompare(b.name);
    })
    .map((node) => ({ ...node, children: sort(node.children) }));

  return sort(root.children);
}

function boundedSearchQuery(value: string): string {
  return Array.from(value.replace(CONTROL_CHARACTERS, ""))
    .slice(0, MAX_SEARCH_QUERY_CHARS)
    .join("");
}

function matchingLoadedFiles(files: WorkspaceFile[], query: string): WorkspaceFile[] {
  const normalized = query.toLowerCase();
  return files.filter((file) => file.relativePath.toLowerCase().includes(normalized));
}

function addSearchExpansionPaths(target: Set<string>, files: WorkspaceFile[]) {
  for (const file of files) {
    const parts = file.relativePath.split("/").filter(Boolean);
    for (let index = 1; index < parts.length; index += 1) {
      target.add(parts.slice(0, index).join("/"));
    }
  }
}

function visibleTreePaths(nodes: FileTreeNode[], expanded: Set<string>, paths: string[] = []) {
  for (const node of nodes) {
    paths.push(node.path);
    if (!node.file && expanded.has(node.path)) visibleTreePaths(node.children, expanded, paths);
  }
  return paths;
}

function FileIcon({ language }: { language: string }) {
  const codeLanguages = new Set(["typescript", "javascript", "rust", "python", "go", "java"]);
  return codeLanguages.has(language.toLowerCase())
    ? <FileCode size={14} weight="duotone" aria-hidden="true" />
    : <File size={14} weight="duotone" aria-hidden="true" />;
}

interface TreeRowProps {
  node: FileTreeNode;
  depth: number;
  expanded: Set<string>;
  activePath?: string;
  onToggle: (path: string) => void;
  onOpenFile: (file: WorkspaceFile, mode?: EditorOpenMode) => void;
  tabStopPath?: string;
  onFocusPath: (path: string) => void;
  disabled: boolean;
}

function TreeRow({ node, depth, expanded, activePath, onToggle, onOpenFile, tabStopPath, onFocusPath, disabled }: TreeRowProps) {
  const isFolder = !node.file;
  const isOpen = expanded.has(node.path);

  return (
    <>
      <button
        type="button"
        role="treeitem"
        className={`tree-row${activePath === node.path ? " is-active" : ""}`}
        style={{ paddingLeft: 8 + depth * 14 }}
        onClick={() => isFolder ? onToggle(node.path) : onOpenFile(node.file!, "preview")}
        onDoubleClick={() => !isFolder && onOpenFile(node.file!, "pinned")}
        onKeyDown={(event) => {
          if (!isFolder && event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            onOpenFile(node.file!, "pinned");
          }
        }}
        title={node.path}
        aria-expanded={isFolder ? isOpen : undefined}
        aria-level={depth + 1}
        aria-selected={activePath === node.path}
        aria-keyshortcuts={isFolder ? undefined : "Meta+Enter Control+Enter"}
        tabIndex={tabStopPath === node.path ? 0 : -1}
        onFocus={() => onFocusPath(node.path)}
        disabled={disabled}
      >
        <span className="tree-chevron" aria-hidden="true">
          {isFolder ? (isOpen ? <CaretDown size={11} /> : <CaretRight size={11} />) : null}
        </span>
        <span className="tree-icon" aria-hidden="true">
          {isFolder
            ? (isOpen ? <FolderOpen size={14} weight="duotone" /> : <Folder size={14} weight="duotone" />)
            : <FileIcon language={node.file!.language} />}
        </span>
        <span className="tree-label">{node.name}</span>
        {node.file?.language && <span className="tree-language">{node.file.language.slice(0, 2).toUpperCase()}</span>}
      </button>
      {isFolder && isOpen && node.children.map((child) => (
        <TreeRow
          key={child.path}
          node={child}
          depth={depth + 1}
          expanded={expanded}
          activePath={activePath}
          onToggle={onToggle}
          onOpenFile={onOpenFile}
          tabStopPath={tabStopPath}
          onFocusPath={onFocusPath}
          disabled={disabled}
        />
      ))}
    </>
  );
}

export function WorkspaceExplorer({
  workspace,
  files,
  activePath,
  loading = false,
  disabled = false,
  onOpenFile,
  onOpenFolder,
  embedded = false,
  title = "Explorer",
  sectionTitle,
  sectionDetail,
}: WorkspaceExplorerProps) {
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState(() => new Set(["src", "src/api", "src/web", "migrations", "openapi"]));
  const [search, setSearch] = useState<SearchState>({ query: "", files: [], phase: "idle" });
  const [treeFocusPath, setTreeFocusPath] = useState<string>();
  const searchGeneration = useRef(0);
  const normalizedQuery = query.trim();
  const loadedMatches = useMemo(
    () => normalizedQuery ? matchingLoadedFiles(files, normalizedQuery) : files,
    [files, normalizedQuery],
  );

  useEffect(() => {
    const generation = ++searchGeneration.current;
    if (!workspace || !normalizedQuery) {
      setSearch({ query: "", files: [], phase: "idle" });
      return undefined;
    }

    setSearch({ query: normalizedQuery, files: loadedMatches, phase: "searching" });
    const timer = window.setTimeout(() => {
      void getWorkspaceFiles(normalizedQuery, SEARCH_RESULT_LIMIT).then(
        (nextFiles) => {
          if (searchGeneration.current !== generation) return;
          setSearch({ query: normalizedQuery, files: nextFiles, phase: "ready" });
        },
        (error: unknown) => {
          if (searchGeneration.current !== generation) return;
          setSearch({
            query: normalizedQuery,
            files: loadedMatches,
            phase: "error",
            error: error instanceof Error ? error.message : "Indexed-file search failed",
          });
        },
      );
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timer);
      if (searchGeneration.current === generation) searchGeneration.current += 1;
    };
  }, [loadedMatches, normalizedQuery, workspace?.id]);

  const searchIsCurrent = search.query === normalizedQuery;
  const searchPending = Boolean(normalizedQuery)
    && (!searchIsCurrent || search.phase === "searching");
  const searchFailed = searchIsCurrent && search.phase === "error";
  const visibleFiles = normalizedQuery && searchIsCurrent ? search.files : loadedMatches;
  const tree = useMemo(() => buildTree(visibleFiles), [visibleFiles]);
  const visibleExpanded = useMemo(() => {
    if (!normalizedQuery) return expanded;
    const next = new Set(expanded);
    addSearchExpansionPaths(next, visibleFiles);
    return next;
  }, [expanded, normalizedQuery, visibleFiles]);
  const treePaths = useMemo(() => visibleTreePaths(tree, visibleExpanded), [tree, visibleExpanded]);
  const tabStopPath = treePaths.includes(treeFocusPath ?? "") ? treeFocusPath
    : treePaths.includes(activePath ?? "") ? activePath
      : treePaths[0];

  const inventoryStatus = useMemo(() => {
    if (!workspace) return "";
    if (!normalizedQuery) {
      return `Showing ${files.length.toLocaleString()} of ${workspace.fileCount.toLocaleString()} indexed files`;
    }
    if (searchPending) return `Searching all ${workspace.fileCount.toLocaleString()} indexed files…`;
    if (searchFailed) return `Search unavailable · showing ${visibleFiles.length.toLocaleString()} loaded matches`;
    const prefix = visibleFiles.length >= SEARCH_RESULT_LIMIT ? "Showing first" : "Showing";
    return `${prefix} ${visibleFiles.length.toLocaleString()} matches · searched all ${workspace.fileCount.toLocaleString()} indexed`;
  }, [files.length, normalizedQuery, searchFailed, searchPending, visibleFiles.length, workspace]);

  const toggle = (path: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const handleTreeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const current = event.target instanceof Element
      ? event.target.closest<HTMLButtonElement>("[role='treeitem']")
      : null;
    if (!current || !event.currentTarget.contains(current)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("[role='treeitem']"));
    const index = items.indexOf(current);
    const focus = (next?: HTMLButtonElement) => {
      if (!next) return;
      setTreeFocusPath(next.title);
      next.focus();
    };
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const offset = event.key === "ArrowDown" ? 1 : -1;
      focus(items[Math.max(0, Math.min(items.length - 1, index + offset))]);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      focus(event.key === "Home" ? items[0] : items[items.length - 1]);
    } else if (event.key === "ArrowRight" && current.getAttribute("aria-expanded") !== null) {
      event.preventDefault();
      if (current.getAttribute("aria-expanded") === "false") current.click();
      else if (items[index + 1]?.title.startsWith(`${current.title}/`)) focus(items[index + 1]);
    } else if (event.key === "ArrowLeft") {
      const expandedState = current.getAttribute("aria-expanded");
      const parentPath = current.title.split("/").slice(0, -1).join("/");
      const parent = items.find((item) => item.title === parentPath);
      if (expandedState === "true" || parent) {
        event.preventDefault();
        if (expandedState === "true") current.click();
        else focus(parent);
      }
    }
  };

  return (
    <aside className={`workspace-panel${embedded ? " is-embedded" : ""}`} aria-label="Workspace explorer">
      <WorkbenchSidebarHeader title={title} />

      <label className="search-field" htmlFor="workspace-search">
        <MagnifyingGlass size={14} aria-hidden="true" />
        <input
          id="workspace-search"
          value={query}
          onChange={(event) => setQuery(boundedSearchQuery(event.target.value))}
          placeholder="Filter files · ⌘K"
          autoComplete="off"
          maxLength={MAX_SEARCH_QUERY_CHARS}
          spellCheck={false}
          aria-describedby={workspace ? "workspace-search-scope" : undefined}
          disabled={disabled}
        />
        {query && (
          <button type="button" onClick={() => setQuery("")} aria-label="Clear file filter" disabled={disabled}>
            <X size={12} />
          </button>
        )}
      </label>

      {workspace && (
        <WorkbenchSidebarSection
          title={sectionTitle ?? workspace.name}
          detail={sectionDetail}
          count={workspace.fileCount}
        />
      )}

      <div className="tree-scroll" role="tree" aria-busy={loading || searchPending || disabled} onKeyDown={handleTreeKeyDown}>
        {!workspace ? (
          <div className="panel-empty compact-empty">
            <FolderOpen size={24} weight="thin" aria-hidden="true" />
            <p>Open a folder to index its structure.</p>
            <button type="button" className="secondary-button" onClick={onOpenFolder} disabled={disabled}>Open folder</button>
          </div>
        ) : loading ? (
          <div className="tree-skeleton" aria-label="Loading workspace files">
            {Array.from({ length: 9 }, (_, index) => <span key={index} style={{ width: `${54 + (index % 4) * 9}%` }} />)}
          </div>
        ) : tree.length === 0 && searchPending ? (
          <div className="panel-empty compact-empty">
            <MagnifyingGlass size={22} weight="thin" aria-hidden="true" />
            <p>No files match the loaded list. Searching all indexed files…</p>
          </div>
        ) : tree.length === 0 && searchFailed ? (
          <div className="panel-empty compact-empty" role="alert">
            <MagnifyingGlass size={22} weight="thin" aria-hidden="true" />
            <p>Could not search all indexed files. {search.error}</p>
          </div>
        ) : tree.length === 0 ? (
          <div className="panel-empty compact-empty">
            <MagnifyingGlass size={22} weight="thin" aria-hidden="true" />
            <p>No files match “{query}”.</p>
          </div>
        ) : (
          tree.map((node) => (
            <TreeRow
              key={node.path}
              node={node}
              depth={0}
              expanded={visibleExpanded}
              activePath={activePath}
              onToggle={toggle}
              onOpenFile={onOpenFile}
              tabStopPath={tabStopPath}
              onFocusPath={setTreeFocusPath}
              disabled={disabled}
            />
          ))
        )}
      </div>

      {workspace && <span id="workspace-search-scope" className="visually-hidden" role="status" aria-live="polite">{inventoryStatus}</span>}
    </aside>
  );
}
