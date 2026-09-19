use std::collections::HashMap;

const MAX_TRACE_SPANS_PER_RUN: usize = 4_096;
const MAX_TRACE_EVENTS_PER_RUN: usize = 8_192;

#[derive(Default)]
pub(super) struct TraceRegistry {
    spans: HashMap<(String, String, String), TraceSpanState>,
    unique_by_run: HashMap<String, usize>,
    accepted_by_run: HashMap<String, usize>,
}

struct TraceSpanState {
    parent: Option<String>,
    ended: bool,
}

impl TraceRegistry {
    pub(super) fn accept(
        &mut self,
        run_id: &str,
        trace_id: &str,
        span_id: &str,
        parent_span_id: Option<&str>,
        phase: &str,
    ) -> Result<(), &'static str> {
        if self.accepted_by_run.get(run_id).copied().unwrap_or(0) >= MAX_TRACE_EVENTS_PER_RUN {
            return Err("trace event capacity reached for this run");
        }
        let key = (run_id.to_owned(), trace_id.to_owned(), span_id.to_owned());
        match phase {
            "start" | "event" => {
                if self.spans.contains_key(&key) {
                    return Err("duplicate trace span identity");
                }
                if let Some(parent) = parent_span_id {
                    let parent_key = (run_id.to_owned(), trace_id.to_owned(), parent.to_owned());
                    if !self.spans.contains_key(&parent_key) {
                        return Err("parent span has not been accepted for this run and trace");
                    }
                }
                if self.unique_by_run.get(run_id).copied().unwrap_or(0) >= MAX_TRACE_SPANS_PER_RUN {
                    return Err("trace span capacity reached for this run");
                }
                self.spans.insert(
                    key,
                    TraceSpanState {
                        parent: parent_span_id.map(str::to_owned),
                        ended: phase == "event",
                    },
                );
                *self.unique_by_run.entry(run_id.to_owned()).or_default() += 1;
            }
            "end" => {
                let Some(span) = self.spans.get_mut(&key) else {
                    return Err("trace end has no accepted start span");
                };
                if span.ended {
                    return Err("trace span already ended");
                }
                if span.parent.as_deref() != parent_span_id {
                    return Err("trace end parent does not match the accepted start span");
                }
                span.ended = true;
            }
            _ => return Err("unsupported trace phase"),
        }
        *self.accepted_by_run.entry(run_id.to_owned()).or_default() += 1;
        Ok(())
    }

    pub(super) fn clear_run(&mut self, run_id: &str) {
        self.spans
            .retain(|(stored_run, _, _), _| stored_run != run_id);
        self.unique_by_run.remove(run_id);
        self.accepted_by_run.remove(run_id);
    }

    pub(super) fn clear(&mut self) {
        self.spans.clear();
        self.unique_by_run.clear();
        self.accepted_by_run.clear();
    }
}
