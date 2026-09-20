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

/// Upper bound on names attached to one profile. `collect_project_hints`
/// already caps detection; this keeps the profile bounded independently of it.
pub(super) const MAX_REQUIRED_ENV: usize = 32;

/// Declares the environment-variable names a discovered profile needs.
///
/// Discovery itself reads no configuration, so `profile` starts every profile
/// with an empty list. The caller joins in the names the project's own config
/// already references, which is what makes the pre-flight check in
/// `ensure_required_env` — and the renderer's missing-variable warning — able to
/// report anything at all. Names only: a value is never carried here.
pub(crate) fn attach_required_env(profiles: &mut [RunProfile], names: &[String]) {
    if names.is_empty() {
        return;
    }
    let mut bounded = names
        .iter()
        .filter(|name| is_environment_name(name))
        .cloned()
        .collect::<Vec<_>>();
    bounded.sort_unstable();
    bounded.dedup();
    bounded.truncate(MAX_REQUIRED_ENV);
    for profile in profiles {
        profile.required_env = bounded.clone();
    }
}

/// Accepts only the conventional shell-portable spelling. A detected token that
/// is not a plausible variable name would otherwise become a requirement the
/// user can never satisfy.
fn is_environment_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}
