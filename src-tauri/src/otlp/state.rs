use std::collections::HashSet;

use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::{
    domain::{OtlpReceiverLifecycle, OtlpReceiverSnapshot},
    error::{AoneError, AoneResult},
};

pub(super) const DEFAULT_OTLP_PORT: u16 = 4_318;
pub(super) const MAX_OTLP_SPANS: usize = 8_192;
pub(super) const OTLP_AUTH_HEADER: &str = "x-aone-ingest-token";
pub(super) const OTLP_PROTOCOL: &str = "http/json";
const RECEIVER_LIMITATION: &str = "Loopback OTLP/HTTP JSON traces only. Protobuf, gRPC, metrics, logs, profiles, raw attributes, span events, links, payloads, and persisted trace storage are not accepted. Replay selects retained observations; it does not execute code or move backward.";

pub(crate) struct OtlpReceiverRegistry {
    lifecycle: OtlpReceiverLifecycle,
    active: Option<ActiveReceiver>,
    starting_workspace_id: Option<String>,
    last_error: Option<String>,
}

struct ActiveReceiver {
    id: String,
    workspace_id: String,
    endpoint: String,
    token: Zeroizing<String>,
    shutdown: watch::Sender<bool>,
    started_at: String,
    accepted_spans: usize,
    rejected_spans: usize,
    span_keys: HashSet<String>,
    trace_ids: HashSet<String>,
}

impl Default for OtlpReceiverRegistry {
    fn default() -> Self {
        Self {
            lifecycle: OtlpReceiverLifecycle::Stopped,
            active: None,
            starting_workspace_id: None,
            last_error: None,
        }
    }
}

impl OtlpReceiverRegistry {
    pub(crate) fn reserve_start(&mut self, workspace_id: &str) -> AoneResult<()> {
        if self.lifecycle == OtlpReceiverLifecycle::Starting || self.active.is_some() {
            return Err(AoneError::InvalidRequest(
                "the local OTLP receiver is already starting or running".into(),
            ));
        }
        self.lifecycle = OtlpReceiverLifecycle::Starting;
        self.starting_workspace_id = Some(workspace_id.to_owned());
        self.last_error = None;
        Ok(())
    }

    pub(crate) fn cancel_start(&mut self) {
        if self.lifecycle == OtlpReceiverLifecycle::Starting {
            self.lifecycle = OtlpReceiverLifecycle::Stopped;
            self.starting_workspace_id = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn activate(
        &mut self,
        workspace_id: &str,
        receiver_id: String,
        endpoint: String,
        token: Zeroizing<String>,
        shutdown: watch::Sender<bool>,
        started_at: String,
    ) -> AoneResult<()> {
        if self.lifecycle != OtlpReceiverLifecycle::Starting
            || self.starting_workspace_id.as_deref() != Some(workspace_id)
        {
            return Err(AoneError::InvalidRequest(
                "the OTLP receiver start reservation is no longer current".into(),
            ));
        }
        self.active = Some(ActiveReceiver {
            id: receiver_id,
            workspace_id: workspace_id.to_owned(),
            endpoint,
            token,
            shutdown,
            started_at,
            accepted_spans: 0,
            rejected_spans: 0,
            span_keys: HashSet::new(),
            trace_ids: HashSet::new(),
        });
        self.lifecycle = OtlpReceiverLifecycle::Running;
        self.starting_workspace_id = None;
        self.last_error = None;
        Ok(())
    }

    pub(crate) fn authorize(
        &self,
        receiver_id: &str,
        workspace_id: &str,
        candidate_token: &str,
    ) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        active.id == receiver_id
            && active.workspace_id == workspace_id
            && constant_time_eq(active.token.as_bytes(), candidate_token.as_bytes())
    }

    pub(crate) fn reserve_span(
        &mut self,
        receiver_id: &str,
        trace_id: &str,
        span_id: &str,
    ) -> Result<(), &'static str> {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.id == receiver_id)
        else {
            return Err("receiver session is no longer active");
        };
        if active.span_keys.len() >= MAX_OTLP_SPANS {
            active.rejected_spans = active.rejected_spans.saturating_add(1);
            return Err("receiver span capacity reached");
        }
        let key = format!("{trace_id}:{span_id}");
        if !active.span_keys.insert(key) {
            active.rejected_spans = active.rejected_spans.saturating_add(1);
            return Err("duplicate span identity");
        }
        active.trace_ids.insert(trace_id.to_owned());
        active.accepted_spans = active.accepted_spans.saturating_add(1);
        Ok(())
    }

    pub(crate) fn record_rejected(&mut self, receiver_id: &str, count: usize) {
        if let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.id == receiver_id)
        {
            active.rejected_spans = active.rejected_spans.saturating_add(count);
        }
    }

    pub(crate) fn forget_trace(&mut self, trace_id: &str) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        active.trace_ids.remove(trace_id);
        let prefix = format!("{trace_id}:");
        active.span_keys.retain(|key| !key.starts_with(&prefix));
    }

    pub(crate) fn stop(&mut self) -> bool {
        self.starting_workspace_id = None;
        self.lifecycle = OtlpReceiverLifecycle::Stopped;
        self.last_error = None;
        self.active.take().is_some_and(|active| {
            let _ = active.shutdown.send(true);
            true
        })
    }

    pub(crate) fn fail(&mut self, receiver_id: &str, message: String) {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.id != receiver_id)
        {
            return;
        }
        self.active = None;
        self.starting_workspace_id = None;
        self.lifecycle = OtlpReceiverLifecycle::Failed;
        self.last_error = Some(message);
    }

    pub(crate) fn active_for_workspace(&self, workspace_id: &str) -> bool {
        self.lifecycle == OtlpReceiverLifecycle::Running
            && self
                .active
                .as_ref()
                .is_some_and(|active| active.workspace_id == workspace_id)
    }

    pub(crate) fn snapshot(&self) -> OtlpReceiverSnapshot {
        OtlpReceiverSnapshot {
            status: self.lifecycle,
            endpoint: self.active.as_ref().map(|active| active.endpoint.clone()),
            protocol: OTLP_PROTOCOL.into(),
            auth_header_name: OTLP_AUTH_HEADER.into(),
            auth_header_value: self.active.as_ref().map(|active| active.token.to_string()),
            accepted_spans: self
                .active
                .as_ref()
                .map_or(0, |active| active.accepted_spans),
            rejected_spans: self
                .active
                .as_ref()
                .map_or(0, |active| active.rejected_spans),
            trace_count: self
                .active
                .as_ref()
                .map_or(0, |active| active.trace_ids.len()),
            started_at: self.active.as_ref().map(|active| active.started_at.clone()),
            error: self.last_error.clone(),
            limitation: RECEIVER_LIMITATION.into(),
        }
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_requires_exact_bytes() {
        assert!(constant_time_eq(b"token", b"token"));
        assert!(!constant_time_eq(b"token", b"Token"));
        assert!(!constant_time_eq(b"token", b"token-longer"));
    }

    #[test]
    fn receiver_lifecycle_invalidates_token_and_duplicate_span_identity() {
        let mut registry = OtlpReceiverRegistry::default();
        registry.reserve_start("workspace-a").unwrap();
        let (shutdown, mut shutdown_receiver) = watch::channel(false);
        registry
            .activate(
                "workspace-a",
                "receiver-a".into(),
                "http://127.0.0.1:4318/v1/traces".into(),
                Zeroizing::new("ephemeral-token".into()),
                shutdown,
                "2026-08-20T00:00:00.000Z".into(),
            )
            .unwrap();
        assert!(registry.authorize("receiver-a", "workspace-a", "ephemeral-token"));
        assert!(!registry.authorize("receiver-a", "workspace-b", "ephemeral-token"));
        assert!(
            registry
                .reserve_span("receiver-a", "trace-a", "span-a")
                .is_ok()
        );
        assert!(
            registry
                .reserve_span("receiver-a", "trace-a", "span-a")
                .is_err()
        );
        assert_eq!(registry.snapshot().accepted_spans, 1);
        assert_eq!(registry.snapshot().rejected_spans, 1);

        assert!(registry.stop());
        assert!(*shutdown_receiver.borrow_and_update());
        assert!(!registry.authorize("receiver-a", "workspace-a", "ephemeral-token"));
        assert!(registry.snapshot().auth_header_value.is_none());
    }
}
