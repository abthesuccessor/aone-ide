use std::{collections::BTreeSet, path::Path};

use ignore::WalkBuilder;

use super::{package_json::package_profiles, profile::profile};
use crate::{domain::RunProfile, error::AoneResult, scanner::is_hard_denied};

const MAX_MANIFESTS: usize = 100;
const MAX_PROFILES: usize = 100;

pub fn detect_profiles(root: &Path, workspace_id: &str) -> AoneResult<Vec<RunProfile>> {
    let mut profiles = Vec::new();
    let root_for_filter = root.to_path_buf();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .parents(true)
        .follow_links(false)
        .max_depth(Some(8))
        .filter_entry(move |entry| {
            entry.path() == root_for_filter
                || entry
                    .path()
                    .strip_prefix(&root_for_filter)
                    .is_ok_and(|relative| !is_hard_denied(relative))
        });
    let mut manifests = builder
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .path()
                .strip_prefix(root)
                .is_ok_and(|relative| !is_hard_denied(relative))
        })
        .filter(|entry| is_run_manifest(entry.path()))
        .take(MAX_MANIFESTS)
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    manifests.sort();

    for manifest in manifests {
        let relative = manifest.strip_prefix(root).unwrap_or(&manifest);
        let cwd = relative
            .parent()
            .filter(|path| !path.as_os_str().is_empty());
        let cwd_relative = cwd.map(super::profile::path_string);
        let source = super::profile::path_string(relative);
        match manifest.file_name().and_then(|value| value.to_str()) {
            Some("package.json") => {
                // A malformed, oversized, unreadable, or concurrently removed
                // package manifest must not prevent discovery in other services.
                if let Ok(package) =
                    package_profiles(root, workspace_id, &manifest, cwd_relative, source)
                {
                    profiles.extend(package);
                }
            }
            Some("Cargo.toml") => profiles.push(profile(
                workspace_id,
                &source,
                "Cargo run",
                "cargo",
                vec!["run".into()],
                cwd_relative,
                "cargo",
            )),
            Some("go.mod") => profiles.push(profile(
                workspace_id,
                &source,
                "Go run",
                "go",
                vec!["run".into(), ".".into()],
                cwd_relative,
                "go",
            )),
            Some("manage.py") => profiles.push(profile(
                workspace_id,
                &source,
                "Django development server",
                "python3",
                vec!["manage.py".into(), "runserver".into()],
                cwd_relative,
                "python",
            )),
            Some("compose.yaml" | "compose.yml" | "docker-compose.yaml" | "docker-compose.yml") => {
                profiles.push(profile(
                    workspace_id,
                    &source,
                    "Docker Compose",
                    "docker",
                    vec!["compose".into(), "up".into()],
                    cwd_relative,
                    "dockerCompose",
                ));
            }
            Some("pom.xml") => profiles.push(profile(
                workspace_id,
                &source,
                "Maven application",
                "mvn",
                vec!["spring-boot:run".into()],
                cwd_relative,
                "java",
            )),
            Some("build.gradle" | "build.gradle.kts") => {
                let wrapper = manifest.parent().unwrap_or(root).join("gradlew");
                profiles.push(profile(
                    workspace_id,
                    &source,
                    "Gradle application",
                    if wrapper.is_file() {
                        "./gradlew"
                    } else {
                        "gradle"
                    },
                    vec!["run".into()],
                    cwd_relative,
                    "java",
                ));
            }
            Some(name) if name.ends_with(".csproj") => profiles.push(profile(
                workspace_id,
                &source,
                ".NET run",
                "dotnet",
                vec!["run".into()],
                cwd_relative,
                "dotnet",
            )),
            _ => {}
        }
        if profiles.len() >= MAX_PROFILES {
            profiles.truncate(MAX_PROFILES);
            break;
        }
    }

    let mut ids = BTreeSet::new();
    profiles.retain(|profile| ids.insert(profile.id.clone()));
    Ok(profiles)
}

fn is_run_manifest(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        name,
        "package.json"
            | "Cargo.toml"
            | "go.mod"
            | "manage.py"
            | "compose.yaml"
            | "compose.yml"
            | "docker-compose.yaml"
            | "docker-compose.yml"
            | "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
    ) || name.ends_with(".csproj")
}
