use std::collections::HashMap;

pub fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }
    format!("{prefix}:{}", &hasher.finalize().to_hex()[..32])
}

pub(super) struct EventIdentity<'a> {
    pub workspace_id: &'a str,
    pub relative_path: &'a str,
    pub language: &'a str,
    pub owner_id: &'a str,
    pub callee: &'a str,
    pub operation: &'a str,
    pub direction: &'a str,
    pub event_name: &'a str,
}

pub(super) fn stable_event_id(
    identity: EventIdentity<'_>,
    occurrences: &mut HashMap<String, usize>,
) -> String {
    let semantic_id = stable_id(
        "event-semantic",
        &[
            identity.workspace_id,
            identity.relative_path,
            identity.language,
            identity.owner_id,
            identity.callee,
            identity.operation,
            identity.direction,
            identity.event_name,
        ],
    );
    let occurrence = occurrences.entry(semantic_id.clone()).or_default();
    let ordinal = occurrence.to_string();
    *occurrence = occurrence.saturating_add(1);
    stable_id("event", &[&semantic_id, &ordinal])
}
