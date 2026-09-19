use super::{ActiveRun, RuntimeState};

impl RuntimeState {
    pub(in crate::runner) fn mark_stopping(&self, run_id: &str) -> Option<u32> {
        let mut runs = self.runs.lock();
        let run = runs.active.get_mut(run_id)?;
        run.stopping = true;
        Some(run.pid)
    }

    pub(in crate::runner) fn confirm_stopping(&self, run_id: &str, pid: u32) {
        let mut runs = self.runs.lock();
        let Some(run) = runs
            .active
            .get_mut(run_id)
            .filter(|run| run.pid == pid && run.stopping)
        else {
            return;
        };
        run.streams_cancelled = true;
        let _ = run.cancellation.send(true);
        drop(runs);
        self.clear_trace_run(run_id);
        // Signal delivery is not process-exit proof. The debug session reaches
        // `stopped` only when the leader watcher observes and reaps the exit.
    }

    pub(in crate::runner) fn cancel_stopping(&self, run_id: &str, pid: u32) {
        let mut runs = self.runs.lock();
        let should_remove = runs
            .active
            .get_mut(run_id)
            .filter(|run| run.pid == pid)
            .is_some_and(|run| {
                run.stopping = false;
                run.leader_exited
            });
        if should_remove {
            runs.active.remove(run_id);
        }
    }

    #[cfg(test)]
    pub(in crate::runner) fn record_leader_exit(&self, run_id: &str) -> Option<ActiveRun> {
        self.record_leader_exit_with(run_id, |_| ())
            .map(|(run, ())| run)
    }

    pub(in crate::runner) fn record_leader_exit_with<T>(
        &self,
        run_id: &str,
        operation: impl FnOnce(&ActiveRun) -> T,
    ) -> Option<(ActiveRun, T)> {
        let mut runs = self.runs.lock();
        let run = runs.active.get_mut(run_id)?;
        run.leader_exited = true;
        run.streams_cancelled = true;
        let _ = run.cancellation.send(true);
        let snapshot = run.clone();
        self.traces.lock().clear_run(run_id);
        self.debugger
            .lock()
            .process_exited(run_id, snapshot.stopping);
        let result = operation(&snapshot);
        if !snapshot.stopping || snapshot.escalation_complete {
            runs.active.remove(run_id);
        }
        Some((snapshot, result))
    }

    #[cfg(test)]
    pub(in crate::runner) fn record_wait_failure(&self, run_id: &str) -> Option<ActiveRun> {
        self.record_wait_failure_with(run_id, |_| ())
            .map(|(run, ())| run)
    }

    pub(in crate::runner) fn record_wait_failure_with<T>(
        &self,
        run_id: &str,
        operation: impl FnOnce(&ActiveRun) -> T,
    ) -> Option<(ActiveRun, T)> {
        let mut runs = self.runs.lock();
        let run = runs.active.get_mut(run_id)?;
        if run.leader_exited {
            return None;
        }
        run.exit_observation_failed = true;
        let snapshot = run.clone();
        self.debugger.lock().control_failed(
            run_id,
            "could not observe managed child exit; process state is unknown and the debug protocol session is terminal"
                .into(),
        );
        let result = operation(&snapshot);
        if snapshot.stopping && snapshot.escalation_complete {
            runs.active.remove(run_id);
        }
        Some((snapshot, result))
    }

    pub(in crate::runner) fn stopping_pid(&self, run_id: &str) -> Option<u32> {
        self.runs
            .lock()
            .active
            .get(run_id)
            .filter(|run| run.stopping && !run.escalation_complete)
            .map(|run| run.pid)
    }

    pub(in crate::runner) fn complete_stop_escalation(&self, run_id: &str, pid: u32) {
        let mut runs = self.runs.lock();
        let should_remove = runs
            .active
            .get_mut(run_id)
            .filter(|run| run.pid == pid && run.stopping)
            .is_some_and(|run| {
                run.escalation_complete = true;
                run.leader_exited || run.exit_observation_failed
            });
        if should_remove {
            runs.active.remove(run_id);
        }
    }

    #[cfg(test)]
    pub(in crate::runner) fn remove_run(&self, run_id: &str) -> Option<ActiveRun> {
        let removed = self.runs.lock().active.remove(run_id);
        if let Some(run) = removed.as_ref() {
            let _ = run.cancellation.send(true);
        }
        self.clear_trace_run(run_id);
        self.debugger.lock().process_exited(run_id, true);
        removed
    }
}
