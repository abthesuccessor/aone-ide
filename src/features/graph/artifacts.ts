import type { EvidenceKind, GraphEdge, GraphNode, GraphSnapshot } from "../../types";

export const GRAPH_ARTIFACT_SCHEMA_VERSION = "aone.graph-artifact.v1" as const;

const EVIDENCE_KINDS = ["declared", "resolved", "observed", "inferred"] as const satisfies readonly EvidenceKind[];
const COMMUNITY_METADATA_KEYS = {
  id: "communityId",
  size: "communitySize",
  algorithm: "communityAlgorithm",
  basis: "communityBasis",
  complete: "communityComplete",
} as const;
const COMMUNITY_ALGORITHM = "deterministicLabelPropagationV1";
const COMMUNITY_BASIS = "boundedGraphSnapshot";
const COMMUNITY_ID_PATTERN = /^community:[0-9a-f]{32}$/;
const REPORT_ITEM_LIMIT = 20;

type JsonPrimitive = boolean | null | number | string;
type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };

export interface GraphArtifactOptions {
  workspaceSafeName: string;
  generatedAt: string;
  graph: GraphSnapshot;
}

export interface GraphArtifactFile {
  filename: "graph.json" | "GRAPH_REPORT.md" | "graph.html";
  mimeType: string;
  content: string;
}

export interface NamedCount {
  name: string;
  count: number;
}

export type EvidenceCounts = Record<EvidenceKind, number>;

export interface DegreeRankedHub {
  nodeId: string;
  label: string;
  kind: string;
  inDegree: number;
  outDegree: number;
  totalDegree: number;
}

export interface StructuralCommunity {
  communityId: string;
  returnedNodeCount: number;
  communitySize: number | null;
  communityAlgorithm: string | null;
  communityBasis: string | null;
  communityComplete: boolean | null;
  contractConsistent: boolean;
  nodeIds: string[];
}

export interface GraphArtifactDocument {
  schemaVersion: typeof GRAPH_ARTIFACT_SCHEMA_VERSION;
  artifactType: "bounded-graph-snapshot";
  generatedAt: string;
  workspace: { safeName: string };
  scope: {
    bounded: true;
    truncated: boolean;
    completeness: "bounded-query-complete" | "bounded-query-truncated";
    nextCursorAvailable: boolean;
    totalRootLinks: number | null;
    omittedRootLinks: number | null;
    statement: string;
  };
  counts: {
    nodeCount: number;
    edgeCount: number;
    evidenceCounts: {
      nodes: EvidenceCounts;
      edges: EvidenceCounts;
      combined: EvidenceCounts;
    };
    nodeKindCounts: NamedCount[];
    relationCounts: NamedCount[];
  };
  degreeRankedHubs: DegreeRankedHub[];
  structuralCommunities: {
    metadataKeys: typeof COMMUNITY_METADATA_KEYS;
    unassignedNodeCount: number;
    communities: StructuralCommunity[];
  };
  limitations: string[];
  graph: GraphSnapshot;
}

export interface GraphArtifactBundle {
  graphJson: GraphArtifactFile;
  graphReport: GraphArtifactFile;
  graphHtml: GraphArtifactFile;
}

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function canonicalJsonValue(value: unknown, ancestors: Set<object>): JsonValue {
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("Graph metadata numbers must be finite.");
    return value;
  }
  if (typeof value !== "object") throw new TypeError("Graph metadata must contain JSON values only.");
  if (ancestors.has(value)) throw new TypeError("Graph metadata must not contain cycles.");

  ancestors.add(value);
  try {
    if (Array.isArray(value)) return value.map((item) => canonicalJsonValue(item, ancestors));
    const result: { [key: string]: JsonValue } = {};
    for (const key of Object.keys(value).sort(compareText)) {
      result[key] = canonicalJsonValue(Reflect.get(value, key), ancestors);
    }
    return result;
  } finally {
    ancestors.delete(value);
  }
}

function canonicalMetadata(metadata: Readonly<Record<string, unknown>>): Record<string, unknown> {
  return canonicalJsonValue(metadata, new Set()) as { [key: string]: JsonValue };
}

function canonicalNode(node: GraphNode): GraphNode {
  return {
    id: node.id,
    kind: node.kind,
    label: node.label,
    ...(node.source ? { source: {
      relativePath: node.source.relativePath,
      startLine: node.source.startLine,
      startColumn: node.source.startColumn,
      endLine: node.source.endLine,
      endColumn: node.source.endColumn,
    } } : {}),
    ...(node.language === undefined ? {} : { language: node.language }),
    evidence: node.evidence,
    metadata: canonicalMetadata(node.metadata),
  };
}

function canonicalEdge(edge: GraphEdge): GraphEdge {
  return {
    id: edge.id,
    source: edge.source,
    target: edge.target,
    kind: edge.kind,
    evidence: edge.evidence,
    ...(edge.confidence === undefined ? {} : { confidence: edge.confidence }),
    metadata: canonicalMetadata(edge.metadata),
  };
}

function stableValue(value: unknown): string {
  return JSON.stringify(value);
}

function canonicalGraph(graph: GraphSnapshot): GraphSnapshot {
  const nodes = graph.nodes.map(canonicalNode).sort((left, right) =>
    compareText(left.id, right.id) || compareText(stableValue(left), stableValue(right)));
  const edges = graph.edges.map(canonicalEdge).sort((left, right) =>
    compareText(left.id, right.id) || compareText(stableValue(left), stableValue(right)));
  return {
    nodes,
    edges,
    truncated: graph.truncated,
    ...(graph.nextCursor === undefined ? {} : { nextCursor: graph.nextCursor }),
    ...(graph.totalRootLinks === undefined ? {} : { totalRootLinks: graph.totalRootLinks }),
    ...(graph.omittedRootLinks === undefined ? {} : { omittedRootLinks: graph.omittedRootLinks }),
  };
}

function emptyEvidenceCounts(): EvidenceCounts {
  return { declared: 0, resolved: 0, observed: 0, inferred: 0 };
}

function evidenceCounts(items: readonly { evidence: EvidenceKind }[]): EvidenceCounts {
  const counts = emptyEvidenceCounts();
  for (const item of items) counts[item.evidence] += 1;
  return counts;
}

function addEvidenceCounts(left: EvidenceCounts, right: EvidenceCounts): EvidenceCounts {
  return Object.fromEntries(EVIDENCE_KINDS.map((kind) => [kind, left[kind] + right[kind]])) as EvidenceCounts;
}

function namedCounts(values: readonly string[]): NamedCount[] {
  const counts = new Map<string, number>();
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
  return [...counts].map(([name, count]) => ({ name, count })).sort((left, right) =>
    right.count - left.count || compareText(left.name, right.name));
}

function degreeRankedHubs(graph: GraphSnapshot): DegreeRankedHub[] {
  const degrees = new Map(graph.nodes.map((node) => [node.id, { inDegree: 0, outDegree: 0 }]));
  for (const edge of graph.edges) {
    const source = degrees.get(edge.source);
    const target = degrees.get(edge.target);
    if (source) source.outDegree += 1;
    if (target) target.inDegree += 1;
  }
  return graph.nodes.map((node) => {
    const degree = degrees.get(node.id) ?? { inDegree: 0, outDegree: 0 };
    return {
      nodeId: node.id,
      label: node.label,
      kind: node.kind,
      ...degree,
      totalDegree: degree.inDegree + degree.outDegree,
    };
  }).sort((left, right) => right.totalDegree - left.totalDegree
    || right.outDegree - left.outDegree
    || right.inDegree - left.inDegree
    || compareText(left.nodeId, right.nodeId));
}

interface CommunityAnnotation {
  communityId: string;
  communitySize: number | null;
  communityAlgorithm: string | null;
  communityBasis: string | null;
  communityComplete: boolean | null;
}

function communityAnnotation(node: GraphNode): CommunityAnnotation | null {
  const id = node.metadata[COMMUNITY_METADATA_KEYS.id];
  if (typeof id !== "string" || id === "") return null;
  const size = node.metadata[COMMUNITY_METADATA_KEYS.size];
  const algorithm = node.metadata[COMMUNITY_METADATA_KEYS.algorithm];
  const basis = node.metadata[COMMUNITY_METADATA_KEYS.basis];
  const complete = node.metadata[COMMUNITY_METADATA_KEYS.complete];
  return {
    communityId: id,
    communitySize: typeof size === "number" && Number.isSafeInteger(size) && size >= 0 ? size : null,
    communityAlgorithm: typeof algorithm === "string" ? algorithm : null,
    communityBasis: typeof basis === "string" ? basis : null,
    communityComplete: typeof complete === "boolean" ? complete : null,
  };
}

function consensus<T extends boolean | number | string>(values: readonly (T | null)[]): T | null {
  const first = values[0];
  if (first === undefined || first === null || values.some((value) => value !== first)) return null;
  return first;
}

function structuralCommunities(graph: GraphSnapshot): GraphArtifactDocument["structuralCommunities"] {
  const groups = new Map<string, { nodeIds: string[]; annotations: CommunityAnnotation[] }>();
  let unassignedNodeCount = 0;
  for (const node of graph.nodes) {
    const annotation = communityAnnotation(node);
    if (annotation === null) {
      unassignedNodeCount += 1;
      continue;
    }
    const group = groups.get(annotation.communityId) ?? { nodeIds: [], annotations: [] };
    group.nodeIds.push(node.id);
    group.annotations.push(annotation);
    groups.set(annotation.communityId, group);
  }
  const communities = [...groups].map(([communityId, group]) => {
    const communitySize = consensus(group.annotations.map((item) => item.communitySize));
    const communityAlgorithm = consensus(group.annotations.map((item) => item.communityAlgorithm));
    const communityBasis = consensus(group.annotations.map((item) => item.communityBasis));
    const communityComplete = consensus(group.annotations.map((item) => item.communityComplete));
    return {
      communityId,
      returnedNodeCount: group.nodeIds.length,
      communitySize,
      communityAlgorithm,
      communityBasis,
      communityComplete,
      contractConsistent: COMMUNITY_ID_PATTERN.test(communityId)
        && communitySize === group.nodeIds.length
        && communityAlgorithm === COMMUNITY_ALGORITHM
        && communityBasis === COMMUNITY_BASIS
        && communityComplete === !graph.truncated,
      nodeIds: group.nodeIds.sort(compareText),
    };
  }).sort((left, right) => compareText(left.communityId, right.communityId));
  return { metadataKeys: COMMUNITY_METADATA_KEYS, unassignedNodeCount, communities };
}

function scopeFor(graph: GraphSnapshot): GraphArtifactDocument["scope"] {
  return {
    bounded: true,
    truncated: graph.truncated,
    completeness: graph.truncated ? "bounded-query-truncated" : "bounded-query-complete",
    nextCursorAvailable: graph.nextCursor !== undefined,
    totalRootLinks: graph.totalRootLinks ?? null,
    omittedRootLinks: graph.omittedRootLinks ?? null,
    statement: graph.truncated
      ? "This bounded query snapshot reports omitted facts because at least one applicable limit was reached."
      : "No applicable limit reported omissions for this bounded query; this is not a whole-workspace completeness claim.",
  };
}

function limitationsFor(graph: GraphSnapshot): string[] {
  return [
    "This artifact contains one bounded query snapshot, not proof that every workspace fact was indexed or returned.",
    graph.truncated
      ? "The snapshot reports truncation; facts beyond one or more node, edge, depth, work, or page bounds may be absent."
      : "truncated=false applies only to the executed bounded query and does not prove whole-workspace completeness.",
    "Declared and resolved evidence describes static source relationships; it is not runtime execution proof.",
    "Inferred evidence remains an inference, and observed evidence is limited to the captured runtime events.",
    "Structural communities are backend-provided deterministic label-propagation annotations over this bounded graph snapshot; they are not semantic or topic clusters.",
    "communitySize counts members returned in this snapshot, while communityComplete only reports whether this query snapshot was truncated; neither is a whole-workspace claim.",
    "Community IDs hash sorted returned member IDs; propagation treats returned edges as undirected and deduplicated, ignores dangling endpoints, assigns isolates singleton communities, and can change when a less-bounded snapshot returns more facts.",
    "Dynamic dispatch, generated behavior, reflection, and unobserved runtime paths may be missing.",
    "Labels, metadata, and workspace-relative source locations can still be sensitive; the standalone viewer performs no network requests.",
    "Viewer search is a local case-insensitive substring match, not semantic search.",
  ];
}

function buildDocument(options: GraphArtifactOptions): GraphArtifactDocument {
  if (options.workspaceSafeName.trim() === "") throw new TypeError("workspaceSafeName must not be empty.");
  if (options.generatedAt.trim() === "") throw new TypeError("generatedAt must not be empty.");
  const graph = canonicalGraph(options.graph);
  const nodeEvidence = evidenceCounts(graph.nodes);
  const edgeEvidence = evidenceCounts(graph.edges);
  return {
    schemaVersion: GRAPH_ARTIFACT_SCHEMA_VERSION,
    artifactType: "bounded-graph-snapshot",
    generatedAt: options.generatedAt,
    workspace: { safeName: options.workspaceSafeName },
    scope: scopeFor(graph),
    counts: {
      nodeCount: graph.nodes.length,
      edgeCount: graph.edges.length,
      evidenceCounts: {
        nodes: nodeEvidence,
        edges: edgeEvidence,
        combined: addEvidenceCounts(nodeEvidence, edgeEvidence),
      },
      nodeKindCounts: namedCounts(graph.nodes.map((node) => node.kind)),
      relationCounts: namedCounts(graph.edges.map((edge) => edge.kind)),
    },
    degreeRankedHubs: degreeRankedHubs(graph),
    structuralCommunities: structuralCommunities(graph),
    limitations: limitationsFor(graph),
    graph,
  } satisfies GraphArtifactDocument;
}

function markdownText(value: string): string {
  return value.replace(/[\\`*_{}[\]()#+.!|<>]/g, "\\$&").replace(/\r?\n|\r/g, " ");
}

function countTable(title: string, label: string, counts: readonly NamedCount[]): string[] {
  return [
    `### ${title}`,
    "",
    `| ${label} | Count |`,
    "| --- | ---: |",
    ...(counts.length ? counts.map((entry) => `| ${markdownText(entry.name)} | ${entry.count} |`) : ["| \(none\) | 0 |"]),
  ];
}

function buildReport(document: GraphArtifactDocument): string {
  const evidenceRows = EVIDENCE_KINDS.map((kind) => {
    const counts = document.counts.evidenceCounts;
    return `| ${kind} | ${counts.nodes[kind]} | ${counts.edges[kind]} | ${counts.combined[kind]} |`;
  });
  const hubs = document.degreeRankedHubs.slice(0, REPORT_ITEM_LIMIT);
  const communities = document.structuralCommunities.communities.slice(0, REPORT_ITEM_LIMIT);
  const lines = [
    "# Aone graph report",
    "",
    `- Workspace safe name: ${markdownText(document.workspace.safeName)}`,
    `- Generated at: ${markdownText(document.generatedAt)}`,
    `- Schema version: ${document.schemaVersion}`,
    "",
    "## Scope",
    "",
    `- Bounded snapshot: yes`,
    `- Truncated: ${document.scope.truncated ? "yes" : "no"}`,
    `- Continuation cursor available: ${document.scope.nextCursorAvailable ? "yes" : "no"}`,
    `- Total eligible root links: ${document.scope.totalRootLinks ?? "not reported"}`,
    `- Omitted root links: ${document.scope.omittedRootLinks ?? "not reported"}`,
    "",
    document.scope.statement,
    "",
    "## Counts",
    "",
    `- Nodes: ${document.counts.nodeCount}`,
    `- Edges: ${document.counts.edgeCount}`,
    "",
    "### Evidence",
    "",
    "| Evidence | Nodes | Edges | Combined |",
    "| --- | ---: | ---: | ---: |",
    ...evidenceRows,
    "",
    ...countTable("Node kinds", "Kind", document.counts.nodeKindCounts),
    "",
    ...countTable("Relations", "Relation", document.counts.relationCounts),
    "",
    `## Degree-ranked hubs (top ${Math.min(REPORT_ITEM_LIMIT, document.degreeRankedHubs.length)} of ${document.degreeRankedHubs.length})`,
    "",
    "| Rank | Node | Kind | In | Out | Total |",
    "| ---: | --- | --- | ---: | ---: | ---: |",
    ...(hubs.length ? hubs.map((hub, index) => `| ${index + 1} | ${markdownText(hub.label)} (${markdownText(hub.nodeId)}) | ${markdownText(hub.kind)} | ${hub.inDegree} | ${hub.outDegree} | ${hub.totalDegree} |`) : ["| - | (none) | - | 0 | 0 | 0 |"]),
    "",
    "## Structural communities",
    "",
    `Summarized from the node metadata contract \`${Object.values(COMMUNITY_METADATA_KEYS).join("\`, \`")}\`.`,
    "These are deterministic structural communities over the returned bounded graph, never semantic or topic communities. SCC metadata is not used as a fallback.",
    "",
    `- Assigned communities: ${document.structuralCommunities.communities.length}`,
    `- Nodes without a communityId annotation: ${document.structuralCommunities.unassignedNodeCount}`,
    "",
    "| Community | Returned | Reported size | Algorithm | Basis | Complete | Contract consistent | Member IDs |",
    "| --- | ---: | ---: | --- | --- | --- | --- | --- |",
    ...(communities.length ? communities.map((community) => `| ${markdownText(community.communityId)} | ${community.returnedNodeCount} | ${community.communitySize ?? "not reported"} | ${markdownText(community.communityAlgorithm ?? "not reported")} | ${markdownText(community.communityBasis ?? "not reported")} | ${community.communityComplete === null ? "not reported" : community.communityComplete ? "yes" : "no"} | ${community.contractConsistent ? "yes" : "no"} | ${community.nodeIds.map(markdownText).join(", ")} |`) : ["| (none) | 0 | - | - | - | - | - | - |"]),
    "",
    "## Limitations",
    "",
    ...document.limitations.map((limitation) => `- ${markdownText(limitation)}`),
    "",
  ];
  return `${lines.join("\n")}\n`;
}

function escapeEmbeddedJson(json: string): string {
  return json.replace(/&/g, "\\u0026").replace(/</g, "\\u003c").replace(/>/g, "\\u003e")
    .replace(/\u2028/g, "\\u2028").replace(/\u2029/g, "\\u2029");
}

function buildHtml(documentJson: string): string {
  const embedded = escapeEmbeddedJson(documentJson.trimEnd());
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <meta name="referrer" content="no-referrer">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; font-src 'none'; media-src 'none'; worker-src 'none'">
  <title>Aone graph artifact</title>
  <style>
    :root{color-scheme:dark;--bg:#0b1020;--panel:#121a2d;--text:#eef2ff;--muted:#9aa7bd;--line:#52627b;--accent:#66d9c9}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:14px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace}header,main{max-width:1440px;margin:auto;padding:20px}h1,h2{margin:.2em 0}.scope{color:var(--muted)}label{display:block;margin:16px 0 8px}input{width:min(640px,100%);padding:10px 12px;border:1px solid var(--line);border-radius:6px;background:#080d19;color:var(--text)}.layout{display:grid;grid-template-columns:minmax(0,2fr) minmax(280px,1fr);gap:16px}.panel{min-width:0;border:1px solid #26334a;border-radius:8px;background:var(--panel);padding:12px}svg{width:100%;height:min(68vh,680px);background:#090f1c;border-radius:6px}.edge{fill:none;stroke:var(--line);stroke-width:1.5}.node circle{fill:#1a2942;stroke:var(--accent);stroke-width:2}.node text{fill:var(--text);font-size:11px}.muted{color:var(--muted)}ul{padding-left:20px;max-height:280px;overflow:auto}li{margin:4px 0;overflow-wrap:anywhere}@media(max-width:850px){.layout{grid-template-columns:1fr}svg{height:480px}}
  </style>
</head>
<body>
  <header><h1 id="title">Aone graph artifact</h1><p id="summary" class="scope"></p><label for="search">Search nodes and relations</label><input id="search" type="search" autocomplete="off" placeholder="Label, ID, kind, path, or relation"></header>
  <main><div class="layout"><section class="panel"><svg id="graph" viewBox="0 0 1000 640" role="img" aria-label="Graph"><g id="edges"></g><g id="nodes"></g></svg></section><aside class="panel"><h2>Visible nodes</h2><p id="node-count" class="muted"></p><ul id="node-list"></ul><h2>Visible relations</h2><p id="edge-count" class="muted"></p><ul id="edge-list"></ul></aside></div><section class="panel"><h2>Scope and limitations</h2><p id="scope"></p><ul id="limitations"></ul></section></main>
  <script id="graph-data" type="application/json">${embedded}</script>
  <script>
  "use strict";
  (()=>{const data=JSON.parse(document.getElementById("graph-data").textContent||"{}");const ns="http://www.w3.org/2000/svg";const nodes=data.graph.nodes;const edges=data.graph.edges;const byId=new Map(nodes.map(node=>[node.id,node]));const positions=new Map();const radius=Math.min(260,70+nodes.length*8);nodes.forEach((node,index)=>{const angle=nodes.length===1?0:(Math.PI*2*index/nodes.length)-Math.PI/2;positions.set(node.id,{x:nodes.length===1?500:500+Math.cos(angle)*radius,y:nodes.length===1?320:320+Math.sin(angle)*radius});});
  const edgeGroups=new Map();edges.forEach(edge=>{const key=edge.source+"\u0000"+edge.target;const group=edgeGroups.get(key)||[];group.push(edge);edgeGroups.set(key,group);});const edgeViews=new Map();for(const edge of edges){const start=positions.get(edge.source);const end=positions.get(edge.target);if(!start||!end)continue;const group=edgeGroups.get(edge.source+"\u0000"+edge.target);const index=group.indexOf(edge);const offset=(index-(group.length-1)/2)*18;const path=document.createElementNS(ns,"path");let d;if(edge.source===edge.target){const loop=24+index*10;d="M "+start.x+" "+start.y+" C "+(start.x+loop)+" "+(start.y-loop*2)+" "+(start.x-loop)+" "+(start.y-loop*2)+" "+start.x+" "+start.y;}else{const dx=end.x-start.x,dy=end.y-start.y,length=Math.max(1,Math.hypot(dx,dy));const mx=(start.x+end.x)/2-(dy/length)*offset,my=(start.y+end.y)/2+(dx/length)*offset;d="M "+start.x+" "+start.y+" Q "+mx+" "+my+" "+end.x+" "+end.y;}path.setAttribute("d",d);path.setAttribute("class","edge");path.setAttribute("data-edge-id",edge.id);const title=document.createElementNS(ns,"title");title.textContent=edge.source+" —"+edge.kind+"→ "+edge.target+" ["+edge.evidence+"]";path.append(title);document.getElementById("edges").append(path);edgeViews.set(edge.id,path);}
  const nodeViews=new Map();for(const node of nodes){const point=positions.get(node.id);const group=document.createElementNS(ns,"g");group.setAttribute("class","node");group.setAttribute("transform","translate("+point.x+" "+point.y+")");group.setAttribute("data-node-id",node.id);const circle=document.createElementNS(ns,"circle");circle.setAttribute("r","9");const text=document.createElementNS(ns,"text");text.setAttribute("x","13");text.setAttribute("y","4");text.textContent=node.label.length>30?node.label.slice(0,29)+"…":node.label;const title=document.createElementNS(ns,"title");title.textContent=node.label+" ["+node.kind+", "+node.evidence+"]";group.append(circle,text,title);document.getElementById("nodes").append(group);nodeViews.set(node.id,group);}
  const searchableNode=node=>[node.id,node.label,node.kind,node.language||"",node.source?.relativePath||""].join(" ").toLowerCase();const searchableEdge=edge=>[edge.id,edge.source,edge.target,edge.kind,edge.evidence,byId.get(edge.source)?.label||"",byId.get(edge.target)?.label||""].join(" ").toLowerCase();const appendItem=(list,text)=>{const item=document.createElement("li");item.textContent=text;list.append(item);};
  const render=()=>{const query=document.getElementById("search").value.trim().toLowerCase();const directNodes=new Set(nodes.filter(node=>!query||searchableNode(node).includes(query)).map(node=>node.id));const visibleEdges=new Set(edges.filter(edge=>!query||searchableEdge(edge).includes(query)||directNodes.has(edge.source)||directNodes.has(edge.target)).map(edge=>edge.id));const visibleNodes=new Set(directNodes);for(const edge of edges)if(visibleEdges.has(edge.id)){visibleNodes.add(edge.source);visibleNodes.add(edge.target);}for(const [id,view] of nodeViews)view.style.display=visibleNodes.has(id)?"":"none";for(const [id,view] of edgeViews)view.style.display=visibleEdges.has(id)?"":"none";const shownNodes=nodes.filter(node=>visibleNodes.has(node.id));const shownEdges=edges.filter(edge=>visibleEdges.has(edge.id));const nodeList=document.getElementById("node-list"),edgeList=document.getElementById("edge-list");nodeList.replaceChildren();edgeList.replaceChildren();shownNodes.forEach(node=>appendItem(nodeList,node.label+" · "+node.kind+" · "+node.evidence));shownEdges.forEach(edge=>appendItem(edgeList,edge.source+" —"+edge.kind+"→ "+edge.target+" · "+edge.evidence));document.getElementById("node-count").textContent=shownNodes.length+" of "+nodes.length;document.getElementById("edge-count").textContent=shownEdges.length+" of "+edges.length;};
  document.getElementById("title").textContent=data.workspace.safeName+" graph";document.getElementById("summary").textContent=data.generatedAt+" · "+data.counts.nodeCount+" nodes · "+data.counts.edgeCount+" edges";document.getElementById("scope").textContent=data.scope.statement;const limitations=document.getElementById("limitations");data.limitations.forEach(item=>appendItem(limitations,item));document.getElementById("search").addEventListener("input",render);render();})();
  </script>
</body>
</html>
`;
}

export function createGraphArtifactBundle(options: GraphArtifactOptions): GraphArtifactBundle {
  const document = buildDocument(options);
  const json = `${JSON.stringify(document, null, 2)}\n`;
  return {
    graphJson: { filename: "graph.json", mimeType: "application/json;charset=utf-8", content: json },
    graphReport: { filename: "GRAPH_REPORT.md", mimeType: "text/markdown;charset=utf-8", content: buildReport(document) },
    graphHtml: { filename: "graph.html", mimeType: "text/html;charset=utf-8", content: buildHtml(json) },
  };
}

export function downloadGraphArtifactBundle(
  bundle: GraphArtifactBundle,
  documentRef: Document = document,
  urlApi: Pick<typeof URL, "createObjectURL" | "revokeObjectURL"> = URL,
): void {
  const files = [bundle.graphJson, bundle.graphReport, bundle.graphHtml];
  for (const file of files) {
    const objectUrl = urlApi.createObjectURL(new Blob([file.content], { type: file.mimeType }));
    const anchor = documentRef.createElement("a");
    try {
      anchor.href = objectUrl;
      anchor.download = file.filename;
      anchor.hidden = true;
      documentRef.body.append(anchor);
      anchor.click();
    } finally {
      anchor.remove();
      urlApi.revokeObjectURL(objectUrl);
    }
  }
}
