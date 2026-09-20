use super::profile::{attach_required_env, profile};
use std::{fs, io::Write};

use tempfile::tempdir;

use super::{detect_profiles, package_json::TEST_MAX_PACKAGE_MANIFEST_BYTES};

#[test]
fn detects_structured_node_profiles_without_shell_interpolation() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"dev":"vite","test":"vitest"}}"#,
    )
    .unwrap();
    let profiles = detect_profiles(root.path(), "workspace").unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].executable, "npm");
    assert_eq!(profiles[0].args, vec!["run", "dev"]);
    assert!(!profiles[0].args.iter().any(|value| value.contains(';')));
}

#[test]
fn oversized_package_manifest_is_skipped_without_hiding_other_profiles() {
    let root = tempdir().unwrap();
    let mut package = fs::File::create(root.path().join("package.json")).unwrap();
    package
        .write_all(&vec![b' '; TEST_MAX_PACKAGE_MANIFEST_BYTES as usize + 1])
        .unwrap();
    fs::write(root.path().join("Cargo.toml"), "[package]\nname='sample'\n").unwrap();

    let profiles = detect_profiles(root.path(), "workspace").unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].executable, "cargo");
}

#[test]
fn malformed_package_manifest_is_skipped_without_hiding_other_profiles() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("package.json"), r#"{"scripts":{"dev":}"#).unwrap();
    fs::write(root.path().join("go.mod"), "module example.test/sample\n").unwrap();

    let profiles = detect_profiles(root.path(), "workspace").unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].executable, "go");
}

#[test]
fn discovered_profiles_declare_the_projects_own_environment_names() {
    let mut profiles = vec![
        profile(
            "w",
            "package.json#dev",
            "dev",
            "npm",
            vec!["run".into()],
            None,
            "node",
        ),
        profile(
            "w",
            "Cargo.toml",
            "cargo run",
            "cargo",
            vec!["run".into()],
            None,
            "rust",
        ),
    ];
    // Unsorted, duplicated, and mixed with tokens that are not variable names.
    attach_required_env(
        &mut profiles,
        &[
            "STRIPE_SECRET_KEY".into(),
            "DATABASE_URL".into(),
            "DATABASE_URL".into(),
            "lowercase".into(),
            "9LEADING_DIGIT".into(),
            "HAS-DASH".into(),
            String::new(),
        ],
    );
    for profile in &profiles {
        assert_eq!(
            profile.required_env,
            vec!["DATABASE_URL".to_string(), "STRIPE_SECRET_KEY".to_string()],
            "names must be filtered, deduped and ordered deterministically"
        );
    }
}

#[test]
fn attaching_no_names_leaves_profiles_untouched() {
    let mut profiles = vec![profile(
        "w",
        "Cargo.toml",
        "cargo run",
        "cargo",
        vec!["run".into()],
        None,
        "rust",
    )];
    attach_required_env(&mut profiles, &[]);
    assert!(profiles[0].required_env.is_empty());
}

#[test]
fn required_env_is_bounded_however_many_names_are_detected() {
    let mut profiles = vec![profile(
        "w",
        "Cargo.toml",
        "cargo run",
        "cargo",
        vec!["run".into()],
        None,
        "rust",
    )];
    let many = (0..200)
        .map(|index| format!("VAR_{index:03}"))
        .collect::<Vec<_>>();
    attach_required_env(&mut profiles, &many);
    assert_eq!(
        profiles[0].required_env.len(),
        super::profile::MAX_REQUIRED_ENV
    );
}
