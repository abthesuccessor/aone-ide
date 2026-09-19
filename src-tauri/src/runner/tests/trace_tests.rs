use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc},
    time::Duration,
};

use tempfile::tempdir;

use crate::{
    domain::{DebugSessionStatus, EvidenceKind},
    runner::{RuntimeState, debugger::debug_control_channel},
};

#[test]
fn trace_registry_enforces_parent_and_phase_transitions() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:trace".into(), 42);

    assert!(
        state
            .accept_trace_span(&session, "trace", "child", Some("parent"), "start")
            .is_err()
    );
    state
        .accept_trace_span(&session, "trace", "parent", None, "start")
        .unwrap();
    state
        .accept_trace_span(&session, "trace", "child", Some("parent"), "start")
        .unwrap();
    state
        .accept_trace_span(&session, "trace", "child", Some("parent"), "end")
        .unwrap();
    assert!(
        state
            .accept_trace_span(&session, "trace", "child", Some("parent"), "end")
            .is_err()
    );
    assert!(
        state
            .accept_trace_span(&session, "trace", "parent", Some("wrong"), "end")
            .is_err()
    );
    state
        .accept_trace_span(&session, "trace", "parent", None, "end")
        .unwrap();
}

#[test]
fn trace_registry_caps_total_events_per_run_and_clears_on_exit() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:first".into(), 42);
    for index in 0..4_096 {
        let span = format!("span-{index}");
        state
            .accept_trace_span(&session, "trace", &span, None, "start")
            .unwrap();
        state
            .accept_trace_span(&session, "trace", &span, None, "end")
            .unwrap();
    }
    let error = state
        .accept_trace_span(&session, "trace", "overflow", None, "event")
        .unwrap_err();
    assert!(error.contains("event capacity"));

    state.record_leader_exit("run:first").unwrap();
    assert!(
        state
            .accept_trace_span(&session, "trace", "late", None, "event")
            .is_err()
    );
    let next = state
        .reserve_run_start()
        .unwrap()
        .activate("run:next".into(), 43);
    state
        .accept_trace_span(&next, "trace", "fresh", None, "event")
        .unwrap();
}

#[test]
fn failed_stop_signal_rollback_does_not_cancel_stream_session() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:stop".into(), 42);
    let cancellation = session.cancellation();

    assert_eq!(state.mark_stopping("run:stop"), Some(42));
    assert!(state.run_session_is_current(&session));
    assert!(!*cancellation.borrow());
    state.cancel_stopping("run:stop", 42);
    assert!(state.run_session_is_current(&session));
    assert!(!*cancellation.borrow());

    assert_eq!(state.mark_stopping("run:stop"), Some(42));
    state.confirm_stopping("run:stop", 42);
    assert!(!state.run_session_is_current(&session));
    assert!(*cancellation.borrow());
}

#[test]
fn stop_signal_delivery_does_not_claim_exit_before_leader_reap() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:debug-stop".into(), 42);
    let (sender, _receiver) = debug_control_channel();
    state
        .register_debug_session(&session, "debug:stop".into(), sender)
        .unwrap();

    assert_eq!(state.mark_stopping("run:debug-stop"), Some(42));
    state.confirm_stopping("run:debug-stop", 42);
    assert_eq!(
        state.debug_session("run:debug-stop").unwrap().status,
        DebugSessionStatus::Starting
    );

    state.record_leader_exit("run:debug-stop").unwrap();
    assert_eq!(
        state.debug_session("run:debug-stop").unwrap().status,
        DebugSessionStatus::Stopped
    );
}

#[test]
fn wait_failure_keeps_exit_unknown_until_bounded_stop_cleanup() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:wait-failed".into(), 42);
    let (sender, _receiver) = debug_control_channel();
    state
        .register_debug_session(&session, "debug:wait-failed".into(), sender)
        .unwrap();

    let failed = state.record_wait_failure("run:wait-failed").unwrap();
    assert!(!failed.leader_exited);
    assert!(failed.exit_observation_failed);
    assert!(state.run_session_is_current(&session));
    let snapshot = state.debug_session("run:wait-failed").unwrap();
    assert_eq!(snapshot.status, DebugSessionStatus::Failed);
    assert!(snapshot.error.unwrap().contains("process state is unknown"));
    assert!(state.reserve_run_start().is_err());

    assert_eq!(state.mark_stopping("run:wait-failed"), Some(42));
    state.confirm_stopping("run:wait-failed", 42);
    state.complete_stop_escalation("run:wait-failed", 42);
    assert_eq!(
        state.debug_session("run:wait-failed").unwrap().status,
        DebugSessionStatus::Failed
    );
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn wait_failure_after_stop_escalation_releases_slot_without_claiming_exit() {
    let state = RuntimeState::default();
    let session = state
        .reserve_run_start()
        .unwrap()
        .activate("run:late-wait-failure".into(), 42);
    let (sender, _receiver) = debug_control_channel();
    state
        .register_debug_session(&session, "debug:late-wait-failure".into(), sender)
        .unwrap();

    assert_eq!(state.mark_stopping("run:late-wait-failure"), Some(42));
    state.confirm_stopping("run:late-wait-failure", 42);
    state.complete_stop_escalation("run:late-wait-failure", 42);
    assert!(state.reserve_run_start().is_err());

    let failed = state.record_wait_failure("run:late-wait-failure").unwrap();
    assert!(!failed.leader_exited);
    assert!(failed.exit_observation_failed);
    assert!(failed.escalation_complete);
    assert_eq!(
        state.debug_session("run:late-wait-failure").unwrap().status,
        DebugSessionStatus::Failed
    );
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn terminal_event_commit_precedes_workspace_switch_slot_release() {
    let state = Arc::new(RuntimeState::default());
    let first = tempdir().unwrap();
    state.begin_workspace_switch().unwrap();
    state.complete_workspace_switch(first.path().canonicalize().unwrap());
    state
        .reserve_run_start()
        .unwrap()
        .activate("run:terminal".into(), 42);

    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let exit_state = Arc::clone(&state);
    let exit = std::thread::spawn(move || {
        exit_state
            .record_leader_exit_with("run:terminal", |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let event = exit_state.next_event(
                    Some("run:terminal".into()),
                    "process.exited",
                    "exited",
                    EvidenceKind::Observed,
                    Some("run:terminal".into()),
                    BTreeMap::new(),
                );
                exit_state.record_event(event);
            })
            .unwrap();
    });
    entered_rx.recv().unwrap();

    let switch_state = Arc::clone(&state);
    let (switch_tx, switch_rx) = mpsc::channel();
    let switch = std::thread::spawn(move || {
        switch_tx
            .send(switch_state.begin_workspace_switch())
            .unwrap();
    });
    assert!(switch_rx.recv_timeout(Duration::from_millis(50)).is_err());
    release_tx.send(()).unwrap();
    exit.join().unwrap();
    assert!(switch_rx.recv().unwrap().is_ok());
    switch.join().unwrap();
    assert!(state.list_events(None).is_empty());

    let second = tempdir().unwrap();
    state.complete_workspace_switch(second.path().canonicalize().unwrap());
    assert!(state.list_events(None).is_empty());
}

#[test]
fn event_snapshot_is_rejected_during_switch_and_old_events_are_cleared() {
    let state = RuntimeState::default();
    let first = tempdir().unwrap();
    let first_root = first.path().canonicalize().unwrap();
    state.begin_workspace_switch().unwrap();
    state.complete_workspace_switch(first_root.clone());
    state.record_event(state.next_event(
        None,
        "test",
        "old workspace event",
        EvidenceKind::Observed,
        None,
        BTreeMap::new(),
    ));
    assert_eq!(
        state
            .list_events_for_workspace(&first_root, None)
            .unwrap()
            .len(),
        1
    );

    state.begin_workspace_switch().unwrap();
    assert!(state.list_events(None).is_empty());
    assert!(state.list_events_for_workspace(&first_root, None).is_err());
    let second = tempdir().unwrap();
    let second_root = second.path().canonicalize().unwrap();
    state.complete_workspace_switch(second_root.clone());
    assert!(
        state
            .list_events_for_workspace(&second_root, None)
            .unwrap()
            .is_empty()
    );
    assert!(state.list_events_for_workspace(&first_root, None).is_err());
}
