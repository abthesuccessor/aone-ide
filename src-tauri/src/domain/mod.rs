mod ai;
mod api;
mod api_inventory;
mod devtools;
mod editor;
mod graph;
mod observability;
mod onboarding;
mod project_environment;
mod runtime;
mod search;
mod terminal;
mod websocket;
mod workspace;

pub use ai::{
    AiCliAdapterStatus, AiConfigurationStatus, AiExplainProjectEnvironmentRequest,
    AiExplainRequest, AiExplanation, AiProjectAgentErrorCode, AiProjectAgentRequest,
    AiProjectAgentResponse, ConfigureAiCliRequest, ConfigureHostedAiRequest,
    ConfigureOllamaRequest, EvidenceReference,
};
pub use api::{ApiHeader, ApiRequest, ApiResponse};
pub use api_inventory::{
    ApiClientCoverage, ApiEndpoint, ApiEndpointGrouping, ApiEndpointHandler, ApiEndpointPage,
    ApiEndpointRole, ApiGroupingBasis, ApiInventoryCounts, ApiProtocol, ListApiEndpointsRequest,
};
pub use devtools::{
    GitCommitRequest, GitDiffRequest, GitDiffResult, GitFileStatus, GitMutationResult,
    GitPathsRequest, GitStatusResult, ToolConfiguration, ToolConfigurationKind,
    ToolConfigurationOpenRequest, ToolConfigurationOpenResult, ToolConfigurationScope,
    ToolInspectionState,
};
pub use editor::{
    FormatDocumentRequest, FormatDocumentResult, FormatterCapability, WriteWorkspaceFileRequest,
};
pub use graph::{
    CapabilityLevel, EvidenceKind, GraphEdge, GraphNode, GraphProjection, GraphQuery,
    GraphSnapshot, SourceLocation,
};
pub use observability::{
    DeleteRuntimeTraceRequest, DeleteRuntimeTraceResult, OtlpReceiverLifecycle,
    OtlpReceiverSnapshot, StartOtlpReceiverRequest,
};
pub use onboarding::{
    CloneGithubRepositoryRequest, CreateDocumentsProjectRequest, GitIdentitySource,
    GitOnboardingIdentity, GitOnboardingPublicKey, GitOnboardingRemote, GitOnboardingReport,
    GitRemoteTransport, InspectGitOnboardingRequest, ProjectBootstrapAction,
    ProjectBootstrapResult,
};
pub use project_environment::{
    InspectProjectEnvironmentRequest, ProjectEnvironmentEvidence, ProjectEnvironmentRecommendation,
    ProjectEnvironmentRecommendationKind, ProjectEnvironmentRecommendationSeverity,
    ProjectEnvironmentReport, ProjectStack, ProjectStackConfidence, ProjectTool, ProjectToolStatus,
};
pub use runtime::{
    DebugActionCapability, DebugBranchOutcome, DebugCapabilities, DebugControlAction,
    DebugControlRequest, DebugControlResult, DebugDataField, DebugDataPreview,
    DebugDataPreviewKind, DebugEventKind, DebugFlowStage, DebugSafePointState,
    DebugSessionSnapshot, DebugSessionStatus, DebugSourceLocation, DebugWorkflowEvent,
    GetDebugSessionRequest, ListRuntimeEventsRequest, RunProfile, RuntimeEvent, StartRunRequest,
    StartRunResult, StopRunRequest, StopRunResult,
};
pub use search::{
    WorkspaceSearchMatch, WorkspaceSearchMatchKind, WorkspaceSearchRequest, WorkspaceSearchResult,
};
pub use terminal::{
    TerminalActionResult, TerminalCloseRequest, TerminalEvent, TerminalEventKind,
    TerminalInputEncoding, TerminalOpenRequest, TerminalOpenResult, TerminalOutputEncoding,
    TerminalProfile, TerminalResizeRequest, TerminalWriteRequest,
};
pub use websocket::{
    WebSocketConnectRequest, WebSocketConnectResult, WebSocketDisconnectRequest,
    WebSocketDisconnectResult, WebSocketEvent, WebSocketEventKind, WebSocketPayloadEncoding,
    WebSocketSendRequest, WebSocketSendResult,
};
pub use workspace::{
    AppSnapshot, EnvLoadResult, LanguageSummary, OpenFileResult, ScanProgress, SourceFile,
    WorkspaceChanged, WorkspaceFile, WorkspaceSummary,
};
