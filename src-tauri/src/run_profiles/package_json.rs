use std::{fs::File, io::Read, path::Path};

use serde_json::Value;

use super::profile::profile;
use crate::{domain::RunProfile, error::AoneResult};

const MAX_PACKAGE_MANIFEST_BYTES: u64 = 1024 * 1024;

pub(super) fn package_profiles(
    root: &Path,
    workspace_id: &str,
    manifest: &Path,
    cwd_relative: Option<String>,
    source: String,
) -> AoneResult<Vec<RunProfile>> {
    let mut content = Vec::new();
    File::open(manifest)?
        .take(MAX_PACKAGE_MANIFEST_BYTES + 1)
        .read_to_end(&mut content)?;
    if content.len() as u64 > MAX_PACKAGE_MANIFEST_BYTES {
        return Ok(Vec::new());
    }
    let Ok(content) = std::str::from_utf8(&content) else {
        return Ok(Vec::new());
    };
    let Ok(document) = serde_json::from_str::<Value>(content) else {
        return Ok(Vec::new());
    };
    let Some(scripts) = document.get("scripts").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let directory = manifest.parent().unwrap_or(root);
    let executable = if directory.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if directory.join("yarn.lock").is_file() {
        "yarn"
    } else if directory.join("bun.lock").is_file() || directory.join("bun.lockb").is_file() {
        "bun"
    } else {
        "npm"
    };
    let mut names = scripts.keys().cloned().collect::<Vec<_>>();
    names.sort_by_key(|name| {
        ["dev", "start", "serve", "preview"]
            .iter()
            .position(|preferred| name == preferred)
            .unwrap_or(usize::MAX)
    });
    Ok(names
        .into_iter()
        .take(30)
        .map(|script| {
            profile(
                workspace_id,
                &format!("{source}#{script}"),
                &format!("{executable} run {script}"),
                executable,
                vec!["run".into(), script],
                cwd_relative.clone(),
                "node",
            )
        })
        .collect())
}

#[cfg(test)]
pub(super) const TEST_MAX_PACKAGE_MANIFEST_BYTES: u64 = MAX_PACKAGE_MANIFEST_BYTES;
