use std::time::{SystemTime, UNIX_EPOCH};

pub fn system_time_string(value: SystemTime) -> String {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".into())
}
