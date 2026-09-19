use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc},
    thread,
    time::{Duration, Instant},
};

use tempfile::tempdir;

use super::{
    batching::{MAX_WATCH_BATCH_PATHS, WatchWork, collect_watch_work},
    worker::{apply_delta, enforce_watcher_deadline},
};
use crate::{
    scanner::{canonical_workspace, scan_workspace, workspace_id},
    state::new_workspace_context,
    store::GraphStore,
};

#[test]
fn file_delta_replaces_and_removes_only_the_changed_file() {
    let workspace = tempdir().unwrap();
    let app_data = tempdir().unwrap();
    fs::create_dir(workspace.path().join("src")).unwrap();
    let source_path = workspace.path().join("src/lib.rs");
    fs::write(&source_path, "fn first() {}\n").unwrap();
    let root = canonical_workspace(workspace.path()).unwrap();
    let id = workspace_id(&root);
    let initial = scan_workspace(&root, &id, |_| {}).unwrap();
    let mut store = GraphStore::open(&app_data.path().join("aone.sqlite")).unwrap();
    store.replace_all(&initial.files).unwrap();
    let context = new_workspace_context(id, root, store).unwrap();

    let before_hash = context
        .store
        .lock()
        .get_file("src/lib.rs")
        .unwrap()
        .unwrap()
        .content_hash;
    fs::write(&source_path, "fn second() { changed(); }\n").unwrap();
    let changed = apply_delta(&context, vec![source_path.clone()]).unwrap();
    assert_eq!(changed, vec!["src/lib.rs"]);
    let after_hash = context
        .store
        .lock()
        .get_file("src/lib.rs")
        .unwrap()
        .unwrap()
        .content_hash;
    assert_ne!(before_hash, after_hash);
    assert_eq!(context.store.lock().counts().unwrap().0, 1);

    fs::remove_file(&source_path).unwrap();
    apply_delta(&context, vec![source_path]).unwrap();
    assert_eq!(context.store.lock().counts().unwrap().0, 0);
}

#[test]
fn watcher_delta_waits_for_an_active_explicit_analysis() {
    let workspace = tempdir().unwrap();
    let app_data = tempdir().unwrap();
    let source_path = workspace.path().join("lib.rs");
    fs::write(&source_path, "fn before() {}\n").unwrap();
    let root = canonical_workspace(workspace.path()).unwrap();
    let id = workspace_id(&root);
    let initial = scan_workspace(&root, &id, |_| {}).unwrap();
    let mut store = GraphStore::open(&app_data.path().join("aone.sqlite")).unwrap();
    store.replace_all(&initial.files).unwrap();
    let context = new_workspace_context(id, root, store).unwrap();
    let before_hash = context
        .store
        .lock()
        .get_file("lib.rs")
        .unwrap()
        .unwrap()
        .content_hash;

    let explicit_reservation = context.try_reserve_analysis().unwrap();
    fs::write(&source_path, "fn after() { changed(); }\n").unwrap();
    let worker_context = context.clone();
    let worker_path = source_path.clone();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        sender
            .send(apply_delta(&worker_context, vec![worker_path]))
            .unwrap();
    });

    assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
    assert_eq!(
        context
            .store
            .lock()
            .get_file("lib.rs")
            .unwrap()
            .unwrap()
            .content_hash,
        before_hash
    );
    drop(explicit_reservation);
    receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_ne!(
        context
            .store
            .lock()
            .get_file("lib.rs")
            .unwrap()
            .unwrap()
            .content_hash,
        before_hash
    );
}

#[test]
fn oversized_watcher_batch_becomes_one_full_rescan_marker() {
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(sender);
    let overflowed = AtomicBool::new(false);
    let first = (0..=MAX_WATCH_BATCH_PATHS)
        .map(|index| PathBuf::from(format!("src/{index}.rs")))
        .collect();

    assert!(matches!(
        collect_watch_work(first, &receiver, &overflowed),
        WatchWork::FullRescan
    ));
}

#[test]
fn watcher_batch_deadline_fails_closed() {
    let error = enforce_watcher_deadline(Instant::now()).err().unwrap();
    assert!(error.to_string().contains("120-second wall-clock budget"));
}
