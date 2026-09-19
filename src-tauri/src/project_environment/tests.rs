use super::{
    analysis::{ReportIdentity, build_report, derive_project_facts},
    catalog::TOOL_CATALOG,
    command::validate_workspace_id_for_test,
    config_hints::{ProjectHints, collect_project_hints, hint_source_limits_for_test},
    confirmation::{discovery_confirmation_message, probe_confirmation_message},
    discovery::{
        DiscoveredTools, ExecutableCandidate, TEST_MAX_CANDIDATES_PER_TOOL,
        candidate_location_allowed_for_test, clean_absolute_for_test,
        home_tool_directories_for_test, safe_display_path_for_test,
    },
    identity::{capture_executable_identity, revalidate_executable_identity},
    markers::{MarkerEvidence, collect_marker_evidence, marker_matches_for_test},
    probe::{build_probe_plan, join_output_for_test, normalize_version_for_test},
    recommendations::build_recommendations,
};
use crate::domain::{
    CapabilityLevel, LanguageSummary, ProjectEnvironmentEvidence, ProjectToolStatus, RunProfile,
};
use crate::{scanner::scan_workspace, store::GraphStore};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
    time::{Duration, Instant},
};
use tempfile::tempdir;

#[test]
fn catalog_is_fixed_unique_and_covers_requested_ecosystems() {
    let ids = TOOL_CATALOG
        .iter()
        .map(|tool| tool.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), TOOL_CATALOG.len());
    assert_eq!(TOOL_CATALOG.len(), 39);
    for expected in [
        "node",
        "npm",
        "pnpm",
        "yarn",
        "bun",
        "deno",
        "python",
        "pip",
        "uv",
        "poetry",
        "cargo",
        "rustc",
        "go",
        "java",
        "javac",
        "maven",
        "gradle",
        "dotnet",
        "ruby",
        "bundler",
        "php",
        "composer",
        "swift",
        "kotlin",
        "kotlinc",
        "clang",
        "clangxx",
        "gcc",
        "gxx",
        "cmake",
        "ninja",
        "make",
        "docker",
        "docker-compose",
        "terraform",
        "git",
        "zig",
        "dart",
        "flutter",
    ] {
        assert!(ids.contains(expected), "missing {expected}");
    }
    assert!(TOOL_CATALOG.iter().all(|tool| {
        !tool.names.is_empty()
            && !tool.version_args.is_empty()
            && tool.names.iter().all(|name| !name.contains('/'))
    }));
}

#[test]
fn discovery_filters_relative_workspace_and_unsafe_display_paths() {
    assert!(clean_absolute_for_test(Path::new("/usr/local/bin")));
    assert!(!clean_absolute_for_test(Path::new("relative/bin")));
    assert!(!clean_absolute_for_test(Path::new("/usr/../tmp/bin")));
    assert!(!safe_display_path_for_test(Path::new("/tmp/bad\nname")));
    assert!(!candidate_location_allowed_for_test(
        Path::new("/tmp/workspace"),
        Path::new("/tmp/workspace/bin/node"),
        Path::new("/opt/node"),
    ));
    assert!(candidate_location_allowed_for_test(
        Path::new("/tmp/workspace"),
        Path::new("/usr/local/bin/node"),
        Path::new("/opt/node/bin/node"),
    ));
    assert_eq!(TEST_MAX_CANDIDATES_PER_TOOL, 4);
    let home_directories = home_tool_directories_for_test(Path::new("/Users/example"));
    assert!(home_directories.contains(&Path::new("/Users/example/.cargo/bin").to_path_buf()));
    assert!(home_directories.contains(&Path::new("/Users/example/.volta/bin").to_path_buf()));
    assert!(
        home_directories.contains(
            &Path::new("/Users/example/.sdkman/candidates/java/current/bin").to_path_buf()
        )
    );
}

#[test]
fn marker_matching_is_exact_and_does_not_read_contents() {
    assert!(marker_matches_for_test(
        "services/api/Gemfile",
        false,
        "Gemfile"
    ));
    assert!(!marker_matches_for_test(
        "services/api/Gemfile.old",
        false,
        "Gemfile"
    ));
    assert!(marker_matches_for_test("infra/main.tf", true, "tf"));
    assert!(!marker_matches_for_test("infra/main.tf.json", true, "tf"));
    assert!(marker_matches_for_test("Dockerfile", false, "Dockerfile"));
    assert!(marker_matches_for_test(
        "services/api/Dockerfile",
        false,
        "Dockerfile"
    ));
    assert!(!marker_matches_for_test(
        "Dockerfile.dev",
        false,
        "Dockerfile"
    ));
}

#[test]
fn executable_identity_detects_changes_and_non_executables() {
    let directory = tempdir().unwrap();
    let executable = directory.path().join("tool");
    std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
    set_executable(&executable, true);
    let identity = capture_executable_identity(&executable).unwrap();
    revalidate_executable_identity(&executable, &identity).unwrap();
    std::fs::write(&executable, b"#!/bin/sh\nexit 0\n# changed\n").unwrap();
    assert!(revalidate_executable_identity(&executable, &identity).is_err());

    let plain = directory.path().join("plain");
    std::fs::write(&plain, b"plain").unwrap();
    set_executable(&plain, false);
    #[cfg(unix)]
    assert!(capture_executable_identity(&plain).is_err());
}

#[test]
fn probe_plan_and_native_message_are_bounded_and_exact() {
    let directory = tempdir().unwrap();
    let executable = directory.path().join("approved-tool");
    std::fs::write(&executable, b"#!/bin/sh\necho version\n").unwrap();
    set_executable(&executable, true);
    let identity = capture_executable_identity(&executable).unwrap();
    let mut discovered = DiscoveredTools::new();
    let mut requested = BTreeSet::new();
    for spec in TOOL_CATALOG {
        requested.insert(spec.id);
        discovered.insert(
            spec.id,
            vec![ExecutableCandidate {
                tool_id: spec.id,
                launch_name: spec.names[0],
                launch_path: executable.clone(),
                identity: identity.clone(),
            }],
        );
    }
    let plan = build_probe_plan(&discovered, &requested);
    let message = probe_confirmation_message(&plan);
    assert_eq!(plan.probes.len(), 16);
    assert!(plan.omitted_count > 0);
    assert!(message.len() < 5_000);
    for planned in &plan.probes {
        assert!(message.contains(&planned.approval_line));
    }
}

#[test]
fn discovery_consent_precedes_and_describes_local_inspection() {
    let message = discovery_confirmation_message("Example", Path::new("/tmp/example"));
    assert!(message.contains("No executable will run during this step"));
    assert!(message.contains("absolute PATH entries"));
    assert!(message.contains("at most 24 files, 64 KiB each, 512 KiB total"));
    assert!(message.contains("names-only environment"));
    assert!(message.contains("will not open project env files"));
    assert!(message.contains("Build/config values are not returned or retained"));
    assert!(message.contains("rather than a required-service or readiness claim"));
}

#[test]
fn version_normalization_is_single_line_printable_and_bounded() {
    assert_eq!(
        normalize_version_for_test(b"\nversion \x071.2.3\nsecret second line"),
        Some("version 1.2.3".into()),
    );
    let long = "é".repeat(300);
    let normalized = normalize_version_for_test(long.as_bytes()).unwrap();
    assert!(normalized.len() <= 240);
    assert!(!normalized.chars().any(char::is_control));
}

#[tokio::test]
async fn inherited_output_pipe_cannot_hold_probe_completion_open() {
    let task = tokio::spawn(async { std::future::pending::<io::Result<Vec<u8>>>().await });
    let started = Instant::now();
    assert!(join_output_for_test(task).await.is_empty());
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn recommendations_are_deterministic_and_manifest_aware() {
    let languages = vec![LanguageSummary {
        language: "TypeScript".into(),
        file_count: 8,
        capability: CapabilityLevel::Semantic,
    }];
    let profiles = vec![RunProfile {
        id: "profile:dev".into(),
        name: "npm run dev".into(),
        kind: "node".into(),
        executable: "npm".into(),
        args: vec!["run".into(), "dev".into()],
        cwd_relative: None,
        required_env: Vec::new(),
        source: "package.json".into(),
    }];
    let markers = vec![MarkerEvidence {
        stack_id: "javascript",
        evidence: ProjectEnvironmentEvidence {
            relative_path: "package.json".into(),
            detail: "Indexed project marker for javascript.".into(),
        },
    }];
    let discovered = DiscoveredTools::new();
    let probes = BTreeMap::new();
    let first = build_report(
        report_identity(),
        derive_project_facts(&languages, &profiles, &markers, true),
        &profiles,
        &discovered,
        &probes,
        &ProjectHints::default(),
    );
    let second = build_report(
        report_identity(),
        derive_project_facts(&languages, &profiles, &markers, true),
        &profiles,
        &discovered,
        &probes,
        &ProjectHints::default(),
    );
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&second).unwrap()
    );
    assert_eq!(first.tools.len(), TOOL_CATALOG.len());
    assert_eq!(
        first.stacks[0].confidence,
        crate::domain::ProjectStackConfidence::Confirmed
    );
    assert!(first.tools.iter().any(|tool| tool.id == "node"
        && tool.required
        && tool.status == ProjectToolStatus::Missing));
    assert_eq!(
        first.recommendations[0].run_profile_id.as_deref(),
        Some("profile:dev")
    );
}

#[test]
fn detected_stack_without_a_profile_always_has_a_warning() {
    let stacks = vec![crate::domain::ProjectStack {
        id: "python".into(),
        label: "Python".into(),
        confidence: crate::domain::ProjectStackConfidence::Inferred,
        evidence: vec![ProjectEnvironmentEvidence {
            relative_path: ".".into(),
            detail: "Language scan found Python.".into(),
        }],
    }];
    let recommendations = build_recommendations(&stacks, &[], &[], true, &ProjectHints::default());
    assert_eq!(recommendations.len(), 1);
    assert_eq!(recommendations[0].id, "environment:no-run-profile");
    assert_eq!(
        recommendations[0].severity,
        crate::domain::ProjectEnvironmentRecommendationSeverity::Warning
    );
    assert!(
        recommendations[0]
            .summary
            .contains("no supported safe-argv run profile")
    );
}

#[test]
fn source_only_python_project_reports_bounded_non_authoritative_hints() {
    let directory = tempdir().unwrap();
    let source_directory = directory.path().join("src/bei_auth");
    std::fs::create_dir_all(&source_directory).unwrap();
    std::fs::write(
        source_directory.join("config.py"),
        r#"
class AuthSettings(ServiceSettings):
    model_config = SettingsConfigDict(
        env_prefix="BEI_AUTH_",
    )
    database_url: SecretStr = SecretStr("postgresql://user:DO_NOT_REPORT@db/app")
    redis_url: SecretStr = SecretStr("redis://:DO_NOT_REPORT@cache/0")
    token_issuer: str = "local"

UNRELATED_UPPERCASE = "SHOULD_NOT_BECOME_AN_ENV_NAME"
explicit = os.getenv("EXPLICIT_SETTING")
"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("pyproject.toml"),
        r#"
[project]
name = "example-auth"
dependencies = ["psycopg[binary]>=3", "redis[hiredis]>=7"]
"#,
    )
    .unwrap();
    std::fs::write(
        directory.path().join("Dockerfile"),
        "FROM python:3.14\nARG BUILD_MODE=release\n",
    )
    .unwrap();

    let root = directory.path().canonicalize().unwrap();
    let scan = scan_workspace(&root, "workspace:test", |_| {}).unwrap();
    let mut store = GraphStore::open(&directory.path().join("index.sqlite")).unwrap();
    store.replace_all(&scan.files).unwrap();
    let markers = collect_marker_evidence(&store).unwrap();
    let hints = collect_project_hints(&root, &store).unwrap();

    assert!(
        hints
            .environment_names
            .contains(&"BEI_AUTH_DATABASE_URL".into()),
        "inferred names: {:?}",
        hints.environment_names,
    );
    assert!(
        hints
            .environment_names
            .contains(&"BEI_AUTH_REDIS_URL".into())
    );
    assert!(hints.environment_names.contains(&"EXPLICIT_SETTING".into()));
    assert!(hints.environment_names.contains(&"BUILD_MODE".into()));
    assert!(
        !hints
            .environment_names
            .iter()
            .any(|name| name.contains("SHOULD_NOT") || name.contains("DO_NOT_REPORT"))
    );
    assert_eq!(
        hints
            .runtime_dependencies
            .iter()
            .map(|hint| hint.label)
            .collect::<Vec<_>>(),
        vec!["PostgreSQL", "Redis"]
    );

    let report = build_report(
        report_identity(),
        derive_project_facts(
            &[LanguageSummary {
                language: "Python".into(),
                file_count: 1,
                capability: CapabilityLevel::Semantic,
            }],
            &[],
            &markers,
            false,
        ),
        &[],
        &DiscoveredTools::new(),
        &BTreeMap::new(),
        &hints,
    );
    assert!(report.stacks.iter().any(|stack| {
        stack.id == "container-image"
            && stack.label == "Container image"
            && stack
                .evidence
                .iter()
                .any(|item| item.relative_path == "Dockerfile")
    }));
    assert!(
        report
            .tools
            .iter()
            .any(|tool| tool.id == "docker" && !tool.required)
    );
    assert!(
        report
            .tools
            .iter()
            .any(|tool| tool.id == "docker-compose" && !tool.required)
    );
    for expected in [
        "environment:no-run-profile",
        "environment:dockerfile-without-compose",
        "environment:inferred-names",
        "environment:runtime-hint:postgresql",
        "environment:runtime-hint:redis",
    ] {
        assert!(
            report
                .recommendations
                .iter()
                .any(|item| item.id == expected)
        );
    }
    let rendered = serde_json::to_string(&report.recommendations).unwrap();
    assert!(rendered.contains("packaging evidence only"));
    assert!(rendered.contains("No Docker Compose run profile was found"));
    assert!(rendered.contains("hint only"));
    assert!(rendered.contains("not proof"));
    assert!(rendered.contains("No values were collected"));
    assert!(!rendered.contains("DO_NOT_REPORT"));
    assert!(!rendered.contains("SHOULD_NOT_BECOME_AN_ENV_NAME"));
    assert_eq!(hint_source_limits_for_test(), (24, 64 * 1024, 512 * 1024));
}

#[test]
fn workspace_request_rejects_empty_oversized_and_control_ids() {
    assert!(validate_workspace_id_for_test("workspace:valid").is_ok());
    assert!(validate_workspace_id_for_test("").is_err());
    assert!(validate_workspace_id_for_test("workspace:\nspoof").is_err());
    assert!(validate_workspace_id_for_test(&"x".repeat(201)).is_err());
}

fn report_identity() -> ReportIdentity {
    ReportIdentity {
        report_id: "report:fixed".into(),
        workspace_id: "workspace:fixed".into(),
        inspected_at: "2026-08-17T00:00:00.000Z".into(),
        version_probe_approved: false,
    }
}

fn set_executable(path: &Path, executable: bool) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = if executable { 0o700 } else { 0o600 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = (path, executable);
}
