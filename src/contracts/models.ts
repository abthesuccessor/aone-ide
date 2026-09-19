import type {
  AiConfigurationStatus as GeneratedAiConfigurationStatus,
  AiExplainRequestRequest as GeneratedAiExplainRequest,
  AiExplanation as GeneratedAiExplanation,
  ApiHeader as GeneratedApiHeader,
  ApiRequest as GeneratedApiRequest,
  AppSnapshot as GeneratedAppSnapshot,
  CapabilityLevel as GeneratedCapabilityLevel,
  DeleteRuntimeTraceResult as GeneratedDeleteRuntimeTraceResult,
  EnvLoadResult as GeneratedEnvLoadResult,
  EvidenceKind as GeneratedEvidenceKind,
  EvidenceReference as GeneratedEvidenceReference,
  GraphEdge as GeneratedGraphEdge,
  GraphNode as GeneratedGraphNode,
  GraphQuery as GeneratedGraphQuery,
  GraphSnapshot as GeneratedGraphSnapshot,
  LanguageSummary as GeneratedLanguageSummary,
  ModelApiResponse as GeneratedApiResponse,
  OtlpReceiverSnapshot as GeneratedOtlpReceiverSnapshot,
  OpenFileResult as GeneratedOpenFileResult,
  RunProfile as GeneratedRunProfile,
  RuntimeEvent as GeneratedRuntimeEvent,
  ScanProgress as GeneratedScanProgress,
  SourceFile as GeneratedSourceFile,
  SourceLocation as GeneratedSourceLocation,
  StartRunRequestRequest as GeneratedStartRunRequest,
  StartOtlpReceiverRequestRequest as GeneratedStartOtlpReceiverRequest,
  WorkspaceFile as GeneratedWorkspaceFile,
  WorkspaceSearchMatch as GeneratedWorkspaceSearchMatch,
  WorkspaceSearchRequest as GeneratedWorkspaceSearchRequest,
  WorkspaceSearchResult as GeneratedWorkspaceSearchResult,
  WorkspaceSummary as GeneratedWorkspaceSummary,
  WebSocketConnectRequest as GeneratedWebSocketConnectRequest,
  WebSocketConnectResult as GeneratedWebSocketConnectResult,
  WebSocketDisconnectRequest as GeneratedWebSocketDisconnectRequest,
  WebSocketDisconnectResult as GeneratedWebSocketDisconnectResult,
  WebSocketEvent as GeneratedWebSocketEvent,
  WebSocketSendRequest as GeneratedWebSocketSendRequest,
  WebSocketSendResult as GeneratedWebSocketSendResult,
} from "../generated/ipc/models";

export type EvidenceKind = `${GeneratedEvidenceKind}`;
export type CapabilityLevel = `${GeneratedCapabilityLevel}`;
export type GraphMetadata = Record<string, unknown>;
export type GraphLens =
  | "system"
  | "all"
  | "architecture"
  | "api-data"
  | "runtime"
  | "dependencies";

export type SourceLocation = GeneratedSourceLocation;
export type SourceFile = GeneratedSourceFile;
export type GraphQuery = GeneratedGraphQuery;
export type RunProfile = GeneratedRunProfile;
export type EnvLoadResult = GeneratedEnvLoadResult;
export type AiConfigurationStatus = Omit<GeneratedAiConfigurationStatus, "provider" | "transport"> & {
  provider: "openai" | "anthropic" | "ollama" | "codex" | "claude" | null;
  transport: "api" | "local_http" | "cli" | null;
};
export type HostedAiProvider = "openai" | "anthropic";
export interface ConfigureHostedAiRequest {
  provider: HostedAiProvider;
  apiKey: string;
  model?: string;
}
export interface ConfigureOllamaRequest {
  endpoint: string;
  model: string;
}
export type AiCliAdapterId = "codex" | "claude" | "copilot";
export type AiCliAdapterReadiness = "ready" | "not_installed" | "not_supported" | "authentication_required" | "error";
export interface AiCliAdapterStatus {
  id: AiCliAdapterId;
  label: string;
  installed: boolean;
  readiness: AiCliAdapterReadiness;
  version?: string;
  detail: string;
}
export type OpenedWorkspaceFile = Omit<GeneratedOpenFileResult, "workspace"> & {
  workspace: WorkspaceSummary;
};
export type StartRunRequest = GeneratedStartRunRequest;
export type StartOtlpReceiverRequest = GeneratedStartOtlpReceiverRequest;
export type OtlpReceiverSnapshot = Omit<
  GeneratedOtlpReceiverSnapshot,
  "status" | "protocol" | "authHeaderName"
> & {
  status: "stopped" | "starting" | "running" | "failed";
  protocol: "http/json";
  authHeaderName: "x-aone-ingest-token";
};
export type DeleteRuntimeTraceResult = GeneratedDeleteRuntimeTraceResult;
export type ApiHeader = GeneratedApiHeader;
export type AiExplainRequest = GeneratedAiExplainRequest & { workspaceId: string };
export type ScanProgress = GeneratedScanProgress;
export type WebSocketEncoding = "text" | "base64";
export type WebSocketEventKind =
  | "connecting"
  | "open"
  | "message"
  | "dropped"
  | "error"
  | "closed";

export type LanguageSummary = Omit<GeneratedLanguageSummary, "capability"> & {
  capability: CapabilityLevel;
};

export type WorkspaceSummary = Omit<GeneratedWorkspaceSummary, "languages"> & {
  languages: LanguageSummary[];
};

export type WorkspaceFile = Omit<GeneratedWorkspaceFile, "capability"> & {
  capability: CapabilityLevel;
};

export type WorkspaceSearchMatchKind =
  | "content"
  | "path"
  | "symbol"
  | "endpoint"
  | "event"
  | "heading"
  | "sentence";

export type WorkspaceSearchRequest = GeneratedWorkspaceSearchRequest;

export type WorkspaceSearchMatch = Omit<GeneratedWorkspaceSearchMatch, "kind"> & {
  kind: WorkspaceSearchMatchKind;
};

export type WorkspaceSearchResult = Omit<GeneratedWorkspaceSearchResult, "matches"> & {
  matches: WorkspaceSearchMatch[];
};

export type GraphNode = Omit<
  GeneratedGraphNode,
  "source" | "evidence" | "metadata"
> & {
  source?: SourceLocation;
  evidence: EvidenceKind;
  metadata: GraphMetadata;
};

export type GraphEdge = Omit<GeneratedGraphEdge, "evidence" | "metadata"> & {
  evidence: EvidenceKind;
  metadata: GraphMetadata;
};

export type GraphSnapshot = Omit<GeneratedGraphSnapshot, "nodes" | "edges"> & {
  nodes: GraphNode[];
  edges: GraphEdge[];
};

export type RuntimeEvent = Omit<
  GeneratedRuntimeEvent,
  "evidence" | "metadata"
> & {
  evidence: EvidenceKind;
  metadata: GraphMetadata;
};

export type ApiRequest = Omit<GeneratedApiRequest, "headers"> & {
  headers: ApiHeader[];
};

export type ApiResponse = Omit<GeneratedApiResponse, "headers"> & {
  headers: ApiHeader[];
};

export type EvidenceReference = Omit<
  GeneratedEvidenceReference,
  "evidence"
> & {
  evidence: EvidenceKind;
};

export type AiExplanation = Omit<GeneratedAiExplanation, "evidence"> & {
  evidence: EvidenceReference[];
};

export type AppSnapshot = Omit<GeneratedAppSnapshot, "workspace"> & {
  workspace?: WorkspaceSummary;
};

export type WebSocketConnectRequest = Omit<
  GeneratedWebSocketConnectRequest,
  "headers"
> & {
  headers: ApiHeader[];
};

export type WebSocketConnectResult = GeneratedWebSocketConnectResult;

export type WebSocketSendRequest = Omit<GeneratedWebSocketSendRequest, "encoding"> & {
  encoding: WebSocketEncoding;
};

export type WebSocketSendResult = GeneratedWebSocketSendResult;
export type WebSocketDisconnectRequest = GeneratedWebSocketDisconnectRequest;
export type WebSocketDisconnectResult = GeneratedWebSocketDisconnectResult;

export type WebSocketEvent = Omit<
  GeneratedWebSocketEvent,
  "kind" | "timestamp" | "encoding"
> & {
  kind: WebSocketEventKind;
  timestamp: string;
  encoding?: WebSocketEncoding;
};
