use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::{Component, Path, PathBuf},
};

use directories::UserDirs;

use super::{
    catalog::TOOL_CATALOG,
    identity::{ExecutableIdentity, canonical_path, capture_executable_identity},
};
use crate::error::AoneResult;

const MAX_INHERITED_PATH_DIRECTORIES: usize = 64;
const MAX_SEARCH_DIRECTORIES: usize = 128;
const MAX_CANDIDATES_PER_TOOL: usize = 4;

const FIXED_SEARCH_DIRECTORIES: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
    "/Library/Developer/CommandLineTools/usr/bin",
    "/Applications/Xcode.app/Contents/Developer/usr/bin",
];

const HOME_TOOL_DIRECTORIES: &[&str] = &[
    ".cargo/bin",
    ".local/bin",
    ".bun/bin",
    ".deno/bin",
    ".volta/bin",
    ".fnm/current/bin",
    ".nvm/current/bin",
    ".nodenv/shims",
    ".pyenv/shims",
    ".rbenv/shims",
    ".asdf/shims",
    ".local/share/mise/shims",
    ".pub-cache/bin",
    ".composer/vendor/bin",
    ".npm-global/bin",
    ".dotnet",
    ".tfenv/bin",
    "go/bin",
    "flutter/bin",
    "development/flutter/bin",
    ".sdkman/candidates/java/current/bin",
    ".sdkman/candidates/maven/current/bin",
    ".sdkman/candidates/gradle/current/bin",
    ".sdkman/candidates/kotlin/current/bin",
    "Library/Python/3.9/bin",
    "Library/Python/3.10/bin",
    "Library/Python/3.11/bin",
    "Library/Python/3.12/bin",
    "Library/Python/3.13/bin",
    "Library/Python/3.14/bin",
];

#[derive(Debug, Clone)]
pub(super) struct ExecutableCandidate {
    pub(super) tool_id: &'static str,
    pub(super) launch_name: &'static str,
    pub(super) launch_path: PathBuf,
    pub(super) identity: ExecutableIdentity,
}

impl ExecutableCandidate {
    pub(super) fn canonical_path(&self) -> &Path {
        canonical_path(&self.identity)
    }
}

pub(super) type DiscoveredTools = BTreeMap<&'static str, Vec<ExecutableCandidate>>;

pub(super) fn discover_tools(workspace_root: &Path) -> AoneResult<DiscoveredTools> {
    let canonical_workspace = workspace_root.canonicalize()?;
    let directories = search_directories(&canonical_workspace);
    let mut discovered = BTreeMap::new();

    for spec in TOOL_CATALOG {
        let mut candidates = Vec::new();
        let mut targets = BTreeSet::new();
        'directory: for directory in &directories {
            for name in spec.names {
                let launch_path = directory.join(name);
                let Ok(identity) = capture_executable_identity(&launch_path) else {
                    continue;
                };
                let target = canonical_path(&identity);
                if !candidate_location_allowed(&canonical_workspace, &launch_path, target)
                    || !targets.insert(target.to_path_buf())
                {
                    continue;
                }
                candidates.push(ExecutableCandidate {
                    tool_id: spec.id,
                    launch_name: name,
                    launch_path,
                    identity,
                });
                if candidates.len() == MAX_CANDIDATES_PER_TOOL {
                    break 'directory;
                }
            }
        }
        discovered.insert(spec.id, candidates);
    }
    Ok(discovered)
}

fn search_directories(workspace_root: &Path) -> Vec<PathBuf> {
    let mut raw_directories = env::var_os("PATH")
        .map(|value| {
            env::split_paths(&value)
                .take(MAX_INHERITED_PATH_DIRECTORIES)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    raw_directories.extend(FIXED_SEARCH_DIRECTORIES.iter().map(PathBuf::from));
    if let Some(user_dirs) = UserDirs::new() {
        raw_directories.extend(home_tool_directories(user_dirs.home_dir()));
    }

    let mut directories = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in raw_directories {
        if directories.len() == MAX_SEARCH_DIRECTORIES {
            break;
        }
        if !is_clean_absolute(&raw) {
            continue;
        }
        let Ok(canonical) = raw.canonicalize() else {
            continue;
        };
        if !canonical.is_dir()
            || canonical.starts_with(workspace_root)
            || !seen.insert(canonical.clone())
        {
            continue;
        }
        directories.push(canonical);
    }
    directories
}

fn home_tool_directories(home: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    HOME_TOOL_DIRECTORIES
        .iter()
        .map(|relative| home.join(relative))
}

fn is_clean_absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
}

fn is_safe_display_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|value| !value.chars().any(char::is_control))
}

fn candidate_location_allowed(workspace: &Path, launch_path: &Path, target: &Path) -> bool {
    !launch_path.starts_with(workspace)
        && !target.starts_with(workspace)
        && is_safe_display_path(launch_path)
        && is_safe_display_path(target)
}

#[cfg(test)]
pub(super) fn clean_absolute_for_test(path: &Path) -> bool {
    is_clean_absolute(path)
}

#[cfg(test)]
pub(super) fn safe_display_path_for_test(path: &Path) -> bool {
    is_safe_display_path(path)
}

#[cfg(test)]
pub(super) fn candidate_location_allowed_for_test(
    workspace: &Path,
    launch_path: &Path,
    target: &Path,
) -> bool {
    candidate_location_allowed(workspace, launch_path, target)
}

#[cfg(test)]
pub(super) fn home_tool_directories_for_test(home: &Path) -> Vec<PathBuf> {
    home_tool_directories(home).collect()
}

#[cfg(test)]
pub(super) const TEST_MAX_CANDIDATES_PER_TOOL: usize = MAX_CANDIDATES_PER_TOOL;
