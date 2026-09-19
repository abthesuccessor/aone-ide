use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::process::Command;
use uuid::Uuid;

use crate::error::{AoneError, AoneResult};

use super::environment::{isolate_configuration, scrub_git_environment};

const MAX_CONTROL_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_INDEX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SHARED_INDEXES: usize = 16;
const EMPTY_TREE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct InspectionSnapshot {
    root: PathBuf,
    git_dir: PathBuf,
    objects: PathBuf,
    alternate_objects: PathBuf,
    empty_tree: String,
}

impl InspectionSnapshot {
    pub(super) async fn capture(workspace_root: &Path, git: &Path) -> AoneResult<Self> {
        // Inspection isolates attribute lookup with `--attr-source`. Confirm the
        // fixed Git can honour that before inspecting anything, so an older
        // build is reported instead of quietly inspecting with the repository's
        // own attribute drivers in play.
        super::capability::require_attribute_isolation(git).await?;
        let actual_git_dir = resolve_git_dir(workspace_root)?;
        let common_dir = resolve_common_dir(&actual_git_dir)?;
        let objects_path = common_dir.join("objects");
        let objects_metadata = fs::symlink_metadata(&objects_path)
            .map_err(|_| invalid("Git object directory is unavailable"))?;
        if !objects_metadata.is_dir() || objects_metadata.file_type().is_symlink() {
            return Err(invalid(
                "symlinked Git object directories are not supported",
            ));
        }
        reject_object_alternates(&objects_path)?;
        let alternate_objects = objects_path
            .canonicalize()
            .map_err(|_| invalid("Git object directory is unavailable"))?;
        if !alternate_objects.starts_with(&common_dir)
            || alternate_objects
                .as_os_str()
                .to_string_lossy()
                .contains(':')
        {
            return Err(invalid("Git object directory is not safe for inspection"));
        }

        let root = create_private_temp_root()?;
        let git_dir = root.join("metadata");
        let objects = git_dir.join("objects");
        create_private_dir(&git_dir)?;
        create_private_dir(&objects)?;
        create_private_dir(&git_dir.join("refs"))?;
        create_private_dir(&git_dir.join("refs/heads"))?;

        let result = async {
            let object_format = object_format(&common_dir)?;
            write_config(&git_dir, object_format)?;
            write_head(&git_dir, resolve_head_oid(&actual_git_dir, &common_dir)?)?;
            copy_index_files(&actual_git_dir, &git_dir)?;
            let empty_tree = create_empty_tree(git, &git_dir, &objects).await?;
            Ok(Self {
                root: root.clone(),
                git_dir,
                objects,
                alternate_objects,
                empty_tree,
            })
        }
        .await;
        if result.is_err() {
            let _ = fs::remove_dir_all(&root);
        }
        result
    }

    pub(super) fn add_global_arguments(&self, command: &mut Command, workspace_root: &Path) {
        command
            .arg(format!("--git-dir={}", self.git_dir.display()))
            .arg(format!("--work-tree={}", workspace_root.display()))
            .arg(format!("--attr-source={}", self.empty_tree));
    }

    pub(super) fn isolate_environment(&self, command: &mut Command) {
        command
            .env("GIT_ATTR_SOURCE", &self.empty_tree)
            .env("GIT_OBJECT_DIRECTORY", &self.objects)
            .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &self.alternate_objects);
        isolate_configuration(command);
    }
}

fn reject_object_alternates(objects: &Path) -> AoneResult<()> {
    for name in ["alternates", "http-alternates"] {
        let path = objects.join("info").join(name);
        match fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(AoneError::Io(error)),
            Ok(_) => {
                return Err(invalid(
                    "alternate Git object databases are not supported for inspection",
                ));
            }
        }
    }
    Ok(())
}

impl Drop for InspectionSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn resolve_git_dir(workspace_root: &Path) -> AoneResult<PathBuf> {
    let dot_git = workspace_root.join(".git");
    let metadata = fs::symlink_metadata(&dot_git)
        .map_err(|_| invalid("the workspace does not contain Git metadata"))?;
    if metadata.file_type().is_symlink() {
        return Err(invalid("symlinked Git metadata is not supported"));
    }
    if metadata.is_dir() {
        return dot_git
            .canonicalize()
            .map_err(|_| invalid("Git metadata directory is unreadable"));
    }
    Err(invalid(
        "linked Git worktrees are not supported; open a workspace with an in-place .git directory",
    ))
}

fn resolve_common_dir(git_dir: &Path) -> AoneResult<PathBuf> {
    let path = git_dir.join("commondir");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(git_dir.to_owned()),
        Err(error) => Err(AoneError::Io(error)),
        Ok(_) => Err(invalid(
            "linked Git common metadata is not supported for read-only inspection",
        )),
    }
}

fn object_format(common_dir: &Path) -> AoneResult<&'static str> {
    let config = common_dir.join("config");
    if !config.exists() {
        return Ok("sha1");
    }
    let text = read_utf8_file(&config, MAX_CONTROL_FILE_BYTES)?;
    let mut extensions = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            extensions = line.eq_ignore_ascii_case("[extensions]");
            continue;
        }
        if extensions {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim().eq_ignore_ascii_case("objectformat") {
                return match value.trim().to_ascii_lowercase().as_str() {
                    "sha1" => Ok("sha1"),
                    "sha256" => Ok("sha256"),
                    _ => Err(invalid("unsupported Git object format")),
                };
            }
        }
    }
    Ok("sha1")
}

fn resolve_head_oid(git_dir: &Path, common_dir: &Path) -> AoneResult<Option<String>> {
    let head = read_utf8_file(&git_dir.join("HEAD"), 4 * 1024)?;
    let head = head.trim();
    if valid_oid(head) {
        return Ok(Some(head.to_ascii_lowercase()));
    }
    let reference = head
        .strip_prefix("ref:")
        .map(str::trim)
        .ok_or_else(|| invalid("malformed Git HEAD"))?;
    validate_reference(reference)?;
    for base in [git_dir, common_dir] {
        if let Some(oid) = read_utf8_within_optional(base, Path::new(reference), 4 * 1024)? {
            let oid = oid.trim();
            return valid_oid(oid)
                .then(|| Some(oid.to_ascii_lowercase()))
                .ok_or_else(|| invalid("malformed Git reference"));
        }
    }
    resolve_packed_reference(common_dir, reference)
}

fn read_utf8_within_optional(
    base: &Path,
    relative: &Path,
    limit: u64,
) -> AoneResult<Option<String>> {
    let mut candidate = base.to_path_buf();
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(name) = component else {
            return Err(invalid("unsafe Git metadata path"));
        };
        candidate.push(name);
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(AoneError::Io(error)),
        };
        if metadata.file_type().is_symlink() {
            return Err(invalid("symlinked Git reference metadata is not supported"));
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            return Err(invalid("Git reference parent is not a directory"));
        }
    }
    let Some(file) = open_bounded_optional(&candidate, limit)? else {
        return Ok(None);
    };
    let mut bytes = Vec::with_capacity(file.metadata()?.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Git metadata changed beyond its size limit"));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| invalid("non-UTF-8 Git metadata is not supported"))
}

pub(super) fn repository_identity(
    workspace_root: &Path,
) -> AoneResult<(Option<String>, Option<String>)> {
    let git_dir = resolve_git_dir(workspace_root)?;
    let common_dir = resolve_common_dir(&git_dir)?;
    let head = read_utf8_file(&git_dir.join("HEAD"), 4 * 1024)?;
    let head = head.trim();
    let branch = if let Some(reference) = head.strip_prefix("ref:").map(str::trim) {
        validate_reference(reference)?;
        Some(
            reference
                .strip_prefix("refs/heads/")
                .unwrap_or(reference)
                .to_owned(),
        )
    } else {
        None
    };
    let oid =
        resolve_head_oid(&git_dir, &common_dir)?.map(|value| value.chars().take(12).collect());
    Ok((branch, oid))
}

fn resolve_packed_reference(common_dir: &Path, reference: &str) -> AoneResult<Option<String>> {
    let path = common_dir.join("packed-refs");
    if !path.exists() {
        return Ok(None);
    }
    let text = read_utf8_file(&path, MAX_CONTROL_FILE_BYTES)?;
    for line in text.lines() {
        let Some((oid, name)) = line.split_once(' ') else {
            continue;
        };
        if name == reference {
            return valid_oid(oid)
                .then(|| oid.to_ascii_lowercase())
                .map(Some)
                .ok_or_else(|| invalid("malformed packed Git reference"));
        }
    }
    Ok(None)
}

fn validate_reference(reference: &str) -> AoneResult<()> {
    if !reference.starts_with("refs/")
        || reference.split('/').any(|part| {
            part.is_empty() || matches!(part, "." | "..") || part.chars().any(char::is_control)
        })
    {
        return Err(invalid("unsafe Git reference name"));
    }
    Ok(())
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn copy_index_files(actual_git_dir: &Path, snapshot_git_dir: &Path) -> AoneResult<()> {
    copy_optional_bounded(
        &actual_git_dir.join("index"),
        &snapshot_git_dir.join("index"),
        MAX_INDEX_BYTES,
    )?;
    let mut shared = 0_usize;
    for entry in fs::read_dir(actual_git_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("sharedindex.") {
            continue;
        }
        shared += 1;
        if shared > MAX_SHARED_INDEXES {
            return Err(invalid("too many shared Git indexes"));
        }
        copy_optional_bounded(
            &entry.path(),
            &snapshot_git_dir.join(&name),
            MAX_INDEX_BYTES,
        )?;
    }
    Ok(())
}

fn copy_optional_bounded(source: &Path, target: &Path, limit: u64) -> AoneResult<()> {
    let Some(source) = open_bounded_optional(source, limit)? else {
        return Ok(());
    };
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    let copied = std::io::copy(&mut source.take(limit + 1), &mut target)?;
    if copied > limit {
        return Err(invalid("Git index changed beyond its size limit"));
    }
    target.sync_all()?;
    Ok(())
}

fn write_config(git_dir: &Path, format: &str) -> AoneResult<()> {
    let version = if format == "sha256" { 1 } else { 0 };
    let extension = if format == "sha256" {
        "[extensions]\n\tobjectFormat = sha256\n"
    } else {
        ""
    };
    write_private(
        &git_dir.join("config"),
        format!("[core]\n\trepositoryFormatVersion = {version}\n\tbare = false\n{extension}")
            .as_bytes(),
    )
}

fn write_head(git_dir: &Path, oid: Option<String>) -> AoneResult<()> {
    let value = oid.map_or_else(
        || "ref: refs/heads/aone-unborn\n".to_owned(),
        |value| format!("{value}\n"),
    );
    write_private(&git_dir.join("HEAD"), value.as_bytes())
}

async fn create_empty_tree(git: &Path, git_dir: &Path, objects: &Path) -> AoneResult<String> {
    let mut command = Command::new(git);
    command
        .arg("--no-pager")
        .arg("--no-replace-objects")
        .arg(format!("--git-dir={}", git_dir.display()))
        .args(["hash-object", "-t", "tree", "-w", "--stdin"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    scrub_git_environment(&mut command);
    isolate_configuration(&mut command);
    command.env("GIT_NO_LAZY_FETCH", "1");
    command.env("GIT_OBJECT_DIRECTORY", objects);
    let output = tokio::time::timeout(EMPTY_TREE_TIMEOUT, command.output())
        .await
        .map_err(|_| invalid("safe Git attribute snapshot timed out"))??;
    let oid = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid("safe Git attribute snapshot was invalid"))?
        .trim();
    if !output.status.success() || !valid_oid(oid) {
        return Err(invalid("could not create safe Git attribute snapshot"));
    }
    Ok(oid.to_ascii_lowercase())
}

fn create_private_temp_root() -> AoneResult<PathBuf> {
    let base = std::env::temp_dir();
    for _ in 0..8 {
        let candidate = base.join(format!("aone-git-inspection-{}", Uuid::now_v7()));
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(AoneError::Io(error)),
        }
    }
    Err(AoneError::Task(
        "could not allocate private Git inspection metadata".into(),
    ))
}

fn create_private_dir(path: &Path) -> AoneResult<()> {
    let mut builder = DirBuilder::new();
    builder.mode(0o700).recursive(false).create(path)?;
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> AoneResult<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_utf8_file(path: &Path, limit: u64) -> AoneResult<String> {
    let file = open_bounded_optional(path, limit)?
        .ok_or_else(|| invalid("required Git metadata is unreadable"))?;
    let mut bytes = Vec::with_capacity(file.metadata()?.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Git metadata changed beyond its size limit"));
    }
    String::from_utf8(bytes).map_err(|_| invalid("non-UTF-8 Git metadata is not supported"))
}

fn open_bounded_optional(path: &Path, limit: u64) -> AoneResult<Option<File>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(AoneError::Io(error)),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(invalid("Git metadata is not a bounded regular file"));
    }
    Ok(Some(file))
}

fn invalid(message: &str) -> AoneError {
    AoneError::InvalidRequest(message.into())
}
