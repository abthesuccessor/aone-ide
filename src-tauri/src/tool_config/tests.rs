use std::fs;

use tempfile::tempdir;

use super::{
    catalog::{describe_static, inspection_state_for_target, specs},
    commands::reserve_inspection,
    confirmation::{configuration_confirmation_message, inspection_confirmation_message},
    files::{attest_target, create_config_atomically, revalidate_target},
};
use crate::domain::{ToolConfigurationKind, ToolInspectionState};

#[test]
fn catalog_has_only_backend_owned_entries() {
    let ids = specs().iter().map(|spec| spec.id).collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "codex",
            "claude-desktop",
            "cursor",
            "gemini-cli",
            "vscode-workspace-mcp",
            "workspace-agents-md",
            "claude-workspace-instructions",
            "gemini-workspace-instructions",
            "cursor-project-rule",
            "copilot-workspace-instructions",
            "portable-project-skill",
        ]
    );
    assert!(
        specs()
            .iter()
            .any(|spec| spec.kind == ToolConfigurationKind::Agent)
    );
    assert!(
        specs()
            .iter()
            .any(|spec| spec.kind == ToolConfigurationKind::Skill)
    );
    assert!(
        specs()
            .iter()
            .any(|spec| spec.kind == ToolConfigurationKind::Rules)
    );
    for spec in specs() {
        assert!(!spec.path_hint.starts_with('/'));
        if spec.path_hint.ends_with(".json") {
            serde_json::from_str::<serde_json::Value>(spec.template)
                .expect("JSON template must remain valid");
        } else if spec.path_hint.ends_with(".toml") {
            toml::from_str::<toml::Value>(spec.template).expect("TOML template must remain valid");
        } else {
            assert!(spec.template.contains('#'));
        }
        let lower = spec.template.to_ascii_lowercase();
        for forbidden in [
            "api key", "password", "secret", "token", "command:", "script:",
        ] {
            assert!(
                !lower.contains(forbidden),
                "unsafe template text for {}",
                spec.id
            );
        }
    }
}

#[test]
fn static_catalog_performs_no_inspection_and_reports_unknown_state() {
    let descriptions = specs().iter().map(describe_static).collect::<Vec<_>>();
    assert_eq!(descriptions.len(), 11);
    assert!(descriptions.iter().all(|description| {
        description.configuration_state == ToolInspectionState::NotInspected
            && description.cli_state == ToolInspectionState::NotInspected
    }));
}

#[tokio::test]
async fn admits_only_one_configuration_inspection() {
    let first = reserve_inspection().expect("first inspection reservation");
    let error = reserve_inspection().expect_err("second inspection must be rejected");
    assert!(error.to_string().contains("already awaiting approval"));
    drop(first);
    assert!(reserve_inspection().is_ok());
}

#[test]
fn inspection_consent_discloses_metadata_scope_and_no_execution() {
    let message = inspection_confirmation_message();
    assert!(message.contains("at most 64 PATH directories"));
    assert!(message.contains("Configuration contents"));
    assert!(message.contains("will not run an executable"));
    assert!(message.contains("recursively scan this Mac"));
}

#[test]
fn creates_private_config_once_without_overwrite() {
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let target = root.join("nested/config.json");
    assert!(create_config_atomically(&root, &target, "{\"first\":true}\n").expect("first create"));
    assert!(
        !create_config_atomically(&root, &target, "{\"second\":true}\n").expect("second create")
    );
    assert_eq!(
        fs::read_to_string(&target).expect("read test target"),
        "{\"first\":true}\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(&target)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[cfg(unix)]
#[test]
fn refuses_symlink_configuration_target() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let real = root.join("real.json");
    let target = root.join("config.json");
    fs::write(&real, "{}\n").expect("real config");
    symlink(&real, &target).expect("symlink");
    assert!(attest_target(&root, &target).is_err());
    assert!(create_config_atomically(&root, &target, "{}\n").is_err());
    assert_eq!(
        inspection_state_for_target(&root, &target),
        ToolInspectionState::NotFound
    );
}

#[cfg(unix)]
#[test]
fn refuses_workspace_parent_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().expect("workspace");
    let outside = tempdir().expect("outside directory");
    let root = workspace.path().canonicalize().expect("canonical root");
    fs::write(outside.path().join("mcp.json"), "{\"outside\":true}\n").expect("outside config");
    symlink(outside.path(), root.join(".vscode")).expect("parent symlink");
    let target = root.join(".vscode/mcp.json");

    assert!(attest_target(&root, &target).is_err());
    assert!(create_config_atomically(&root, &target, "{\"servers\":{}}\n").is_err());
}

#[test]
fn rejects_post_consent_target_identity_change() {
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let target = root.join("config.json");
    let original = root.join("original.json");
    fs::write(&target, "{\"first\":true}\n").expect("original config");
    let approved = attest_target(&root, &target).expect("pre-consent attestation");

    fs::rename(&target, &original).expect("replace original inode");
    fs::write(&target, "{\"other\":true}\n").expect("replacement config");

    let error = revalidate_target(&root, &target, &approved)
        .expect_err("replacement after consent must fail");
    assert!(error.to_string().contains("changed after confirmation"));
}

#[cfg(unix)]
#[test]
fn rejects_post_consent_parent_containment_change() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().expect("workspace");
    let outside = tempdir().expect("outside directory");
    let root = workspace.path().canonicalize().expect("canonical root");
    let parent = root.join(".vscode");
    let held_parent = root.join(".vscode-approved");
    fs::create_dir(&parent).expect("configuration parent");
    let target = parent.join("mcp.json");
    let approved = attest_target(&root, &target).expect("missing target attestation");

    fs::rename(&parent, &held_parent).expect("move approved parent");
    symlink(outside.path(), &parent).expect("replacement parent symlink");

    assert!(revalidate_target(&root, &target, &approved).is_err());
}

#[test]
fn confirmation_binds_provider_to_absolute_attested_target() {
    let directory = tempdir().expect("temp directory");
    let root = directory.path().canonicalize().expect("canonical root");
    let target = root.join("config.toml");
    let approved = attest_target(&root, &target).expect("target attestation");
    let message = configuration_confirmation_message(&specs()[0], true, approved.approved_path());

    assert!(message.contains("Owner: OpenAI"));
    assert!(message.contains("~/.codex/config.toml"));
    assert!(message.contains(&format!("{:?}", approved.approved_path())));
}
