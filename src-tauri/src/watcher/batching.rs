use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

const WATCH_DEBOUNCE: Duration = Duration::from_millis(250);
const MAX_WATCH_BATCH_WINDOW: Duration = Duration::from_secs(2);
pub(super) const MAX_WATCH_BATCH_PATHS: usize = 2_048;
pub(super) const MAX_WATCH_QUEUED_EVENTS: usize = 64;

pub(super) enum WatchWork {
    Delta(Vec<PathBuf>),
    FullRescan,
}

pub(super) fn collect_watch_work(
    first: Vec<PathBuf>,
    receiver: &Receiver<Vec<PathBuf>>,
    overflowed: &AtomicBool,
) -> WatchWork {
    let started = Instant::now();
    let mut paths = BTreeSet::new();
    let mut full_rescan = overflowed.swap(false, Ordering::AcqRel);
    extend_watch_paths(&mut paths, first, &mut full_rescan);

    while let Some(remaining) = MAX_WATCH_BATCH_WINDOW.checked_sub(started.elapsed()) {
        let timeout = WATCH_DEBOUNCE.min(remaining);
        match receiver.recv_timeout(timeout) {
            Ok(more) => extend_watch_paths(&mut paths, more, &mut full_rescan),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
        }
    }

    full_rescan |= overflowed.swap(false, Ordering::AcqRel);
    if full_rescan {
        WatchWork::FullRescan
    } else {
        WatchWork::Delta(paths.into_iter().collect())
    }
}

fn extend_watch_paths(
    paths: &mut BTreeSet<PathBuf>,
    incoming: Vec<PathBuf>,
    full_rescan: &mut bool,
) {
    if *full_rescan {
        return;
    }
    for path in incoming {
        if paths.len() >= MAX_WATCH_BATCH_PATHS && !paths.contains(&path) {
            paths.clear();
            *full_rescan = true;
            return;
        }
        paths.insert(path);
    }
}
