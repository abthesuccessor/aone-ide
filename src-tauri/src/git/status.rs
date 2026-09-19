use std::{fs, path::Path};

use super::{
    inspection::repository_identity,
    process::{git_failure, run_git_inspection},
};
use crate::{
    domain::{GitFileStatus, GitStatusResult},
    error::{AoneError, AoneResult},
};

const STATUS_OUTPUT_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RepositoryLocation {
    Exact,
    Ancestor,
    None,
}

pub(super) async fn repository_location(root: &Path) -> AoneResult<RepositoryLocation> {
    if let Some(location) = classify_dot_git(&root.join(".git"), true)? {
        return Ok(location);
    }
    for parent in root.ancestors().skip(1) {
        if classify_dot_git(&parent.join(".git"), false)?.is_some() {
            return Ok(RepositoryLocation::Ancestor);
        }
    }
    Ok(RepositoryLocation::None)
}

fn classify_dot_git(path: &Path, exact: bool) -> AoneResult<Option<RepositoryLocation>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AoneError::Io(error)),
    };
    if !exact {
        // An ancestor marker is enough to prevent nested initialization. Its
        // contents and target are never opened or followed.
        return Ok(Some(RepositoryLocation::Ancestor));
    }
    if metadata.file_type().is_symlink() {
        return Err(AoneError::InvalidRequest(
            "symlinked Git metadata is not supported".into(),
        ));
    }
    if metadata.is_dir() {
        return Ok(Some(RepositoryLocation::Exact));
    }
    Err(AoneError::InvalidRequest(
        "linked Git worktrees are not supported; open a workspace with an in-place .git directory"
            .into(),
    ))
}

pub(super) async fn load_status(root: &Path) -> AoneResult<GitStatusResult> {
    if repository_location(root).await? != RepositoryLocation::Exact {
        return Ok(GitStatusResult {
            is_repository: false,
            branch: None,
            head: None,
            files: Vec::new(),
        });
    }

    let (branch, head) = repository_identity(root)?;
    let output = run_git_inspection(
        root,
        &strings(&[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=all",
            "--no-renames",
        ]),
        STATUS_OUTPUT_LIMIT,
    )
    .await?;
    if !output.success {
        return Err(git_failure("status", &output));
    }
    if output.stdout_truncated {
        return Err(AoneError::Task(format!(
            "Git status exceeded the {} MiB output limit",
            STATUS_OUTPUT_LIMIT / (1024 * 1024)
        )));
    }
    Ok(GitStatusResult {
        is_repository: true,
        branch,
        head,
        files: parse_porcelain_v1_z(&output.stdout)?,
    })
}

pub(super) fn parse_porcelain_v1_z(bytes: &[u8]) -> AoneResult<Vec<GitFileStatus>> {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0_usize;
    while index < fields.len() {
        let field = fields[index];
        index += 1;
        if field.is_empty() {
            continue;
        }
        if field.len() < 4 || field[2] != b' ' {
            return Err(AoneError::Task("Git returned malformed status data".into()));
        }
        let index_code = field[0] as char;
        let working_code = field[1] as char;
        let path = std::str::from_utf8(&field[3..]).map_err(|_| {
            AoneError::InvalidRequest("non-UTF-8 Git paths are not supported".into())
        })?;
        let rename_or_copy = matches!(index_code, 'R' | 'C') || matches!(working_code, 'R' | 'C');
        if rename_or_copy {
            if index >= fields.len() || fields[index].is_empty() {
                return Err(AoneError::Task("Git returned an incomplete rename".into()));
            }
            index += 1;
        }
        let pair = [index_code, working_code];
        files.push(GitFileStatus {
            relative_path: path.to_owned(),
            index_status: index_code.to_string(),
            working_tree_status: working_code.to_string(),
            staged: !matches!(index_code, ' ' | '?'),
            conflicted: matches!(
                pair,
                ['D', 'D']
                    | ['A', 'U']
                    | ['U', 'D']
                    | ['U', 'A']
                    | ['D', 'U']
                    | ['A', 'A']
                    | ['U', 'U']
            ),
        });
    }
    files.sort_unstable_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
