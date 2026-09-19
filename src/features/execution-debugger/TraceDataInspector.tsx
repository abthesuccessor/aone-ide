import { BracketsCurly, Code, Database, Warning } from "@phosphor-icons/react";
import { useMemo, useState, type ReactNode } from "react";
import type { ApiHeader, ApiRequest, ApiResponse, SourceLocation } from "../../types";
import { parseApiBody } from "../api-client/model";
import type { EditorOpenMode } from "../editor/model";
import type { DataShapePreview, DebugExecutionStep } from "./contracts";

type InspectorView = "shape" | "request" | "response";
type FieldState = "added" | "changed" | "same" | "removed";

const SECRET_HEADER = /authorization|cookie|token|secret|api[-_]?key/i;
const MAX_JSON_ENTRIES = 80;
const MAX_JSON_DEPTH = 7;

function scalar(value: unknown): ReactNode {
  if (value === null) return <span className="json-null">null</span>;
  if (typeof value === "string") return <span className="json-string">{JSON.stringify(value.length > 500 ? `${value.slice(0, 500)}…` : value)}</span>;
  if (typeof value === "number") return <span className="json-number">{String(value)}</span>;
  if (typeof value === "boolean") return <span className="json-boolean">{String(value)}</span>;
  return <span className="json-null">{String(value)}</span>;
}

function JsonNode({ name, value, depth }: { name?: string; value: unknown; depth: number }) {
  if (value === null || typeof value !== "object") {
    return <div className="json-leaf">{name !== undefined && <span className="json-key">{name}: </span>}{scalar(value)}</div>;
  }
  if (depth >= MAX_JSON_DEPTH) {
    return <div className="json-leaf">{name !== undefined && <span className="json-key">{name}: </span>}<span className="json-null">… depth limited</span></div>;
  }
  const entries = Array.isArray(value)
    ? value.map((item, index) => [String(index), item] as const)
    : Object.entries(value as Record<string, unknown>);
  const visible = entries.slice(0, MAX_JSON_ENTRIES);
  const bracket = Array.isArray(value) ? ["[", "]"] : ["{", "}"];
  return (
    <details className="json-branch" open={depth < 2}>
      <summary>
        {name !== undefined && <span className="json-key">{name}: </span>}
        <span>{bracket[0]} {entries.length} {Array.isArray(value) ? "items" : "fields"} {bracket[1]}</span>
      </summary>
      <div className="json-children">
        {visible.map(([key, child]) => <JsonNode key={key} name={key} value={child} depth={depth + 1} />)}
        {entries.length > visible.length && <div className="json-leaf json-more">… {entries.length - visible.length} more</div>}
      </div>
    </details>
  );
}

function BodyView({ body, empty }: { body?: string; empty: string }) {
  const parsed = useMemo(() => parseApiBody(body), [body]);
  if (parsed.kind === "empty") return <div className="trace-data-empty">{empty}</div>;
  if (parsed.kind === "text") return <pre className="trace-data-raw">{String(parsed.value)}</pre>;
  return <div className="json-tree"><JsonNode value={parsed.value} depth={0} /></div>;
}

function visibleHeaders(headers: ApiHeader[]) {
  return headers.map((header) => ({
    name: header.name,
    value: SECRET_HEADER.test(header.name) ? "[hidden in trace view]" : header.value,
  }));
}

function fieldRows(current?: DataShapePreview, previous?: DataShapePreview) {
  const before = new Map(previous?.fields.map((field) => [field.name, field]) ?? []);
  const after = new Map(current?.fields.map((field) => [field.name, field]) ?? []);
  const rows: Array<{ name: string; type: string; nullable: boolean; state: FieldState }> = [];
  for (const field of current?.fields ?? []) {
    const prior = before.get(field.name);
    const changed = prior && (prior.valueType !== field.valueType || prior.nullable !== field.nullable);
    rows.push({
      name: field.name,
      type: field.valueType,
      nullable: field.nullable,
      state: !prior ? "added" : changed ? "changed" : "same",
    });
  }
  for (const field of previous?.fields ?? []) {
    if (!after.has(field.name)) {
      rows.push({ name: field.name, type: field.valueType, nullable: field.nullable, state: "removed" });
    }
  }
  return rows;
}

function ShapeView({
  step,
  previousStep,
  onOpenSource,
  breakpointArmed,
  onToggleBreakpoint,
}: {
  step?: DebugExecutionStep;
  previousStep?: DebugExecutionStep;
  onOpenSource: (source: SourceLocation, mode?: EditorOpenMode) => void;
  breakpointArmed: boolean;
  onToggleBreakpoint?: () => void;
}) {
  const preview = step?.preview;
  const rows = fieldRows(preview, previousStep?.preview);
  if (!step) {
    return <div className="trace-data-empty"><BracketsCurly size={24} weight="thin" /><span>Select a reported safe point to inspect its data shape.</span></div>;
  }
  return (
    <div className="trace-shape-view">
      <section className="trace-step-summary">
        <div className="trace-step-title">
          <span>{step.kind}</span>
          <strong>{step.label}</strong>
        </div>
        <dl>
          {step.operation && <div><dt>Operation</dt><dd>{step.operation}</dd></div>}
          {step.resource && <div><dt>Resource</dt><dd>{step.resource}</dd></div>}
          {step.branchOutcome && <div><dt>Branch</dt><dd>{step.branchOutcome}</dd></div>}
          {preview?.typeName && <div><dt>Type</dt><dd>{preview.typeName}</dd></div>}
          {preview?.rowCount !== undefined && <div><dt>Rows</dt><dd>{preview.rowCount}</dd></div>}
          {preview?.itemCount !== undefined && <div><dt>Items</dt><dd>{preview.itemCount}</dd></div>}
          {preview?.nullCount !== undefined && <div><dt>Nulls</dt><dd>{preview.nullCount}</dd></div>}
        </dl>
        {step.source && (
          <button type="button" className="trace-source-jump" onClick={() => onOpenSource(step.source!)}>
            <Code size={13} aria-hidden="true" />
            <span>{step.source.relativePath}</span>
            <strong>L{step.source.startLine}</strong>
          </button>
        )}
        {step.protocol === "AONE_DEBUG_V1" && step.reportedStepId && onToggleBreakpoint && (
          <button
            type="button"
            className={`trace-breakpoint${breakpointArmed ? " is-armed" : ""}`}
            aria-pressed={breakpointArmed}
            onClick={onToggleBreakpoint}
          >
            <i aria-hidden="true" />
            {breakpointArmed ? "Remove cooperative breakpoint" : "Break on this reported safe point"}
          </button>
        )}
      </section>

      <section className="trace-shape-fields" aria-label="Reported data shape">
        <header><Database size={13} aria-hidden="true" /><span>Shape changes</span></header>
        {rows.length > 0 ? (
          <table>
            <thead><tr><th>Field</th><th>Type</th><th>Change</th></tr></thead>
            <tbody>{rows.map((field) => (
              <tr key={`${field.state}:${field.name}`} className={`is-${field.state}`}>
                <td>{field.name}{field.nullable ? "?" : ""}</td>
                <td>{field.type}</td>
                <td>{field.state}</td>
              </tr>
            ))}</tbody>
          </table>
        ) : (
          <p>No field shape was reported at this safe point.</p>
        )}
        {preview?.truncated && <small>Shape was truncated by the debug protocol.</small>}
      </section>
      <p className="trace-data-boundary">Runtime internals are shape and count metadata only. Request and response tabs show the values available to the API client.</p>
    </div>
  );
}

function RequestView({ request, sending }: { request: ApiRequest | null; sending: boolean }) {
  if (!request) return <div className="trace-data-empty">Send a request to capture its payload.</div>;
  return (
    <div className="trace-message-view">
      <header><strong>{request.method}</strong><span>{request.url}</span>{sending && <small>waiting</small>}</header>
      <BodyView body={request.body} empty="This request has no body." />
      <details className="trace-header-list">
        <summary>{request.headers.length} request headers</summary>
        <dl>{visibleHeaders(request.headers).map((header, index) => (
          <div key={`${header.name}-${index}`}><dt>{header.name}</dt><dd>{header.value}</dd></div>
        ))}</dl>
      </details>
    </div>
  );
}

function ResponseView({ response, error }: { response: ApiResponse | null; error: string | null }) {
  if (error) return <div className="trace-data-error" role="alert"><Warning size={16} />{error}</div>;
  if (!response) return <div className="trace-data-empty">The response will appear after the request completes.</div>;
  return (
    <div className="trace-message-view">
      <header>
        <strong className={response.status < 400 ? "is-ok" : "is-error"}>{response.status} {response.statusText}</strong>
        <span>{response.durationMs} ms</span>
        {response.truncated && <small>truncated</small>}
      </header>
      <BodyView body={response.body} empty="The response has no body." />
      <details className="trace-header-list">
        <summary>{response.headers.length} response headers</summary>
        <dl>{response.headers.map((header, index) => (
          <div key={`${header.name}-${index}`}><dt>{header.name}</dt><dd>{header.value}</dd></div>
        ))}</dl>
      </details>
    </div>
  );
}

interface TraceDataInspectorProps {
  step?: DebugExecutionStep;
  previousStep?: DebugExecutionStep;
  request: ApiRequest | null;
  response: ApiResponse | null;
  requestError: string | null;
  sending: boolean;
  onOpenSource: (source: SourceLocation, mode?: EditorOpenMode) => void;
  breakpointArmed?: boolean;
  onToggleBreakpoint?: () => void;
}

export function TraceDataInspector({
  step,
  previousStep,
  request,
  response,
  requestError,
  sending,
  onOpenSource,
  breakpointArmed = false,
  onToggleBreakpoint,
}: TraceDataInspectorProps) {
  const [view, setView] = useState<InspectorView>("shape");
  return (
    <section className="trace-data-pane" aria-label="Trace data and values">
      <header className="trace-data-tabs" role="tablist" aria-label="Trace data view">
        {(["shape", "request", "response"] as const).map((item) => (
          <button
            key={item}
            type="button"
            role="tab"
            aria-selected={view === item}
            className={view === item ? "is-active" : ""}
            onClick={() => setView(item)}
          >
            {item === "shape" ? "Step data" : item === "request" ? "Request" : "Response"}
            {item === "response" && response && <span>{response.status}</span>}
          </button>
        ))}
      </header>
      <div className="trace-data-scroll">
        {view === "shape" && (
          <ShapeView
            step={step}
            previousStep={previousStep}
            onOpenSource={onOpenSource}
            breakpointArmed={breakpointArmed}
            onToggleBreakpoint={onToggleBreakpoint}
          />
        )}
        {view === "request" && <RequestView request={request} sending={sending} />}
        {view === "response" && <ResponseView response={response} error={requestError} />}
      </div>
    </section>
  );
}
