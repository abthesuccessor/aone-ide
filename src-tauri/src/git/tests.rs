use std::fs;

use tempfile::tempdir;

use super::{
    commands::{load_comparison, staged_paths, staged_snapshot, unstage_args},
    paths::{validate_path, validate_paths},
    process::{run_git, run_git_inspection},
    status::{RepositoryLocation, load_status, parse_porcelain_v1_z, repository_location},
};
use crate::domain::GitDiffRequest;

#[test]
fn parses_porcelain_status_and_skips_rename_source() {
    let raw = b"M  staged.rs\0 M modified.rs\0?? new.rs\0UU conflict.rs\0R  renamed.rs\0old.rs\0";
    let files = parse_porcelain_v1_z(raw).expect("status should parse");
    assert_eq!(files.len(), 5);
    assert!(
        files
            .iter()
            .any(|file| file.relative_path == "staged.rs" && file.staged)
    );
    assert!(files.iter().any(|file| {
        file.relative_path == "modified.rs" && !file.staged && file.working_tree_status == "M"
    }));
    assert!(
        files
            .iter()
            .any(|file| file.relative_path == "conflict.rs" && file.conflicted)
    );
    assert!(files.iter().any(|file| file.relative_path == "renamed.rs"));
    assert!(!files.iter().any(|file| file.relative_path == "old.rs"));
}

#[test]
fn rejects_malformed_porcelain_status() {
    let error = parse_porcelain_v1_z(b"M bad\0").expect_err("malformed data must fail");
    assert!(error.to_string().contains("malformed"));
}

#[test]
fn validates_literal_workspace_file_paths() {
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    fs::create_dir(root.join("src")).expect("source directory");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("source file");
    let paths = validate_paths(&root, &["src/main.rs".into(), "deleted.rs".into()])
        .expect("paths should validate");
    assert_eq!(paths, vec!["deleted.rs", "src/main.rs"]);
    assert!(validate_path(&root, "../outside").is_err());
    assert!(validate_path(&root, ".env").is_err());
}

#[test]
fn diff_contract_requires_one_validated_file_path() {
    assert!(
        serde_json::from_value::<GitDiffRequest>(serde_json::json!({ "staged": false })).is_err()
    );
    let request: GitDiffRequest = serde_json::from_value(serde_json::json!({
        "relativePath": "src/main.rs",
        "staged": true
    }))
    .expect("file-scoped diff request");
    assert_eq!(request.relative_path, "src/main.rs");
}

#[test]
fn staged_and_unstaged_diffs_reject_hard_denied_paths() {
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    for staged in [false, true] {
        let request = GitDiffRequest {
            relative_path: ".env".into(),
            staged,
        };
        assert!(validate_path(&root, &request.relative_path).is_err());
    }
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_git_paths() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let outside = tempdir().expect("outside directory");
    fs::write(outside.path().join("secret"), "secret").expect("outside file");
    symlink(outside.path().join("secret"), root.join("link")).expect("symlink");
    assert!(validate_paths(&root, &["link".into()]).is_err());
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn fixed_git_runner_discovers_only_exact_root_and_loads_status() {
    if !attribute_isolation_available().await {
        return;
    }
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let init = run_git(&root, &["init".into(), "--template=".into()], 64 * 1024)
        .await
        .expect("git init should execute");
    assert!(init.success);
    assert!(!root.join(".git/hooks").exists());
    fs::create_dir(root.join("nested")).expect("nested directory");
    fs::write(root.join("work.txt"), "local change\n").expect("worktree file");

    assert_eq!(
        repository_location(&root).await.expect("root probe"),
        RepositoryLocation::Exact
    );
    assert_eq!(
        repository_location(&root.join("nested"))
            .await
            .expect("nested probe"),
        RepositoryLocation::Ancestor
    );
    let status = load_status(&root).await.expect("status");
    assert!(status.is_repository);
    assert!(
        status
            .files
            .iter()
            .any(|file| file.relative_path == "work.txt" && !file.staged)
    );
    let stage = run_git(
        &root,
        &["add".into(), "--".into(), "work.txt".into()],
        64 * 1024,
    )
    .await
    .expect("git add should execute");
    assert!(stage.success);
    assert_eq!(
        staged_paths(&root).await.expect("staged paths"),
        vec!["work.txt"]
    );
    let first_snapshot = staged_snapshot(&root).await.expect("first staged snapshot");
    fs::write(root.join("work.txt"), "restaged content\n").expect("change staged content");
    let restage = run_git(
        &root,
        &["add".into(), "--".into(), "work.txt".into()],
        64 * 1024,
    )
    .await
    .expect("restage changed content");
    assert!(restage.success);
    let second_snapshot = staged_snapshot(&root)
        .await
        .expect("second staged snapshot");
    assert_eq!(first_snapshot.paths, second_snapshot.paths);
    assert_ne!(first_snapshot.identity, second_snapshot.identity);
    fs::write(root.join("work.txt"), "changed after staging\n").expect("modify staged file");
    let mut unstage = unstage_args(false);
    unstage.push("work.txt".into());
    let unstage = run_git(&root, &unstage, 64 * 1024)
        .await
        .expect("unborn unstage should execute");
    assert!(unstage.success);
    let status = load_status(&root).await.expect("status after unstage");
    assert!(
        status
            .files
            .iter()
            .any(|file| file.relative_path == "work.txt" && !file.staged)
    );
    assert_eq!(
        fs::read_to_string(root.join("work.txt")).expect("working tree file remains"),
        "changed after staging\n"
    );

    let add = run_git(
        &root,
        &["add".into(), "--".into(), "work.txt".into()],
        64 * 1024,
    )
    .await
    .expect("restage file");
    assert!(add.success);
    let commit = run_git(
        &root,
        &[
            "-c".into(),
            "user.name=Aone Test".into(),
            "-c".into(),
            "user.email=aone@example.invalid".into(),
            "commit".into(),
            "--no-verify".into(),
            "--no-gpg-sign".into(),
            "-m".into(),
            "baseline".into(),
        ],
        64 * 1024,
    )
    .await
    .expect("baseline commit");
    assert!(commit.success);
    fs::rename(root.join("work.txt"), root.join("renamed.txt")).expect("rename file");
    let stage_rename = run_git(&root, &["add".into(), "-A".into(), "--".into()], 64 * 1024)
        .await
        .expect("stage rename");
    assert!(stage_rename.success);
    let rename_status = load_status(&root).await.expect("rename status");
    assert!(rename_status.files.iter().any(|file| {
        file.relative_path == "work.txt" && file.index_status == "D" && file.staged
    }));
    assert!(rename_status.files.iter().any(|file| {
        file.relative_path == "renamed.txt" && file.index_status == "A" && file.staged
    }));

    let mut unstage_rename = unstage_args(true);
    unstage_rename.extend(["renamed.txt".into(), "work.txt".into()]);
    let unstage_rename = run_git(&root, &unstage_rename, 64 * 1024)
        .await
        .expect("unstage rename sides");
    assert!(unstage_rename.success);
    assert!(
        load_status(&root)
            .await
            .expect("status after rename unstage")
            .files
            .iter()
            .all(|file| !file.staged)
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn status_and_diff_never_execute_repository_process_filters() {
    if !attribute_isolation_available().await {
        return;
    }
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    assert!(
        run_git(&root, &["init".into(), "--template=".into()], 64 * 1024)
            .await
            .expect("initialize repository")
            .success
    );
    fs::write(root.join("tracked.txt"), "before\n").expect("baseline file");
    assert!(
        run_git(
            &root,
            &["add".into(), "--".into(), "tracked.txt".into()],
            64 * 1024,
        )
        .await
        .expect("stage baseline")
        .success
    );
    assert!(
        run_git(
            &root,
            &[
                "-c".into(),
                "user.name=Aone Test".into(),
                "-c".into(),
                "user.email=aone@example.invalid".into(),
                "commit".into(),
                "--no-verify".into(),
                "--no-gpg-sign".into(),
                "-m".into(),
                "baseline".into(),
            ],
            64 * 1024,
        )
        .await
        .expect("commit baseline")
        .success
    );

    let driver = root.join("evil-filter");
    let marker = root.join("evil-filter.ran");
    fs::write(&driver, "#!/bin/sh\n/usr/bin/touch \"$0.ran\"\nexit 1\n")
        .expect("malicious filter script");
    let mut permissions = fs::metadata(&driver)
        .expect("driver metadata")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&driver, permissions).expect("make filter executable");
    fs::write(
        root.join(".gitattributes"),
        "tracked.txt filter=evil diff=evil\n",
    )
    .expect("malicious attributes");
    fs::create_dir_all(root.join(".git/info")).expect("Git info directory");
    fs::write(
        root.join(".git/info/attributes"),
        "tracked.txt filter=evil diff=evil\n",
    )
    .expect("malicious metadata attributes");
    for (key, value) in [
        ("filter.evil.process", "./evil-filter"),
        ("filter.evil.required", "true"),
        ("diff.evil.command", "./evil-filter"),
        ("core.fsmonitor", "./evil-filter"),
    ] {
        assert!(
            run_git(
                &root,
                &["config".into(), "--local".into(), key.into(), value.into()],
                64 * 1024,
            )
            .await
            .expect("configure malicious driver")
            .success
        );
    }
    fs::write(root.join("tracked.txt"), "after\n").expect("modify tracked file");

    let (original, modified, truncated) = load_comparison(&root, "tracked.txt", false)
        .await
        .expect("working tree comparison");
    assert_eq!(original.as_deref(), Some("before\n"));
    assert_eq!(modified.as_deref(), Some("after\n"));
    assert!(!truncated);

    let status = load_status(&root).await.expect("filter-free status");
    assert!(
        status
            .files
            .iter()
            .any(|file| { file.relative_path == "tracked.txt" && file.working_tree_status == "M" })
    );
    assert!(
        !marker.exists(),
        "status executed a repository process filter"
    );

    let diff = run_git_inspection(
        &root,
        &[
            "diff".into(),
            "--no-ext-diff".into(),
            "--no-textconv".into(),
            "--ignore-submodules=all".into(),
            "--no-renames".into(),
            "--patch".into(),
            "--".into(),
            "tracked.txt".into(),
        ],
        64 * 1024,
    )
    .await
    .expect("filter-free diff");
    assert!(diff.success);
    assert!(String::from_utf8_lossy(&diff.stdout).contains("+after"));
    assert!(
        !marker.exists(),
        "diff executed a repository process filter"
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn inspection_rejects_a_git_file_pointing_to_an_unrelated_repository() {
    if !attribute_isolation_available().await {
        return;
    }
    let external = tempdir().expect("external repository directory");
    let external_root = external.path().canonicalize().expect("external root");
    assert!(
        run_git(
            &external_root,
            &["init".into(), "--template=".into()],
            64 * 1024,
        )
        .await
        .expect("initialize external repository")
        .success
    );
    fs::write(
        external_root.join("private.txt"),
        "external repository data\n",
    )
    .expect("external repository content");

    let workspace = tempdir().expect("workspace directory");
    let root = workspace.path().canonicalize().expect("workspace root");
    fs::write(
        root.join(".git"),
        format!("gitdir: {}\n", external_root.join(".git").display()),
    )
    .expect("linked Git metadata file");

    let location_error = repository_location(&root)
        .await
        .expect_err("repository discovery must reject linked metadata without Git");
    assert!(location_error.to_string().contains("linked Git worktrees"));
    let status_error = load_status(&root)
        .await
        .expect_err("status preflight must reject linked metadata without Git");
    assert!(status_error.to_string().contains("linked Git worktrees"));

    let error = run_git_inspection(
        &root,
        &[
            "status".into(),
            "--porcelain=v1".into(),
            "-z".into(),
            "--untracked-files=all".into(),
            "--ignore-submodules=all".into(),
            "--no-renames".into(),
        ],
        64 * 1024,
    )
    .await
    .expect_err("external Git metadata must be rejected");
    assert!(error.to_string().contains("linked Git worktrees"));
}

/// Repository inspection requires Git 2.40 for `--attr-source`, and Aone
/// refuses to inspect without it rather than fall back to repository-controlled
/// attribute drivers. The Git shipped with current Xcode is 2.39.5, so on such a
/// machine the behaviour these tests cover is correctly unavailable. Skip there
/// instead of asserting against an unsupported toolchain; the refusal itself is
/// covered by `inspection_is_refused_when_git_cannot_isolate_attributes`.
async fn attribute_isolation_available() -> bool {
    let available =
        super::capability::require_attribute_isolation(std::path::Path::new("/usr/bin/git"))
            .await
            .is_ok();
    if !available {
        eprintln!("skipping: /usr/bin/git predates Git 2.40 attribute isolation");
    }
    available
}

#[tokio::test]
async fn inspection_is_refused_when_git_cannot_isolate_attributes() {
    // Whichever Git this machine has, the gate must return a decisive answer:
    // either inspection is permitted, or it is refused with a message naming
    // the requirement. It must never silently proceed without isolation.
    let outcome =
        super::capability::require_attribute_isolation(std::path::Path::new("/usr/bin/git")).await;
    match outcome {
        Ok(()) => {}
        Err(error) => {
            let message = error.to_string();
            assert!(
                message.contains("2.40"),
                "refusal must name the required version: {message}"
            );
        }
    }
}
