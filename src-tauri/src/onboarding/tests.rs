use std::{fs, path::Path};

use tempfile::tempdir;

use super::{
    commands::ensure_workspace_match,
    inspection::{parse_config, read_public_keys, sanitize_remote},
    paths::{create_attested_directory, direct_child, ensure_absent, ensure_created_directory},
    process::{clone_arguments, initialize_repository},
    state::OnboardingState,
    validation::{validate_github_repository, validate_project_name},
};
use crate::domain::GitRemoteTransport;

#[test]
fn github_url_is_strict_and_normalized() {
    let repository = validate_github_repository("https://github.com/openai/codex").unwrap();
    assert_eq!(
        repository.canonical_url,
        "https://github.com/openai/codex.git"
    );
    assert_eq!(repository.owner_repo, "openai/codex");
    for rejected in [
        "http://github.com/openai/codex",
        "https://user@github.com/openai/codex",
        "https://github.com/openai/codex?token=secret",
        "https://github.com/openai/codex/extra",
        "https://github.com/openai/%63odex",
    ] {
        assert!(validate_github_repository(rejected).is_err(), "{rejected}");
    }
}

#[test]
fn project_names_are_single_safe_components() {
    assert_eq!(
        validate_project_name("hello-world", "name").unwrap(),
        "hello-world"
    );
    for rejected in [
        "",
        ".hidden",
        "../escape",
        "nested/name",
        "bad name",
        "name.",
    ] {
        assert!(
            validate_project_name(rejected, "name").is_err(),
            "{rejected}"
        );
    }
}

#[test]
fn clone_plan_is_shallow_bounded_and_separator_protected() {
    let args = clone_arguments("https://github.com/openai/codex.git");
    assert!(args.contains(&"--depth=1".into()));
    assert!(args.contains(&"--no-recurse-submodules".into()));
    assert!(!args.iter().any(|argument| argument.starts_with("--filter")));
    assert_eq!(args[args.len() - 3], "--");
    assert_eq!(args.last().unwrap(), ".");
}

#[tokio::test]
async fn isolated_git_initialization_creates_only_local_metadata() {
    let root = tempdir().unwrap();
    initialize_repository(root.path()).await.unwrap();
    assert!(root.path().join(".git").is_dir());
    assert!(!root.path().join(".git/hooks").exists());
    let config = fs::read_to_string(root.path().join(".git/config")).unwrap();
    assert!(!config.contains("remote \""));
}

#[test]
fn destination_creation_refuses_overwrite_and_identity_changes() {
    let root = tempdir().unwrap();
    let documents = root.path();
    let target = direct_child(documents, "project").unwrap();
    ensure_absent(&target).unwrap();
    let identity = create_attested_directory(&target).unwrap();
    assert!(ensure_absent(&target).is_err());
    ensure_created_directory(documents, &target, &identity).unwrap();
    fs::rename(&target, documents.join("moved")).unwrap();
    fs::create_dir(&target).unwrap();
    assert!(ensure_created_directory(documents, &target, &identity).is_err());
}

#[cfg(unix)]
#[test]
fn destination_attestation_rejects_symlink_replacement() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let target = root.path().join("project");
    let identity = create_attested_directory(&target).unwrap();
    fs::rename(&target, root.path().join("real")).unwrap();
    symlink(root.path().join("real"), &target).unwrap();
    assert!(ensure_created_directory(root.path(), &target, &identity).is_err());
}

#[test]
fn remote_urls_are_sanitized() {
    let remote = sanitize_remote(
        "origin",
        "https://user:token@github.com/example/repo.git?access_token=secret#fragment",
    );
    assert_eq!(remote.display_url, "https://github.com/example/repo.git");
    assert_eq!(remote.host.as_deref(), Some("github.com"));
    assert_eq!(remote.owner_repo.as_deref(), Some("example/repo"));
    assert!(matches!(remote.transport, GitRemoteTransport::Https));
    let local = sanitize_remote("backup", "/Users/person/private/repo.git");
    assert_eq!(local.display_url, "[local repository]");
    assert!(local.host.is_none());
}

#[test]
fn git_config_inline_comments_never_enter_identity_or_remote_reports() {
    let config = parse_config(
        br#"
[user]
name = Alice # private note
email = alice@example.com ; internal account
[remote "origin"]
url = https://github.com/example/repo.git ; private note
[remote "quoted"]
url = "https://github.com/example/repo.git#literal"
"#,
    );
    assert_eq!(config.name.as_deref(), Some("Alice"));
    assert_eq!(config.email.as_deref(), Some("alice@example.com"));
    assert_eq!(
        config.remotes.get("origin").map(String::as_str),
        Some("https://github.com/example/repo.git")
    );
    assert_eq!(
        config.remotes.get("quoted").map(String::as_str),
        Some("https://github.com/example/repo.git#literal")
    );
    let serialized = format!("{config:?}");
    assert!(!serialized.contains("private note"));
    assert!(!serialized.contains("internal account"));
}

#[test]
fn public_key_scan_ignores_every_non_pub_file() {
    let root = tempdir().unwrap();
    let ssh = root.path().join(".ssh");
    fs::create_dir(&ssh).unwrap();
    fs::write(ssh.join("id_ed25519"), b"PRIVATE MATERIAL MUST NOT BE READ").unwrap();
    let blob = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        b"bounded-public-key",
    );
    fs::write(
        ssh.join("id_ed25519.pub"),
        format!("ssh-ed25519 {blob} ignored-comment\n"),
    )
    .unwrap();
    let keys = read_public_keys(root.path());
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].file_name, "id_ed25519.pub");
    assert!(keys[0].fingerprint.starts_with("SHA256:"));
}

#[cfg(unix)]
#[test]
fn public_key_scan_does_not_follow_pub_symlinks() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let ssh = root.path().join(".ssh");
    fs::create_dir(&ssh).unwrap();
    let outside = root.path().join("outside");
    fs::write(&outside, b"ssh-ed25519 YmFk bad").unwrap();
    symlink(&outside, ssh.join("leak.pub")).unwrap();
    assert!(read_public_keys(root.path()).is_empty());
}

#[cfg(unix)]
#[test]
fn public_key_scan_does_not_follow_ssh_directory_symlinks() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("outside.pub"), b"ssh-ed25519 YmFk bad").unwrap();
    symlink(outside.path(), root.path().join(".ssh")).unwrap();
    assert!(read_public_keys(root.path()).is_empty());
}

#[test]
fn workspace_identity_and_single_flight_fail_closed() {
    assert!(ensure_workspace_match("actual", "other").is_err());
    let state = OnboardingState::new();
    let first = state.reserve().unwrap();
    assert!(state.reserve().is_err());
    drop(first);
    assert!(state.reserve().is_ok());
}

#[allow(dead_code)]
fn _path_is_never_renderer_supplied(_path: &Path) {}
