use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;

use crate::{
    domain::ProjectEnvironmentReport,
    error::{AoneError, AoneResult},
};

pub struct ProjectEnvironmentState {
    inspection_in_flight: AtomicBool,
    latest_report: RwLock<Option<ProjectEnvironmentReport>>,
}

impl ProjectEnvironmentState {
    pub fn new() -> Self {
        Self {
            inspection_in_flight: AtomicBool::new(false),
            latest_report: RwLock::new(None),
        }
    }

    pub(super) fn reserve_inspection(&self) -> AoneResult<InspectionReservation<'_>> {
        self.inspection_in_flight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                AoneError::InvalidRequest(
                    "a project environment inspection is already in progress".into(),
                )
            })?;
        Ok(InspectionReservation {
            in_flight: &self.inspection_in_flight,
        })
    }

    pub(super) fn store_report(&self, report: ProjectEnvironmentReport) {
        *self.latest_report.write() = Some(report);
    }

    /// Returns the retained report only when it belongs to the requested
    /// workspace. A report retained from a previously open workspace is
    /// deliberately treated as unavailable rather than exposed to the new
    /// workspace.
    pub(super) fn report_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Option<ProjectEnvironmentReport> {
        self.latest_report
            .read()
            .as_ref()
            .filter(|report| report.workspace_id == workspace_id)
            .cloned()
    }

    pub fn latest_report(
        &self,
        workspace_id: &str,
        report_id: &str,
    ) -> AoneResult<ProjectEnvironmentReport> {
        let report = self.latest_report.read();
        let report = report.as_ref().ok_or_else(|| {
            AoneError::InvalidRequest("no project environment report is available".into())
        })?;
        if report.workspace_id != workspace_id || report.report_id != report_id {
            return Err(AoneError::InvalidRequest(
                "project environment report is stale or belongs to another workspace".into(),
            ));
        }
        Ok(report.clone())
    }
}

impl Default for ProjectEnvironmentState {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) struct InspectionReservation<'a> {
    in_flight: &'a AtomicBool,
}

impl Drop for InspectionReservation<'_> {
    fn drop(&mut self) {
        self.in_flight.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(workspace_id: &str, report_id: &str) -> ProjectEnvironmentReport {
        ProjectEnvironmentReport {
            report_id: report_id.into(),
            workspace_id: workspace_id.into(),
            inspected_at: "2026-08-17T00:00:00.000Z".into(),
            version_probe_approved: false,
            stacks: Vec::new(),
            tools: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    #[test]
    fn state_is_single_flight_and_report_ids_are_workspace_bound() {
        let state = ProjectEnvironmentState::new();
        assert!(state.report_for_workspace("workspace:one").is_none());
        let reservation = state.reserve_inspection().unwrap();
        assert!(state.reserve_inspection().is_err());
        drop(reservation);
        assert!(state.reserve_inspection().is_ok());

        state.store_report(report("workspace:one", "report:one"));
        assert_eq!(
            state
                .report_for_workspace("workspace:one")
                .map(|report| report.report_id),
            Some("report:one".into())
        );
        assert!(state.report_for_workspace("workspace:two").is_none());
        assert!(state.latest_report("workspace:one", "report:one").is_ok());
        assert!(state.latest_report("workspace:two", "report:one").is_err());
        assert!(state.latest_report("workspace:one", "report:old").is_err());
    }
}
