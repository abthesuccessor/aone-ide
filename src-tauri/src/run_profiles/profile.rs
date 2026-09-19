use std::path::Path;

use crate::{analyzer::stable_id, domain::RunProfile};

pub(super) fn profile(
    workspace_id: &str,
    identity: &str,
    name: &str,
    executable: &str,
    args: Vec<String>,
    cwd_relative: Option<String>,
    kind: &str,
) -> RunProfile {
    RunProfile {
        id: stable_id("runProfile", &[workspace_id, identity, executable]),
        name: name.into(),
        kind: kind.into(),
        executable: executable.into(),
        args,
        cwd_relative,
        required_env: Vec::new(),
        source: identity.split('#').next().unwrap_or(identity).into(),
    }
}

pub(super) fn path_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}
