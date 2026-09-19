use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::EvidenceKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProfile {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub executable: String,
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd_relative: Option<String>,
    #[serde(default)]
    pub required_env: Vec<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRunRequest {
    pub profile_id: String,
    pub env_names: Vec<String>,
    pub observe: bool,
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunResult {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRunResult {
    pub stopped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRunRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListRuntimeEventsRequest {
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEvent {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub kind: String,
    pub timestamp: String,
    pub label: String,
    pub evidence: EvidenceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugControlAction {
    Pause,
    Resume,
    StepInto,
    StepOver,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugSessionStatus {
    Starting,
    Running,
    Paused,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugSafePointState {
    Running,
    Paused,
    WorkflowCompleted,
    WorkflowFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugEventKind {
    Request,
    Method,
    Line,
    Branch,
    Database,
    Agent,
    External,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugFlowStage {
    Trigger,
    Api,
    Backend,
    Service,
    Data,
    External,
    Response,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugBranchOutcome {
    Then,
    Else,
    Case,
    Loop,
    ShortCircuit,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DebugDataPreviewKind {
    None,
    Scalar,
    Object,
    Array,
    RowSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DebugDataField {
    pub name: String,
    pub value_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DebugDataPreview {
    pub kind: DebugDataPreviewKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    pub fields: Vec<DebugDataField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_count: Option<u64>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSourceLocation {
    pub relative_path: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugWorkflowEvent {
    pub id: String,
    pub workflow_id: String,
    pub sequence: u64,
    pub control_epoch: u64,
    pub step_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_step_id: Option<String>,
    pub kind: DebugEventKind,
    pub label: String,
    pub flow_stage: DebugFlowStage,
    pub source: DebugSourceLocation,
    pub safe_point_state: DebugSafePointState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_outcome: Option<DebugBranchOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_preview: Option<DebugDataPreview>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugActionCapability {
    pub supported: bool,
    pub cooperative: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugCapabilities {
    pub pause: DebugActionCapability,
    pub resume: DebugActionCapability,
    pub step_into: DebugActionCapability,
    pub step_over: DebugActionCapability,
    pub stop: DebugActionCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSessionSnapshot {
    pub debug_session_id: String,
    pub run_id: String,
    pub status: DebugSessionStatus,
    pub event_count: usize,
    pub control_epoch: u64,
    pub acknowledged_control_epoch: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_source: Option<DebugSourceLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_action: Option<DebugControlAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_control_epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_workflow_id: Option<String>,
    pub workflow_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_workflow_state: Option<DebugSafePointState>,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub capabilities: DebugCapabilities,
    pub limitation: String,
    pub events: Vec<DebugWorkflowEvent>,
    pub events_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_start_sequence: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDebugSessionRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugControlRequest {
    pub run_id: String,
    pub debug_session_id: String,
    pub action: DebugControlAction,
    pub expected_sequence: u64,
    pub expected_control_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugControlResult {
    pub accepted: bool,
    pub snapshot: DebugSessionSnapshot,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::StartRunRequest;

    #[test]
    fn start_run_request_accepts_only_profile_selection_and_mode() {
        let request = serde_json::from_value::<StartRunRequest>(json!({
            "profileId": "profile:dev",
            "envNames": ["PORT"],
            "observe": true,
            "debug": false
        }))
        .unwrap();
        assert_eq!(request.profile_id, "profile:dev");
        assert_eq!(request.env_names, ["PORT"]);

        let error = serde_json::from_value::<StartRunRequest>(json!({
            "profileId": "profile:dev",
            "executable": "/tmp/unregistered-command",
            "args": ["--unsafe-override"],
            "envNames": [],
            "observe": false,
            "debug": false
        }))
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("unknown field"));
        assert!(message.contains("`args`") || message.contains("`executable`"));
    }
}
