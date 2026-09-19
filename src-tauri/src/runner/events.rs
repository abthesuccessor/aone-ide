use std::{collections::BTreeMap, path::Path};

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use zeroize::Zeroizing;

use super::{
    RuntimeState,
    environment::redact_args,
    preparation::{PreparedRun, display_relative_cwd},
};
use crate::domain::RuntimeEvent;

pub const RUNTIME_EVENT_NAME: &str = "aone-runtime-event";

pub fn publish_event(app: &AppHandle, state: &RuntimeState, event: RuntimeEvent) {
    state.record_event(event.clone());
    // Persistence is intentionally best-effort and local operation must continue
    // if there is no renderer listening.
    let _ = app.emit(RUNTIME_EVENT_NAME, event);
}

pub(super) fn process_started_metadata(
    workspace_root: &Path,
    prepared: &PreparedRun,
    selected_env_names: &[String],
    secret_values: &[Zeroizing<String>],
    pid: u32,
    observe: bool,
    debug: bool,
) -> BTreeMap<String, Value> {
    let mut environment_names = selected_env_names.to_vec();
    environment_names.sort();
    environment_names.dedup();

    BTreeMap::from([
        (
            "args".into(),
            json!(redact_args(&prepared.args, secret_values)),
        ),
        ("authorization".into(), json!("explicitUserAction")),
        (
            "cwd".into(),
            json!(display_relative_cwd(workspace_root, &prepared.cwd)),
        ),
        ("debug".into(), json!(debug)),
        ("environmentNames".into(), json!(environment_names)),
        ("executable".into(), json!(prepared.display_executable)),
        ("observe".into(), json!(observe)),
        ("pid".into(), json!(pid)),
        ("profileId".into(), json!(prepared.profile_id)),
    ])
}
