use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use tempfile::tempdir;

use super::{AppState, new_workspace_context};
use crate::store::GraphStore;

#[test]
fn database_is_always_below_application_support_root() {
    let app_data = tempdir().unwrap();
    let state = AppState::new(app_data.path().to_path_buf()).unwrap();
    let database = state.database_path("workspace:abc");
    assert!(database.starts_with(app_data.path()));
    assert!(database.ends_with("workspaces/workspace_abc/aone.sqlite"));
}

#[test]
fn workspace_operations_are_single_flight_and_release_on_drop() {
    let app_data = tempdir().unwrap();
    let state = AppState::new(app_data.path().to_path_buf()).unwrap();

    let first = state.try_reserve_workspace_operation().unwrap();
    let error = state.try_reserve_workspace_operation().err().unwrap();
    assert!(error.to_string().contains("already in progress"));

    drop(first);
    assert!(state.try_reserve_workspace_operation().is_ok());
}

#[test]
fn analysis_reservations_fail_fast_or_wait_without_overlap() {
    let workspace = tempdir().unwrap();
    let database = tempdir().unwrap();
    let store = GraphStore::open(&database.path().join("aone.sqlite")).unwrap();
    let context = new_workspace_context(
        "workspace".into(),
        workspace.path().canonicalize().unwrap(),
        store,
    )
    .unwrap();
    let first = context.try_reserve_analysis().unwrap();
    let error = context.try_reserve_analysis().err().unwrap();
    assert!(error.to_string().contains("analysis is already running"));

    let waiting_context = context.clone();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _reservation = waiting_context.wait_for_analysis();
        sender.send(()).unwrap();
    });
    assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
    drop(first);
    receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.join().unwrap();
}

#[test]
fn identified_editor_commit_blocks_workspace_installation() {
    let app_data = tempdir().unwrap();
    let first_root = tempdir().unwrap();
    let second_root = tempdir().unwrap();
    let first_database = tempdir().unwrap();
    let second_database = tempdir().unwrap();
    let first = new_workspace_context(
        "workspace:first".into(),
        first_root.path().canonicalize().unwrap(),
        GraphStore::open(&first_database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();
    let second = new_workspace_context(
        "workspace:second".into(),
        second_root.path().canonicalize().unwrap(),
        GraphStore::open(&second_database.path().join("aone.sqlite")).unwrap(),
    )
    .unwrap();
    let state = Arc::new(AppState::new(app_data.path().to_path_buf()).unwrap());
    state.install_workspace_for_test(first);
    assert!(
        state
            .while_workspace_current("workspace:other", || Ok(()))
            .is_err()
    );

    let (commit_started_tx, commit_started_rx) = mpsc::channel();
    let (release_commit_tx, release_commit_rx) = mpsc::channel();
    let commit_state = Arc::clone(&state);
    let commit = thread::spawn(move || {
        commit_state
            .while_workspace_current("workspace:first", || {
                commit_started_tx.send(()).unwrap();
                release_commit_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    commit_started_rx.recv().unwrap();

    let (installed_tx, installed_rx) = mpsc::channel();
    let install_state = Arc::clone(&state);
    let install = thread::spawn(move || {
        install_state.install_workspace_for_test(second);
        installed_tx.send(()).unwrap();
    });
    assert!(
        installed_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
    release_commit_tx.send(()).unwrap();
    installed_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    commit.join().unwrap();
    install.join().unwrap();
    assert_eq!(state.workspace().unwrap().id, "workspace:second");
}
