use super::*;
use crate::domain::{
    DebugControlAction, DebugControlRequest, DebugDataPreviewKind, DebugSessionStatus,
};

fn event(
    workflow: &str,
    sequence: u64,
    epoch: u64,
    state: DebugSafePointState,
) -> DebugWorkflowEvent {
    event_with_step(
        workflow,
        sequence,
        epoch,
        state,
        &format!("step-{sequence}"),
    )
}

fn event_with_step(
    workflow: &str,
    sequence: u64,
    epoch: u64,
    state: DebugSafePointState,
    step_id: &str,
) -> DebugWorkflowEvent {
    DebugWorkflowEvent {
        id: format!("event-{sequence}"),
        workflow_id: workflow.into(),
        sequence,
        control_epoch: epoch,
        step_id: step_id.into(),
        parent_step_id: None,
        kind: DebugEventKind::Line,
        label: format!("Line {sequence}"),
        flow_stage: DebugFlowStage::Backend,
        source: DebugSourceLocation {
            relative_path: "src/server.js".into(),
            line: sequence as usize,
            line_end: None,
        },
        safe_point_state: state,
        branch_outcome: None,
        operation: None,
        resource: None,
        data_preview: None,
        timestamp: utc_timestamp(),
    }
}

fn control(action: DebugControlAction, sequence: u64, control_epoch: u64) -> DebugControlRequest {
    DebugControlRequest {
        run_id: "run:one".into(),
        debug_session_id: "debug:one".into(),
        action,
        expected_sequence: sequence,
        expected_control_epoch: control_epoch,
    }
}

#[test]
fn step_releases_exactly_one_fresh_safe_point() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event("workflow:one", 1, 0, DebugSafePointState::Paused),
        )
        .unwrap();
    assert!(
        registry
            .accept(
                "run:one",
                event("workflow:one", 2, 0, DebugSafePointState::Paused),
            )
            .is_err()
    );

    let queued = registry
        .queue_control(&control(DebugControlAction::StepInto, 1, 0))
        .unwrap();
    assert_eq!(queued.control_epoch, 1);
    assert_eq!(queued.acknowledged_control_epoch, 0);
    registry
        .accept(
            "run:one",
            event("workflow:one", 2, 1, DebugSafePointState::Paused),
        )
        .unwrap();
    assert!(
        registry
            .accept(
                "run:one",
                event("workflow:one", 3, 1, DebugSafePointState::Paused),
            )
            .is_err()
    );
}

#[test]
fn pause_cutover_accepts_in_flight_old_epoch_before_new_epoch_ack() {
    let mut registry = DebugRegistry::default();
    let (sender, mut receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event_with_step(
                "workflow:one",
                1,
                0,
                DebugSafePointState::Paused,
                "route-entry",
            ),
        )
        .unwrap();
    registry
        .queue_control(&control(DebugControlAction::Resume, 1, 0))
        .unwrap();
    assert_eq!(receiver.try_recv().unwrap().expected_sequence, 1);
    registry
        .accept(
            "run:one",
            event("workflow:one", 2, 1, DebugSafePointState::Running),
        )
        .unwrap();
    registry
        .accept(
            "run:one",
            event("workflow:one", 3, 1, DebugSafePointState::Running),
        )
        .unwrap();
    // The renderer snapshot is one event behind. Pause is bound atomically to
    // the registry's actual current sequence while the control epoch remains exact.
    registry
        .queue_control(&control(DebugControlAction::Pause, 2, 1))
        .unwrap();
    assert_eq!(receiver.try_recv().unwrap().expected_sequence, 3);
    registry
        .accept(
            "run:one",
            event("workflow:one", 4, 1, DebugSafePointState::Running),
        )
        .unwrap();
    registry
        .accept(
            "run:one",
            event("workflow:one", 5, 2, DebugSafePointState::Paused),
        )
        .unwrap();
    let snapshot = registry.snapshot("run:one").unwrap();
    assert_eq!(snapshot.status, DebugSessionStatus::Paused);
    assert_eq!(snapshot.acknowledged_control_epoch, 2);
    assert_eq!(snapshot.event_count, 5);

    let mut stale_epoch = control(DebugControlAction::Resume, 5, 1);
    stale_epoch.expected_control_epoch = 0;
    assert!(registry.queue_control(&stale_epoch).is_err());
}

#[test]
fn two_serial_workflows_can_complete_in_one_server_run() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event_with_step(
                "workflow:one",
                1,
                0,
                DebugSafePointState::Paused,
                "route-entry",
            ),
        )
        .unwrap();
    registry
        .queue_control(&control(DebugControlAction::StepOver, 1, 0))
        .unwrap();
    registry
        .accept(
            "run:one",
            event_with_step(
                "workflow:one",
                2,
                1,
                DebugSafePointState::WorkflowCompleted,
                "response",
            ),
        )
        .unwrap();
    registry
        .accept(
            "run:one",
            event_with_step(
                "workflow:two",
                3,
                1,
                DebugSafePointState::Paused,
                "route-entry",
            ),
        )
        .unwrap();
    assert_eq!(
        registry.snapshot("run:one").unwrap().active_workflow_id,
        Some("workflow:two".into())
    );
    assert_eq!(registry.snapshot("run:one").unwrap().workflow_count, 2);
}

#[test]
fn timeout_terminalizes_only_debug_protocol_and_rejects_late_ack() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event("workflow:one", 1, 0, DebugSafePointState::Paused),
        )
        .unwrap();
    registry
        .queue_control(&control(DebugControlAction::StepInto, 1, 0))
        .unwrap();
    registry.expire_control("run:one", 1);
    let snapshot = registry.snapshot("run:one").unwrap();
    assert_eq!(snapshot.status, DebugSessionStatus::Failed);
    assert!(
        snapshot
            .error
            .unwrap()
            .contains("protocol session is terminal")
    );
    assert!(
        registry
            .accept(
                "run:one",
                event("workflow:one", 2, 1, DebugSafePointState::Paused),
            )
            .is_err()
    );
}

#[test]
fn preview_is_shape_only_bounded_and_redacts_sensitive_field_names() {
    let mut preview = DebugDataPreview {
        kind: DebugDataPreviewKind::Object,
        type_name: Some("Shipment".into()),
        fields: vec![DebugDataField {
            name: "apiKey".into(),
            value_type: "string".into(),
            nullable: false,
        }],
        item_count: Some(1),
        row_count: None,
        null_count: Some(0),
        truncated: false,
    };
    sanitize_preview(&mut preview).unwrap();
    assert_eq!(preview.fields[0].name, "[sensitive-field]");
    preview.row_count = Some(MAX_PREVIEW_COUNT + 1);
    assert!(sanitize_preview(&mut preview).is_err());
}

#[test]
fn preview_wire_rejects_unknown_value_metadata_and_missing_required_shape_fields() {
    let preview = json!({
        "kind": "object",
        "fields": [{"name": "status", "valueType": "string", "nullable": false}],
        "truncated": false
    });
    let parse = |data_preview: Value| {
        serde_json::from_value::<DebugEnvelope>(json!({
            "nonce": "nonce",
            "debugSessionId": "debug:one",
            "workflowId": "workflow:one",
            "sequence": 1,
            "controlEpoch": 0,
            "stepId": "route-entry",
            "kind": "line",
            "flowStage": "backend",
            "source": {"relativePath": "src/server.js", "line": 1},
            "safePointState": "paused",
            "dataPreview": data_preview
        }))
    };
    assert!(parse(preview.clone()).is_ok());

    let mut with_value = preview.clone();
    with_value["value"] = json!("must-not-be-ignored");
    assert!(parse(with_value).is_err());

    let mut with_metadata = preview.clone();
    with_metadata["fields"][0]["metadata"] = json!({"raw": "must-not-be-ignored"});
    assert!(parse(with_metadata).is_err());

    let mut without_fields = preview.clone();
    without_fields.as_object_mut().unwrap().remove("fields");
    assert!(parse(without_fields).is_err());

    let mut without_nullable = preview;
    without_nullable["fields"][0]
        .as_object_mut()
        .unwrap()
        .remove("nullable");
    assert!(parse(without_nullable).is_err());
}

#[test]
fn source_semantic_and_control_wire_guards_are_strict() {
    let source = |relative_path: &str| DebugSource {
        relative_path: relative_path.into(),
        line: 1,
        line_end: None,
        node_id: None,
    };
    assert!(valid_source(&source("src/server.js")));
    assert!(!valid_source(&source(".env")));
    assert!(!valid_source(&source("../outside.js")));
    assert!(valid_semantic("ShipmentRepository.findById"));
    assert!(!valid_semantic("SELECT * WHERE id='secret'"));
    assert!(!valid_semantic("method\nsecret"));

    let context = DebugSessionContext {
        nonce: Arc::new(Zeroizing::new("nonce".into())),
        workspace_id: "workspace".into(),
        debug_session_id: "debug:one".into(),
    };
    let encoded = String::from_utf8(encode_control_line(
        &context,
        &QueuedDebugControl {
            action: DebugControlAction::StepOver,
            control_epoch: 4,
            expected_sequence: 9,
        },
    ))
    .unwrap();
    assert_eq!(
        encoded,
        "AONE_DEBUG_CONTROL_V1 {\"nonce\":\"nonce\",\"debugSessionId\":\"debug:one\",\"action\":\"stepOver\",\"controlEpoch\":4,\"expectedSequence\":9}\n"
    );
}

#[test]
fn first_protocol_failure_is_terminal_and_later_sequences_cannot_spam() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event("workflow:one", 1, 0, DebugSafePointState::Paused),
        )
        .unwrap();
    registry.control_failed("run:one", "invalid bounded debug JSON object".into());
    assert!(
        registry
            .accept(
                "run:one",
                event("workflow:one", 2, 0, DebugSafePointState::Paused),
            )
            .is_err()
    );
    let snapshot = registry.snapshot("run:one").unwrap();
    assert_eq!(snapshot.status, DebugSessionStatus::Failed);
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(
        snapshot.error.as_deref(),
        Some("invalid bounded debug JSON object")
    );
}

#[test]
fn snapshot_returns_a_truthful_bounded_suffix() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event("workflow:one", 1, 0, DebugSafePointState::Paused),
        )
        .unwrap();
    registry
        .queue_control(&control(DebugControlAction::Resume, 1, 0))
        .unwrap();
    for sequence in 2..=251 {
        registry
            .accept(
                "run:one",
                event("workflow:one", sequence, 1, DebugSafePointState::Running),
            )
            .unwrap();
    }
    let snapshot = registry.snapshot("run:one").unwrap();
    assert_eq!(snapshot.event_count, 251);
    assert_eq!(snapshot.events.len(), 250);
    assert!(snapshot.events_truncated);
    assert_eq!(snapshot.events_start_sequence, Some(2));
}

#[test]
fn workflow_failure_survives_process_session_completion_as_a_distinct_outcome() {
    let mut registry = DebugRegistry::default();
    let (sender, _receiver) = debug_control_channel();
    registry.start("run:one".into(), "debug:one".into(), sender);
    registry
        .accept(
            "run:one",
            event("workflow:one", 1, 0, DebugSafePointState::Paused),
        )
        .unwrap();
    registry
        .queue_control(&control(DebugControlAction::Resume, 1, 0))
        .unwrap();
    registry
        .accept(
            "run:one",
            event("workflow:one", 2, 1, DebugSafePointState::WorkflowFailed),
        )
        .unwrap();
    registry.process_exited("run:one", false);
    let snapshot = registry.snapshot("run:one").unwrap();
    assert_eq!(snapshot.status, DebugSessionStatus::Completed);
    assert_eq!(
        snapshot.last_workflow_state,
        Some(DebugSafePointState::WorkflowFailed)
    );
    assert_eq!(snapshot.workflow_count, 1);
}

#[test]
fn loaded_secret_substrings_are_rejected_in_process_reported_names() {
    let envelope: DebugEnvelope = serde_json::from_value(json!({
        "nonce": "nonce",
        "debugSessionId": "debug:one",
        "workflowId": "workflow:one",
        "sequence": 1,
        "controlEpoch": 0,
        "stepId": "token-needle",
        "kind": "database",
        "flowStage": "data",
        "source": { "relativePath": "src/server.js", "line": 1 },
        "safePointState": "paused",
        "operation": "SELECT one row",
        "resource": "shipments",
        "dataPreview": {
            "kind": "object",
            "typeName": "Shipment",
            "fields": [],
            "truncated": false
        }
    }))
    .unwrap();
    let secrets = vec![Zeroizing::new("needle".into())];
    assert!(envelope_contains_known_secret(&envelope, &secrets));
}
