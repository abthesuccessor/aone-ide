export function StageLoading({ kind }: { kind: "graph" | "source" | "debugger" }) {
  const title = kind === "graph"
    ? "Loading code graph"
    : kind === "debugger"
      ? "Loading instrumented debugger"
      : "Loading source editor";
  const detail = kind === "graph"
    ? "Preparing force-directed workspace relationships..."
    : kind === "debugger"
      ? "Preparing the retained safe-point workflow..."
      : "Preparing the local code editor...";
  return (
    <div className="source-state" role="status" aria-live="polite">
      <div className="graph-loader" aria-hidden="true" />
      <strong>{title}</strong>
      <span>{detail}</span>
    </div>
  );
}

export function SidebarLoading({ label }: { label: string }) {
  return (
    <aside className="workspace-panel" aria-label={label} aria-busy="true">
      <div className="panel-empty" role="status">Loading...</div>
    </aside>
  );
}
