import { Check, Clipboard, Play, Stop, Trash } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import type { OtlpReceiverSnapshot } from "../../types";

interface OtlpReceiverBarProps {
  snapshot: OtlpReceiverSnapshot | null;
  available: boolean;
  startEnabled?: boolean;
  loading: boolean;
  error?: string | null;
  selectedTraceId?: string;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
  onDeleteTrace: (traceId: string) => Promise<void>;
}

function exporterConfiguration(snapshot: OtlpReceiverSnapshot): string | undefined {
  if (!snapshot.endpoint || !snapshot.authHeaderValue) return undefined;
  return [
    `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=${snapshot.endpoint}`,
    "OTEL_EXPORTER_OTLP_TRACES_PROTOCOL=http/json",
    `OTEL_EXPORTER_OTLP_TRACES_HEADERS=${snapshot.authHeaderName}=${snapshot.authHeaderValue}`,
  ].join("\n");
}

export function OtlpReceiverBar({
  snapshot,
  available,
  startEnabled = true,
  loading,
  error,
  selectedTraceId,
  onStart,
  onStop,
  onDeleteTrace,
}: OtlpReceiverBarProps) {
  const [copied, setCopied] = useState(false);
  const configuration = useMemo(
    () => snapshot ? exporterConfiguration(snapshot) : undefined,
    [snapshot],
  );
  const running = snapshot?.status === "running";
  const status = snapshot?.status ?? (loading ? "starting" : "stopped");

  const copyConfiguration = async () => {
    if (!configuration) return;
    await navigator.clipboard.writeText(configuration);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1_500);
  };

  const deleteSelectedTrace = () => {
    if (!selectedTraceId) return;
    if (!window.confirm("Delete all retained in-memory events for the selected trace?")) return;
    void onDeleteTrace(selectedTraceId);
  };

  return (
    <section className="otlp-receiver-bar" aria-label="Local OpenTelemetry receiver">
      <div className="otlp-receiver-summary">
        <strong>Local OTLP trace receiver</strong>
        <span className={`otlp-receiver-status status-${status}`}>{status}</span>
        {running && snapshot && (
          <span>{snapshot.traceCount} trace{snapshot.traceCount === 1 ? "" : "s"}, {snapshot.acceptedSpans} accepted, {snapshot.rejectedSpans} rejected</span>
        )}
        {!available ? <span>Desktop app only</span> : !startEnabled && !running ? <span>Start Debug first</span> : null}
      </div>
      <div className="otlp-receiver-actions">
        {running ? (
          <button type="button" onClick={() => void onStop()} disabled={loading}>
            <Stop size={12} weight="fill" /> Stop receiver
          </button>
        ) : (
          <button type="button" onClick={() => void onStart()} disabled={!available || !startEnabled || loading}>
            <Play size={12} weight="fill" /> Start receiver
          </button>
        )}
        <button type="button" onClick={deleteSelectedTrace} disabled={!available || loading || !selectedTraceId} title="Delete retained events for the selected trace">
          <Trash size={12} /> Delete trace
        </button>
      </div>
      {configuration && (
        <details className="otlp-exporter-config">
          <summary>Exporter configuration</summary>
          <pre>{configuration}</pre>
          <button type="button" onClick={() => void copyConfiguration()}>
            {copied ? <Check size={12} /> : <Clipboard size={12} />}
            {copied ? "Copied" : "Copy environment variables"}
          </button>
          <small>The capability token is ephemeral and becomes invalid when the receiver stops or the workspace changes.</small>
        </details>
      )}
      {(error ?? snapshot?.error) && <span className="otlp-receiver-error" role="alert">{error ?? snapshot?.error}</span>}
      {snapshot?.limitation && <small className="otlp-receiver-limitation">{snapshot.limitation}</small>}
    </section>
  );
}
