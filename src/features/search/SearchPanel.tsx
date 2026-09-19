import {
  CaretDown,
  CaretRight,
  FileCode,
  FolderOpen,
  MagnifyingGlass,
  Warning,
  X,
} from "@phosphor-icons/react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";
import { WorkbenchSidebarHeader, WorkbenchSidebarSection } from "../../components/WorkbenchSidebarChrome";
import { searchWorkspace } from "../../lib/searchBridge";
import { failureMessage } from "../../lib/errorMessage";
import type { SourceLocation } from "../../types";
import type { EditorOpenMode } from "../editor/model";
import {
  groupSearchMatches,
  type SearchMatch,
  type SearchResultGroup,
  type SearchWorkspaceResult,
} from "./model";

const SEARCH_DEBOUNCE_MS = 170;
const SEARCH_RESULT_LIMIT = 500;
const MAX_SEARCH_QUERY_CHARS = 256;
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/g;

type SearchPhase = "idle" | "searching" | "ready" | "error";

interface SearchState {
  phase: SearchPhase;
  query: string;
  result?: SearchWorkspaceResult;
  error?: string;
}

interface SearchPanelProps {
  workspaceId?: string;
  workspaceName?: string;
  workspaceGeneration: number;
  disabled?: boolean;
  onOpenMatch: (location: SourceLocation, mode?: EditorOpenMode) => void;
  onOpenFolder: () => void;
}

function boundedQuery(value: string) {
  return Array.from(value.replace(CONTROL_CHARACTERS, ""))
    .slice(0, MAX_SEARCH_QUERY_CHARS)
    .join("");
}

function resultCountLabel(result: SearchWorkspaceResult) {
  if (result.truncated && result.totalMatches === 0) return "0 observed";
  return `${result.totalMatches.toLocaleString()}${result.truncated ? "+" : ""}`;
}

function asciiLowerCase(value: string) {
  return value.replace(/[A-Z]/g, (character) =>
    String.fromCharCode(character.charCodeAt(0) + 32));
}

function highlightedPreview(match: SearchMatch): ReactNode {
  if (!match.matchText) return match.preview;
  const exactOffset = match.preview.indexOf(match.matchText);
  const offset = exactOffset >= 0
    ? exactOffset
    : asciiLowerCase(match.preview).indexOf(asciiLowerCase(match.matchText));
  if (offset < 0) return match.preview;
  return (
    <>
      {match.preview.slice(0, offset)}
      <mark>{match.preview.slice(offset, offset + match.matchText.length)}</mark>
      {match.preview.slice(offset + match.matchText.length)}
    </>
  );
}

function MatchKind({ match }: { match: SearchMatch }) {
  const label = match.kind === "endpoint" ? "API"
    : match.kind.charAt(0).toUpperCase() + match.kind.slice(1);
  return <span className={`search-kind is-${match.kind}`}>{label}</span>;
}

interface ResultGroupProps {
  group: SearchResultGroup;
  expanded: boolean;
  disabled: boolean;
  tabStopKey?: string;
  onFocusKey: (key: string) => void;
  onToggle: (path: string) => void;
  onOpenMatch: (location: SourceLocation, mode?: EditorOpenMode) => void;
}

function ResultGroup({ group, expanded, disabled, tabStopKey, onFocusKey, onToggle, onOpenMatch }: ResultGroupProps) {
  const groupKey = `group:${group.relativePath}`;
  return (
    <div className="search-result-group" role="none">
      <button
        type="button"
        role="treeitem"
        className="search-file-row"
        aria-expanded={expanded}
        aria-level={1}
        data-search-key={groupKey}
        tabIndex={tabStopKey === groupKey ? 0 : -1}
        title={group.relativePath}
        disabled={disabled}
        onFocus={() => onFocusKey(groupKey)}
        onClick={() => onToggle(group.relativePath)}
      >
        {expanded ? <CaretDown size={11} /> : <CaretRight size={11} />}
        <FileCode size={13} weight="duotone" />
        <strong>{group.fileName}</strong>
        {group.directory && <span>{group.directory}</span>}
        <b>{group.matches.length}</b>
      </button>
      {expanded && (
        <div role="group">
          {group.matches.map((match) => (
            <button
              key={match.key}
              type="button"
              role="treeitem"
              aria-level={2}
              className="search-match-row"
              data-search-key={match.key}
              data-parent-key={groupKey}
              tabIndex={tabStopKey === match.key ? 0 : -1}
              title={`${match.relativePath}:${match.startLine}:${match.startColumn}`}
              disabled={disabled}
              onFocus={() => onFocusKey(match.key)}
              onClick={() => onOpenMatch(match, "preview")}
              onDoubleClick={() => onOpenMatch(match, "pinned")}
              onKeyDown={(event) => {
                if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                  event.preventDefault();
                  onOpenMatch(match, "pinned");
                }
              }}
              aria-keyshortcuts="Meta+Enter Control+Enter"
            >
              <span className="search-match-line">{match.startLine}</span>
              <span className="search-match-preview">{highlightedPreview(match)}</span>
              <MatchKind match={match} />
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function SearchPanel({
  workspaceId,
  workspaceName,
  workspaceGeneration,
  disabled = false,
  onOpenMatch,
  onOpenFolder,
}: SearchPanelProps) {
  const [query, setQuery] = useState("");
  const [search, setSearch] = useState<SearchState>({ phase: "idle", query: "" });
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [focusKey, setFocusKey] = useState<string>();
  const requestGeneration = useRef(0);
  const workspaceRef = useRef({ workspaceId, workspaceGeneration });
  const inputRef = useRef<HTMLInputElement>(null);
  workspaceRef.current = { workspaceId, workspaceGeneration };
  const normalizedQuery = query.trim();

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    requestGeneration.current += 1;
    setQuery("");
    setSearch({ phase: "idle", query: "" });
    setExpanded(new Set());
    setFocusKey(undefined);
  }, [workspaceGeneration, workspaceId]);

  useEffect(() => {
    const generation = ++requestGeneration.current;
    const target = { workspaceId, workspaceGeneration };
    if (!workspaceId || !normalizedQuery) {
      setSearch({ phase: "idle", query: normalizedQuery });
      return undefined;
    }
    setSearch({ phase: "searching", query: normalizedQuery });
    const timer = window.setTimeout(() => {
      const workspaceIsCurrent = () => workspaceRef.current.workspaceId === target.workspaceId
        && workspaceRef.current.workspaceGeneration === target.workspaceGeneration;
      void searchWorkspace({
        workspaceId,
        query: normalizedQuery,
        limit: SEARCH_RESULT_LIMIT,
      }).then(
        (result) => {
          if (generation !== requestGeneration.current || !workspaceIsCurrent()) return;
          setSearch({ phase: "ready", query: normalizedQuery, result });
          setExpanded(new Set(result.matches.map((match) => match.relativePath)));
          setFocusKey(result.matches[0] ? `group:${result.matches[0].relativePath}` : undefined);
        },
        (failure: unknown) => {
          if (generation !== requestGeneration.current || !workspaceIsCurrent()) return;
          setSearch({
            phase: "error",
            query: normalizedQuery,
            error: failureMessage(failure, "Workspace search failed"),
          });
        },
      );
    }, SEARCH_DEBOUNCE_MS);
    return () => window.clearTimeout(timer);
  }, [normalizedQuery, workspaceGeneration, workspaceId]);

  const result = search.query === normalizedQuery ? search.result : undefined;
  const groups = useMemo(() => groupSearchMatches(result?.matches ?? []), [result?.matches]);
  const fallbackFocusKey = groups[0] ? `group:${groups[0].relativePath}` : undefined;
  const visibleKeys = useMemo(() => groups.flatMap((group) => {
    const groupKey = `group:${group.relativePath}`;
    return expanded.has(group.relativePath)
      ? [groupKey, ...group.matches.map((match) => match.key)]
      : [groupKey];
  }), [expanded, groups]);
  const tabStopKey = focusKey && visibleKeys.includes(focusKey) ? focusKey : fallbackFocusKey;

  const toggle = (relativePath: string) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(relativePath)) next.delete(relativePath);
      else next.add(relativePath);
      return next;
    });
  };

  const handleTreeKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    const current = event.target instanceof Element
      ? event.target.closest<HTMLButtonElement>("[role='treeitem']")
      : null;
    if (!current || !event.currentTarget.contains(current)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("[role='treeitem']:not(:disabled)"));
    const index = items.indexOf(current);
    const focus = (next?: HTMLButtonElement) => {
      if (!next) return;
      setFocusKey(next.dataset.searchKey);
      next.focus();
    };
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const offset = event.key === "ArrowDown" ? 1 : -1;
      focus(items[Math.max(0, Math.min(items.length - 1, index + offset))]);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      focus(event.key === "Home" ? items[0] : items.at(-1));
    } else if (event.key === "ArrowRight" && current.getAttribute("aria-expanded") !== null) {
      event.preventDefault();
      if (current.getAttribute("aria-expanded") === "false") current.click();
      else focus(items[index + 1]);
    } else if (event.key === "ArrowLeft") {
      const parentKey = current.dataset.parentKey;
      const parent = parentKey
        ? items.find((item) => item.dataset.searchKey === parentKey)
        : undefined;
      if (current.getAttribute("aria-expanded") === "true" || parent) {
        event.preventDefault();
        if (parent) focus(parent);
        else current.click();
      }
    }
  };

  return (
    <aside className="workspace-panel search-panel" aria-label="Search">
      <WorkbenchSidebarHeader title="Search" />
      <label className="search-field search-query-field" htmlFor="workspace-content-search">
        <MagnifyingGlass size={14} aria-hidden="true" />
        <input
          ref={inputRef}
          id="workspace-content-search"
          value={query}
          onChange={(event) => setQuery(boundedQuery(event.target.value))}
          placeholder="Search · ⇧⌘F"
          autoComplete="off"
          autoFocus
          maxLength={MAX_SEARCH_QUERY_CHARS}
          spellCheck={false}
          disabled={!workspaceId || disabled}
          aria-describedby={workspaceId ? "workspace-content-search-scope" : undefined}
        />
        {query && <button type="button" onClick={() => setQuery("")} aria-label="Clear search" disabled={disabled}><X size={12} /></button>}
      </label>
      <WorkbenchSidebarSection
        title="Results"
        detail={workspaceName ?? "No folder"}
        count={result ? resultCountLabel(result) : undefined}
      />
      <div className="search-results" role="tree" aria-label="Workspace search results" aria-busy={search.phase === "searching" || disabled} onKeyDown={handleTreeKeyDown}>
        {!workspaceId ? (
          <div className="panel-empty compact-empty"><FolderOpen size={24} weight="thin" /><p>Open a folder to search its indexed source.</p><button type="button" className="secondary-button" onClick={onOpenFolder} disabled={disabled}>Open folder</button></div>
        ) : !normalizedQuery ? (
          <div className="panel-empty compact-empty"><MagnifyingGlass size={24} weight="thin" /><p>Search safe indexed source, paths, symbols, APIs, events, headings, and sentences.</p></div>
        ) : search.phase === "searching" ? (
          <div className="search-skeleton" role="status"><span>Searching indexed workspace…</span>{Array.from({ length: 7 }, (_, index) => <i key={index} style={{ width: `${58 + (index % 3) * 12}%` }} />)}</div>
        ) : search.phase === "error" ? (
          <div className="panel-empty compact-empty" role="alert"><Warning size={22} weight="duotone" /><p>{search.error}</p></div>
        ) : groups.length === 0 ? (
          <div className="panel-empty compact-empty"><MagnifyingGlass size={22} weight="thin" /><p>{result?.truncated
            ? `No match observed for “${normalizedQuery}” inside the bounded search window. Refine the query or type at least 3 characters.`
            : `No indexed matches for “${normalizedQuery}”.`}</p></div>
        ) : groups.map((group) => (
          <ResultGroup
            key={group.relativePath}
            group={group}
            expanded={expanded.has(group.relativePath)}
            disabled={disabled}
            tabStopKey={tabStopKey}
            onFocusKey={setFocusKey}
            onToggle={toggle}
            onOpenMatch={onOpenMatch}
          />
        ))}
      </div>
      {workspaceId && (
        <footer className="workspace-footer" id="workspace-content-search-scope" role="status" aria-live="polite">
          {search.phase === "searching" ? <span>Searching…</span> : result ? <span>{resultCountLabel(result)} matches in {groups.length.toLocaleString()} {groups.length === 1 ? "file" : "files"}</span> : <span>Indexed search</span>}
          {result?.truncated && <span>{result.matches.length === 0
            ? "Bounded subset"
            : `First ${result.matches.length.toLocaleString()}`}</span>}
        </footer>
      )}
    </aside>
  );
}
