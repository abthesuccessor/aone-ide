use std::{collections::BTreeMap, collections::HashMap};

use serde_json::json;
use tempfile::tempdir;
use tokio::io::BufReader;
use zeroize::Zeroizing;

use super::{
    environment::{
        MAX_ENV_NAME_REQUESTS, MAX_SELECTED_ENV_NAMES, SAFE_PARENT_ENV, redact_args, redact_json,
        validate_args, validate_run_env_name,
    },
    events::process_started_metadata,
    output::{MAX_STREAM_LINE_BYTES, read_bounded_line},
    preparation::{
        PathKind, capture_path_identity, prepare_run, resolve_cwd, resolve_executable,
        revalidate_path_identity,
    },
    state::RuntimeState,
    timestamp::{civil_date_from_unix_days, utc_timestamp},
};
use crate::{
    domain::{EvidenceKind, RunProfile, StartRunRequest},
    error::AoneError,
};

mod trace_tests;

#[test]
fn child_processes_do_not_inherit_credential_brokers() {
    assert!(!SAFE_PARENT_ENV.contains(&"SSH_AUTH_SOCK"));
}

#[test]
fn environment_api_exposes_names_not_values() {
    let state = RuntimeState::default();
    state
        .replace_env(HashMap::from([
            ("PUBLIC_PORT".into(), "3000".into()),
            ("DATABASE_PASSWORD".into(), "do-not-return".into()),
        ]))
        .unwrap();

    assert_eq!(
        state.env_names(),
        vec!["DATABASE_PASSWORD".to_string(), "PUBLIC_PORT".to_string()]
    );
    assert!(!format!("{:?}", state.env_names()).contains("do-not-return"));
}

#[test]
fn invalid_environment_names_are_rejected() {
    let state = RuntimeState::default();
    let error = state
        .replace_env(HashMap::from([("BAD-NAME".into(), "value".into())]))
        .unwrap_err();
    assert!(error.to_string().contains("invalid environment variable"));
}

#[test]
fn loader_hooks_and_parent_controls_are_rejected() {
    for name in [
        "PATH",
        "HOME",
        "BASH_ENV",
        "NODE_OPTIONS",
        "PYTHONPATH",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "RUSTC_WRAPPER",
        "JAVA_TOOL_OPTIONS",
        "DOTNET_STARTUP_HOOKS",
        "GIT_CONFIG_GLOBAL",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER",
    ] {
        let error = validate_run_env_name(name).unwrap_err();
        assert!(
            error.to_string().contains("is not allowed"),
            "{name}: {error}"
        );
    }
    validate_run_env_name("PORT").unwrap();
    validate_run_env_name("DATABASE_URL").unwrap();

    let state = RuntimeState::default();
    let error = state
        .replace_env(HashMap::from([("PATH".into(), "/tmp/attacker".into())]))
        .unwrap_err();
    assert!(error.to_string().contains("PATH"));
}

#[test]
fn environment_selection_is_deduplicated_and_bounded() {
    let state = RuntimeState::default();
    state
        .replace_env(HashMap::from([
            ("PORT".into(), "4310".into()),
            ("DATABASE_URL".into(), "postgres://local".into()),
        ]))
        .unwrap();

    assert_eq!(
        state
            .child_environment(&["PORT".into(), "DATABASE_URL".into(), "PORT".into(),])
            .unwrap(),
        vec![
            ("DATABASE_URL".into(), "postgres://local".into()),
            ("PORT".into(), "4310".into()),
        ]
    );

    let oversized = vec!["PORT".to_string(); MAX_ENV_NAME_REQUESTS + 1];
    let error = state.child_environment(&oversized).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("too many environment name selections")
    );

    let distinct_values = (0..=MAX_SELECTED_ENV_NAMES)
        .map(|index| (format!("SAFE_VALUE_{index}"), index.to_string()))
        .collect::<HashMap<_, _>>();
    let distinct_names = distinct_values.keys().cloned().collect::<Vec<_>>();
    let distinct_state = RuntimeState::default();
    distinct_state.replace_env(distinct_values).unwrap();
    let error = distinct_state
        .child_environment(&distinct_names)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("too many distinct environment names")
    );
}

#[test]
fn empty_environment_request_injects_nothing() {
    let state = RuntimeState::default();
    state
        .replace_env(HashMap::from([
            ("PORT".into(), "4310".into()),
            ("DATABASE_URL".into(), "postgres://local".into()),
        ]))
        .unwrap();

    assert!(state.child_environment(&[]).unwrap().is_empty());
    assert_eq!(
        state.child_environment(&["PORT".into()]).unwrap(),
        vec![("PORT".into(), "4310".into())]
    );
}

#[test]
fn workspace_switch_rejects_active_run_and_resets_scoped_state() {
    let state = RuntimeState::default();
    state
        .replace_env(HashMap::from([("PORT".into(), "4310".into())]))
        .unwrap();
    state
        .reserve_run_start()
        .unwrap()
        .activate("run:test".into(), 42);
    assert!(state.begin_workspace_switch().is_err());
    state.remove_run("run:test");

    let root = tempdir().unwrap();
    state.begin_workspace_switch().unwrap();
    state.complete_workspace_switch(root.path().canonicalize().unwrap());
    assert!(state.env_names().is_empty());
    assert!(state.list_events(None).is_empty());
    assert_eq!(
        state.workspace_root().unwrap(),
        root.path().canonicalize().unwrap()
    );
}

#[test]
fn only_one_run_can_be_starting_or_active() {
    let state = RuntimeState::default();

    let pending = state.reserve_run_start().unwrap();
    let error = state.reserve_run_start().err().unwrap();
    assert!(error.to_string().contains("already starting or running"));
    drop(pending);

    state
        .reserve_run_start()
        .unwrap()
        .activate("run:only".into(), 42);
    let error = state.reserve_run_start().err().unwrap();
    assert!(error.to_string().contains("already starting or running"));
    state.remove_run("run:only");
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn stop_keeps_global_slot_until_leader_exit_and_escalation_complete() {
    let state = RuntimeState::default();
    state
        .reserve_run_start()
        .unwrap()
        .activate("run:stopping".into(), 42);
    assert_eq!(state.mark_stopping("run:stopping"), Some(42));

    let exited = state.record_leader_exit("run:stopping").unwrap();
    assert!(exited.stopping);
    assert!(state.reserve_run_start().is_err());
    assert_eq!(state.stopping_pid("run:stopping"), Some(42));

    state.complete_stop_escalation("run:stopping", 42);
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn stop_keeps_global_slot_when_escalation_completes_before_leader_reap() {
    let state = RuntimeState::default();
    state
        .reserve_run_start()
        .unwrap()
        .activate("run:stopping".into(), 42);
    assert_eq!(state.mark_stopping("run:stopping"), Some(42));

    state.complete_stop_escalation("run:stopping", 42);
    assert!(state.reserve_run_start().is_err());
    let exited = state.record_leader_exit("run:stopping").unwrap();
    assert!(exited.escalation_complete);
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn normal_leader_exit_releases_global_slot() {
    let state = RuntimeState::default();
    state
        .reserve_run_start()
        .unwrap()
        .activate("run:complete".into(), 42);
    let exited = state.record_leader_exit("run:complete").unwrap();
    assert!(!exited.stopping);
    assert!(state.reserve_run_start().is_ok());
}

#[test]
fn explicit_run_action_has_no_native_dialog_dependency() {
    let command_source = include_str!("commands.rs");
    assert!(!command_source.contains("tauri_plugin_dialog"));
    assert!(!command_source.contains(".dialog()"));
    assert!(!command_source.contains("Confirm local process"));
}

#[test]
fn run_preparation_uses_only_the_backend_registered_profile() {
    let workspace = tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let source = root.join("package.json");
    std::fs::write(&source, "{}").unwrap();
    let state = RuntimeState::default();
    state.complete_workspace_switch(root.clone());
    state
        .replace_profiles(vec![RunProfile {
            id: "profile:test".into(),
            name: "Backend profile".into(),
            kind: "command".into(),
            executable: "/usr/bin/env".into(),
            args: vec!["--token".into(), "do-not-display".into()],
            cwd_relative: None,
            required_env: Vec::new(),
            source: "package.json".into(),
        }])
        .unwrap();
    let request = StartRunRequest {
        profile_id: "profile:test".into(),
        env_names: Vec::new(),
        observe: false,
        debug: false,
    };

    let prepared = prepare_run(&request, &state, &root).unwrap();
    assert_eq!(prepared.profile_id, "profile:test");
    assert_eq!(prepared.display_executable, "/usr/bin/env");
    assert_eq!(prepared.args, vec!["--token", "do-not-display"]);
    assert_eq!(prepared.source, source);

    let metadata = process_started_metadata(
        &root,
        &prepared,
        &["PUBLIC_PORT".into(), "DATABASE_PASSWORD".into()],
        &[Zeroizing::new("do-not-display".into())],
        42,
        true,
        false,
    );
    assert_eq!(metadata["authorization"], "explicitUserAction");
    assert_eq!(
        metadata["environmentNames"],
        json!(["DATABASE_PASSWORD", "PUBLIC_PORT"])
    );
    assert_eq!(metadata["args"], json!(["--token", "[REDACTED]"]));
    assert!(
        !serde_json::to_string(&metadata)
            .unwrap()
            .contains("do-not-display")
    );
}

#[test]
fn relative_working_directory_cannot_escape_workspace() {
    let workspace = tempdir().unwrap();
    let error = resolve_cwd(workspace.path(), Some("../outside")).unwrap_err();
    assert!(matches!(error, AoneError::PathEscape));
}

#[test]
fn relative_executable_is_resolved_from_profile_working_directory() {
    let workspace = tempdir().unwrap();
    let nested = workspace.path().join("service");
    std::fs::create_dir(&nested).unwrap();
    let executable = nested.join("gradlew");
    std::fs::write(&executable, "#!/bin/sh\n").unwrap();

    let resolved = resolve_executable(workspace.path(), &nested, "./gradlew").unwrap();
    assert_eq!(resolved.target, executable.canonicalize().unwrap());
    assert_eq!(resolved.launch_path, nested.join("./gradlew"));
}

#[cfg(unix)]
#[test]
fn explicit_symlink_keeps_launch_name_while_executing_canonical_target() {
    let workspace = tempdir().unwrap();
    let target = workspace.path().join("multicall-target");
    let launch = workspace.path().join("cargo");
    std::fs::write(&target, "target").unwrap();
    std::os::unix::fs::symlink(&target, &launch).unwrap();

    let resolved = resolve_executable(
        workspace.path(),
        workspace.path(),
        &launch.to_string_lossy(),
    )
    .unwrap();
    assert_eq!(resolved.launch_path, launch);
    assert_eq!(resolved.target, target.canonicalize().unwrap());
}

#[test]
fn path_identity_revalidation_rejects_replaced_file() {
    let workspace = tempdir().unwrap();
    let executable = workspace.path().join("tool");
    std::fs::write(&executable, "first").unwrap();
    let identity = capture_path_identity(&executable, PathKind::File, "executable").unwrap();

    std::fs::remove_file(&executable).unwrap();
    std::fs::write(&executable, "second").unwrap();
    let error = revalidate_path_identity(&executable, &identity, "executable").unwrap_err();
    assert!(error.to_string().contains("changed after validation"));
}

#[test]
fn path_identity_revalidation_rejects_in_place_file_change() {
    let workspace = tempdir().unwrap();
    let executable = workspace.path().join("tool");
    std::fs::write(&executable, "first").unwrap();
    let identity = capture_path_identity(&executable, PathKind::File, "executable").unwrap();

    std::fs::write(&executable, "changed in place").unwrap();
    let error = revalidate_path_identity(&executable, &identity, "executable").unwrap_err();
    assert!(error.to_string().contains("changed after validation"));
}

#[test]
fn directory_identity_allows_entry_changes_but_not_directory_replacement() {
    let workspace = tempdir().unwrap();
    let cwd = workspace.path().join("service");
    std::fs::create_dir(&cwd).unwrap();
    let identity = capture_path_identity(&cwd, PathKind::Directory, "working directory").unwrap();

    std::fs::write(cwd.join("new-file.txt"), "change").unwrap();
    revalidate_path_identity(&cwd, &identity, "working directory").unwrap();

    std::fs::remove_file(cwd.join("new-file.txt")).unwrap();
    std::fs::remove_dir(&cwd).unwrap();
    std::fs::create_dir(&cwd).unwrap();
    let error = revalidate_path_identity(&cwd, &identity, "working directory").unwrap_err();
    assert!(error.to_string().contains("changed after validation"));
}

#[cfg(unix)]
#[test]
fn path_identity_revalidation_rejects_swapped_launch_symlink() {
    let workspace = tempdir().unwrap();
    let first = workspace.path().join("first");
    let second = workspace.path().join("second");
    let launch = workspace.path().join("tool");
    std::fs::write(&first, "first").unwrap();
    std::fs::write(&second, "second").unwrap();
    std::os::unix::fs::symlink(&first, &launch).unwrap();
    let identity = capture_path_identity(&launch, PathKind::File, "executable").unwrap();

    std::fs::remove_file(&launch).unwrap();
    std::os::unix::fs::symlink(&second, &launch).unwrap();
    let error = revalidate_path_identity(&launch, &identity, "executable").unwrap_err();
    assert!(error.to_string().contains("changed after validation"));
}

#[test]
fn shell_characters_remain_plain_arguments() {
    let args = vec!["hello; rm -rf /".to_string(), "$(touch nope)".to_string()];
    validate_args(&args).unwrap();
    assert_eq!(args[0], "hello; rm -rf /");
    assert_eq!(args[1], "$(touch nope)");
}

#[test]
fn sensitive_arguments_and_known_values_are_redacted() {
    let secrets = vec![Zeroizing::new("known-secret".to_string())];
    let args = vec![
        "--token".into(),
        "abc".into(),
        "--password=hunter2".into(),
        "value=known-secret".into(),
    ];
    assert_eq!(
        redact_args(&args, &secrets),
        vec![
            "--token",
            "[REDACTED]",
            "--password=[REDACTED]",
            "value=[REDACTED]",
        ]
    );
}

#[test]
fn nested_json_secret_fields_are_redacted() {
    let value = json!({
        "safe": "visible",
        "nested": { "apiKey": "hidden" },
        "alsoSafe": "prefix known-secret suffix"
    });
    let secrets = vec![Zeroizing::new("known-secret".to_string())];
    let redacted = redact_json(&value, &secrets);
    assert_eq!(redacted["safe"], "visible");
    assert_eq!(redacted["nested"]["apiKey"], "[REDACTED]");
    assert_eq!(redacted["alsoSafe"], "prefix [REDACTED] suffix");
}

#[test]
fn event_listing_keeps_tail_in_chronological_order() {
    let state = RuntimeState::default();
    for index in 0..4 {
        state.record_event(state.next_event(
            None,
            "test",
            format!("event {index}"),
            EvidenceKind::Observed,
            None,
            BTreeMap::new(),
        ));
    }
    let events = state.list_events(Some(2));
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].label, "event 2");
    assert_eq!(events[1].label, "event 3");
}

#[test]
fn runtime_timestamp_is_browser_parseable_rfc3339() {
    assert_eq!(civil_date_from_unix_days(0), (1970, 1, 1));
    assert_eq!(civil_date_from_unix_days(20_454), (2026, 1, 1));
    let timestamp = utc_timestamp();
    assert!(timestamp.contains('T'));
    assert!(timestamp.ends_with('Z'));
}

#[tokio::test]
async fn output_reader_bounds_allocation_and_discards_line_remainder() {
    let mut input = vec![b'x'; MAX_STREAM_LINE_BYTES + 5_000];
    input.extend_from_slice(b"\nnext\n");
    let mut reader = BufReader::with_capacity(256, input.as_slice());
    let mut output = Vec::with_capacity(MAX_STREAM_LINE_BYTES);

    assert_eq!(
        read_bounded_line(&mut reader, &mut output, MAX_STREAM_LINE_BYTES)
            .await
            .unwrap(),
        Some(true)
    );
    assert_eq!(output.len(), MAX_STREAM_LINE_BYTES);
    assert!(output.capacity() <= MAX_STREAM_LINE_BYTES);

    assert_eq!(
        read_bounded_line(&mut reader, &mut output, MAX_STREAM_LINE_BYTES)
            .await
            .unwrap(),
        Some(false)
    );
    assert_eq!(output, b"next\n");
}
