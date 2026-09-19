use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, TrySendError},
    },
    thread,
    time::Instant,
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter, Manager};

use super::{
    batching::{MAX_WATCH_BATCH_PATHS, MAX_WATCH_QUEUED_EVENTS, WatchWork, collect_watch_work},
    paths::{lexical_relative, normalize_event_path, relative_components},
};
use crate::{
    ai::SecretState,
    domain::{GraphQuery, WorkspaceChanged},
    error::{AoneError, AoneResult},
    scanner::{
        MAX_WORKSPACE_SCAN_DURATION, analyze_file_with_deadline, is_discoverable_file,
        is_hard_denied, relative_path, scan_workspace,
    },
    state::{AppState, WorkspaceContext},
};

pub struct WorkspaceWatcher {
    watcher: Option<RecommendedWatcher>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Drop for WorkspaceWatcher {
    fn drop(&mut self) {
        self.watcher.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start_workspace_watcher(
    app: AppHandle,
    context: WorkspaceContext,
) -> AoneResult<WorkspaceWatcher> {
    let (sender, receiver) = mpsc::sync_channel::<Vec<PathBuf>>(MAX_WATCH_QUEUED_EVENTS);
    let overflowed = Arc::new(AtomicBool::new(false));
    let callback_overflowed = Arc::clone(&overflowed);
    let mut watcher = notify::recommended_watcher(move |event: Result<Event, notify::Error>| {
        if let Ok(event) = event
            && !matches!(event.kind, EventKind::Access(_))
        {
            if event.paths.len() > MAX_WATCH_BATCH_PATHS {
                callback_overflowed.store(true, Ordering::Release);
                let _ = sender.try_send(Vec::new());
                return;
            }
            if let Err(TrySendError::Full(_)) = sender.try_send(event.paths) {
                callback_overflowed.store(true, Ordering::Release);
            }
        }
    })?;
    watcher.watch(&context.root, RecursiveMode::Recursive)?;

    let worker = thread::Builder::new()
        .name("aone-workspace-watcher".into())
        .spawn(move || {
            while let Ok(first) = receiver.recv() {
                let work = collect_watch_work(first, &receiver, &overflowed);
                let result = match work {
                    WatchWork::Delta(paths) => {
                        apply_delta(&context, paths).or_else(|_| apply_full_rescan(&context, "*"))
                    }
                    WatchWork::FullRescan => apply_full_rescan(&context, "*"),
                };
                if let Ok(changed) = result {
                    if changed.is_empty() {
                        continue;
                    }
                    let state = app.state::<AppState>();
                    if state
                        .workspace_if_open()
                        .is_some_and(|current| current.id == context.id)
                    {
                        let _ = state.refresh_summary(&context.id);
                        if let Ok(snapshot) = context.store.lock().query_graph(&GraphQuery {
                            limit: Some(500),
                            depth: Some(1),
                            ..GraphQuery::default()
                        }) {
                            let secrets = app.state::<SecretState>();
                            let _ = state.while_workspace_current(&context.id, || {
                                secrets.replace_graph_snapshot(&context.id, &snapshot);
                                Ok(())
                            });
                        }
                        let _ = app.emit(
                            "aone-workspace-changed",
                            WorkspaceChanged { paths: changed },
                        );
                    }
                }
            }
        })
        .map_err(AoneError::Io)?;

    Ok(WorkspaceWatcher {
        watcher: Some(watcher),
        worker: Some(worker),
    })
}

pub(super) fn apply_full_rescan(
    context: &WorkspaceContext,
    changed_marker: &str,
) -> AoneResult<Vec<String>> {
    let _reservation = context.wait_for_analysis();
    let result = scan_workspace(&context.root, &context.id, |_| {})?;
    context.store.lock().replace_all(&result.files)?;
    Ok(vec![changed_marker.into()])
}

pub(super) fn apply_delta(
    context: &WorkspaceContext,
    paths: Vec<PathBuf>,
) -> AoneResult<Vec<String>> {
    if paths.len() > MAX_WATCH_BATCH_PATHS {
        return apply_full_rescan(context, "*");
    }
    let _reservation = context.wait_for_analysis();
    let should_rebuild_ignore_set = paths.iter().any(|path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| matches!(name, ".gitignore" | ".ignore"))
    });
    if should_rebuild_ignore_set {
        let result = scan_workspace(&context.root, &context.id, |_| {})?;
        context.store.lock().replace_all(&result.files)?;
        return Ok(vec![".gitignore".into()]);
    }

    let mut changed = BTreeSet::new();
    let deadline = Instant::now()
        .checked_add(MAX_WORKSPACE_SCAN_DURATION)
        .ok_or_else(|| {
            AoneError::InvalidRequest(
                "watcher analysis duration is outside the supported range".into(),
            )
        })?;
    for path in paths {
        enforce_watcher_deadline(deadline)?;
        let path = normalize_event_path(&context.root, &path);
        let Some(relative) = lexical_relative(&context.root, &path) else {
            continue;
        };
        if is_hard_denied(&relative) {
            continue;
        }
        let relative_string = relative_components(&relative)?;

        if !path.exists() {
            let mut store = context.store.lock();
            store.remove_file(&relative_string)?;
            store.remove_files_under(&relative_string)?;
            changed.insert(relative_string);
            continue;
        }
        if path.is_dir() {
            // Native watchers emit child paths for created files. Keeping the directory event
            // itself out of the graph avoids an unnecessary repository-wide rescan.
            continue;
        }
        if !is_discoverable_file(&context.root, &path) {
            context.store.lock().remove_file(&relative_string)?;
            changed.insert(relative_string);
            continue;
        }
        match analyze_file_with_deadline(&context.root, &context.id, &path, Some(deadline))? {
            Some(indexed) => context.store.lock().replace_file(&indexed)?,
            None => context.store.lock().remove_file(&relative_string)?,
        }
        changed.insert(relative_path(&context.root, &path.canonicalize()?)?);
        enforce_watcher_deadline(deadline)?;
    }
    Ok(changed.into_iter().collect())
}

pub(super) fn enforce_watcher_deadline(deadline: Instant) -> AoneResult<()> {
    if Instant::now() >= deadline {
        return Err(AoneError::InvalidRequest(
            "watcher analysis exceeded its 120-second wall-clock budget".into(),
        ));
    }
    Ok(())
}
