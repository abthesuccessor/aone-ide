import {
  CircleNotch,
  Clock,
  Globe,
  PaperPlaneTilt,
  Warning,
} from "@phosphor-icons/react";
import { useId, useState } from "react";
import { SlidingTabs } from "../../components/SlidingTabs";
import { findUnresolvedRoutePlaceholders } from "./model";
import type { ApiClientSession } from "./useApiClientSession";
import "./api-client.css";

const STANDARD_METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE"];

export interface ApiClientPaneProps {
  session: ApiClientSession;
  debugActive?: boolean;
  responseMode?: "full" | "none";
  compact?: boolean;
}

export function ApiClientPane({
  session,
  debugActive = false,
  responseMode = "full",
  compact = false,
}: ApiClientPaneProps) {
  const [editorView, setEditorView] = useState<"body" | "headers">("body");
  const routeWarningId = useId();
  const { draft, response, requestError, sending } = session;
  const unresolvedRoutePlaceholders = findUnresolvedRoutePlaceholders(draft.url);
  const hasUnresolvedRoute = unresolvedRoutePlaceholders.length > 0;
  const methods = STANDARD_METHODS.includes(draft.method)
    ? STANDARD_METHODS
    : [draft.method, ...STANDARD_METHODS];

  return (
    <div className={`api-console${responseMode === "none" ? " is-request-only" : ""}${compact ? " is-compact" : ""}`}>
      <section className="request-composer" aria-label="API request">
        <div className="request-toolbar">
          <SlidingTabs
            value={draft.protocol}
            onChange={session.setProtocol}
            label="API protocol"
            options={[
              { value: "rest", label: "REST" },
              { value: "graphql", label: "GraphQL" },
            ]}
            className="protocol-tabs"
          />
          <span
            className="request-privacy"
            title={`Sensitive headers are redacted in session evidence${debugActive ? "; Debug requests allow up to 60 seconds for safe-point stepping" : ""}`}
          >
            {compact ? "Local · redacted" : "Sensitive headers are redacted in session evidence"}
            {!compact && debugActive ? ", Debug requests allow up to 60 seconds for safe-point stepping" : ""}
          </span>
        </div>
        <div className="request-line">
          <select
            aria-label="HTTP method"
            value={draft.protocol === "graphql" ? "POST" : draft.method}
            onChange={(event) => session.updateDraft({ method: event.target.value })}
            disabled={draft.protocol === "graphql"}
          >
            {methods.map((item) => <option key={item}>{item}</option>)}
          </select>
          <input
            aria-label="Request URL"
            value={draft.url}
            onChange={(event) => session.updateDraft({ url: event.target.value })}
            aria-invalid={hasUnresolvedRoute || undefined}
            aria-describedby={hasUnresolvedRoute ? routeWarningId : undefined}
            spellCheck={false}
          />
          <button
            type="button"
            className="send-button"
            onClick={() => void session.send()}
            disabled={sending || !draft.url.trim() || hasUnresolvedRoute}
            aria-describedby={hasUnresolvedRoute ? routeWarningId : undefined}
            title={hasUnresolvedRoute ? "Replace every route parameter before sending" : undefined}
          >
            {sending ? <CircleNotch className="spin" size={14} /> : <PaperPlaneTilt size={14} weight="fill" />}
            {sending ? "Sending" : "Send"}
          </button>
        </div>
        {hasUnresolvedRoute && (
          <div className="request-validation" id={routeWarningId} role="alert">
            <Warning size={13} />
            <span>Replace route parameters before sending: <code>{unresolvedRoutePlaceholders.join(", ")}</code></span>
          </div>
        )}
        {compact && (
          <div className="request-editor-switcher" role="tablist" aria-label="Request editor">
            <button type="button" role="tab" aria-selected={editorView === "body"} onClick={() => setEditorView("body")}>Body</button>
            <button type="button" role="tab" aria-selected={editorView === "headers"} onClick={() => setEditorView("headers")}>Headers</button>
          </div>
        )}
        <div className={`request-editors${draft.protocol === "graphql" ? " is-graphql" : ""}${compact ? " is-compact" : ""}`}>
          {(!compact || editorView === "headers") && <label>
            <span>Headers</span>
            <textarea
              aria-label="Request headers"
              value={draft.headers}
              onChange={(event) => session.updateDraft({ headers: event.target.value })}
              spellCheck={false}
            />
          </label>}
          {(!compact || editorView === "body") && <label className="request-body-field">
            <span>{draft.protocol === "graphql" ? "Query" : "JSON body"}</span>
            <textarea
              aria-label={draft.protocol === "graphql" ? "GraphQL query" : "Request body"}
              value={draft.protocol === "graphql" ? draft.graphqlBody : draft.restBody}
              onChange={(event) => session.updateDraft(draft.protocol === "graphql"
                ? { graphqlBody: event.target.value }
                : { restBody: event.target.value })}
              spellCheck={false}
            />
          </label>}
          {draft.protocol === "graphql" && (!compact || editorView === "body") && (
            <label>
              <span>Variables</span>
              <textarea
                aria-label="GraphQL variables"
                value={draft.graphqlVariables}
                onChange={(event) => session.updateDraft({ graphqlVariables: event.target.value })}
                spellCheck={false}
              />
            </label>
          )}
        </div>
      </section>

      {responseMode === "full" && (
        <section className="response-view" aria-label="API response">
          <div className="response-header">
            <span>Response</span>
            {response && (
              <>
                <strong className={response.status < 400 ? "response-ok" : "response-bad"}>{response.status} {response.statusText}</strong>
                <span><Clock size={12} /> {response.durationMs} ms</span>
              </>
            )}
          </div>
          {requestError ? (
            <div className="response-error" role="alert"><Warning size={18} />{requestError}</div>
          ) : !response ? (
            <div className="console-empty"><Globe size={22} weight="thin" /><span>Compose and send a local request.</span></div>
          ) : (
            <div className="response-content">
              <pre>{response.body}{response.truncated ? "\n\n[response truncated]" : ""}</pre>
              <div className="response-inspector">
                <div className="request-timeline" aria-label="Request timeline">
                  <div className="timeline-item" title={`Request ${response.requestId}`}>
                    <span>Total</span>
                    <div><i style={{ width: "100%" }} /></div>
                    <strong>{response.durationMs} ms</strong>
                  </div>
                  <div className="timeline-item" title="HTTP response status">
                    <span>HTTP {response.status}</span>
                    <div><i style={{ width: response.status < 400 ? "100%" : "45%" }} /></div>
                    <strong>{response.headers.length} hdr</strong>
                  </div>
                </div>
                <section className="response-headers-list" aria-label="Response headers">
                  <header><span>Headers</span><small>{response.headers.length}</small></header>
                  {response.headers.length === 0 ? (
                    <p>No response headers</p>
                  ) : response.headers.map((header, index) => (
                    <div key={`${header.name}-${index}`}>
                      <code>{header.name}</code>
                      <span>{header.value}</span>
                    </div>
                  ))}
                </section>
              </div>
            </div>
          )}
        </section>
      )}
    </div>
  );
}
