use std::collections::{BTreeMap, BTreeSet};

use super::{
    catalog::TOOL_CATALOG, config_hints::ProjectHints, discovery::DiscoveredTools,
    markers::MarkerEvidence, probe::ProbeResult, recommendations::build_recommendations,
};
use crate::domain::{
    LanguageSummary, ProjectEnvironmentEvidence, ProjectEnvironmentReport, ProjectStack,
    ProjectStackConfidence, ProjectTool, ProjectToolStatus, RunProfile,
};

struct StackRule {
    id: &'static str,
    label: &'static str,
    languages: &'static [&'static str],
    profile_kinds: &'static [&'static str],
    base_tools: &'static [&'static str],
}

const STACK_RULES: &[StackRule] = &[
    StackRule {
        id: "javascript",
        label: "Node.js / JavaScript",
        languages: &["JavaScript", "TypeScript"],
        profile_kinds: &["node"],
        base_tools: &["node"],
    },
    StackRule {
        id: "rust",
        label: "Rust",
        languages: &["Rust"],
        profile_kinds: &["cargo"],
        base_tools: &["cargo", "rustc"],
    },
    StackRule {
        id: "python",
        label: "Python",
        languages: &["Python"],
        profile_kinds: &["python"],
        base_tools: &["python"],
    },
    StackRule {
        id: "go",
        label: "Go",
        languages: &["Go"],
        profile_kinds: &["go"],
        base_tools: &["go"],
    },
    StackRule {
        id: "jvm",
        label: "Java / Kotlin",
        languages: &["Java", "Kotlin"],
        profile_kinds: &["java"],
        base_tools: &["java"],
    },
    StackRule {
        id: "dotnet",
        label: ".NET",
        languages: &["C#"],
        profile_kinds: &["dotnet"],
        base_tools: &["dotnet"],
    },
    StackRule {
        id: "native",
        label: "C / C++",
        languages: &["C++"],
        profile_kinds: &[],
        base_tools: &["clangxx"],
    },
    StackRule {
        id: "containers",
        label: "Docker Compose",
        languages: &[],
        profile_kinds: &["dockerCompose"],
        base_tools: &["docker", "docker-compose"],
    },
    StackRule {
        id: "container-image",
        label: "Container image",
        languages: &[],
        profile_kinds: &[],
        base_tools: &[],
    },
    StackRule {
        id: "ruby",
        label: "Ruby",
        languages: &[],
        profile_kinds: &[],
        base_tools: &["ruby", "bundler"],
    },
    StackRule {
        id: "php",
        label: "PHP",
        languages: &[],
        profile_kinds: &[],
        base_tools: &["php", "composer"],
    },
    StackRule {
        id: "swift",
        label: "Swift",
        languages: &[],
        profile_kinds: &[],
        base_tools: &["swift"],
    },
    StackRule {
        id: "terraform",
        label: "Terraform",
        languages: &[],
        profile_kinds: &[],
        base_tools: &["terraform"],
    },
    StackRule {
        id: "dart",
        label: "Dart / Flutter",
        languages: &[],
        profile_kinds: &[],
        base_tools: &["dart"],
    },
];

pub(super) struct ProjectFacts {
    pub(super) stacks: Vec<ProjectStack>,
    pub(super) required_tool_ids: BTreeSet<&'static str>,
    pub(super) used_by: BTreeMap<&'static str, Vec<String>>,
    pub(super) probe_tool_ids: BTreeSet<&'static str>,
}

pub(super) struct ReportIdentity {
    pub(super) report_id: String,
    pub(super) workspace_id: String,
    pub(super) inspected_at: String,
    pub(super) version_probe_approved: bool,
}

pub(super) fn derive_project_facts(
    languages: &[LanguageSummary],
    profiles: &[RunProfile],
    markers: &[MarkerEvidence],
    git_workspace: bool,
) -> ProjectFacts {
    let language_counts = languages
        .iter()
        .map(|language| (language.language.as_str(), language.file_count))
        .collect::<BTreeMap<_, _>>();
    let mut stacks = Vec::new();
    let mut required_tool_ids = BTreeSet::new();
    let mut used_by: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();

    for rule in STACK_RULES {
        let matched_profiles = profiles
            .iter()
            .filter(|profile| rule.profile_kinds.contains(&profile.kind.as_str()))
            .collect::<Vec<_>>();
        let matched_languages = rule
            .languages
            .iter()
            .filter_map(|language| {
                language_counts
                    .get(language)
                    .map(|count| (*language, *count))
            })
            .collect::<Vec<_>>();
        let matched_markers = markers
            .iter()
            .filter(|marker| marker.stack_id == rule.id)
            .collect::<Vec<_>>();
        if matched_profiles.is_empty() && matched_languages.is_empty() && matched_markers.is_empty()
        {
            continue;
        }
        let mut evidence = matched_profiles
            .iter()
            .take(5)
            .map(|profile| ProjectEnvironmentEvidence {
                relative_path: safe_text(&profile.source, 300),
                detail: format!("Detected run profile: {}", safe_text(&profile.name, 160)),
            })
            .collect::<Vec<_>>();
        evidence.extend(matched_languages.iter().take(3).map(|(language, count)| {
            ProjectEnvironmentEvidence {
                relative_path: ".".into(),
                detail: format!("Language scan found {count} {language} file(s)."),
            }
        }));
        evidence.extend(
            matched_markers
                .iter()
                .take(5)
                .map(|marker| marker.evidence.clone()),
        );
        stacks.push(ProjectStack {
            id: rule.id.into(),
            label: rule.label.into(),
            confidence: if matched_profiles.is_empty() && matched_markers.is_empty() {
                ProjectStackConfidence::Inferred
            } else {
                ProjectStackConfidence::Confirmed
            },
            evidence,
        });
        for tool_id in rule.base_tools {
            required_tool_ids.insert(*tool_id);
            used_by.entry(*tool_id).or_default().push(rule.id.into());
        }
        for profile in matched_profiles {
            if let Some(tool_id) = profile_tool_id(&profile.executable) {
                required_tool_ids.insert(tool_id);
                used_by.entry(tool_id).or_default().push(rule.id.into());
            }
        }
        for marker in matched_markers {
            if let Some(tool_id) = marker_tool_id(&marker.evidence.relative_path) {
                required_tool_ids.insert(tool_id);
                used_by.entry(tool_id).or_default().push(rule.id.into());
            }
        }
        if rule.id == "jvm"
            && matched_languages
                .iter()
                .any(|(language, _)| *language == "Kotlin")
        {
            required_tool_ids.insert("kotlinc");
            used_by.entry("kotlinc").or_default().push(rule.id.into());
        }
    }
    for stack_ids in used_by.values_mut() {
        stack_ids.sort();
        stack_ids.dedup();
    }
    let mut probe_tool_ids = required_tool_ids.clone();
    if git_workspace {
        probe_tool_ids.insert("git");
    }
    ProjectFacts {
        stacks,
        required_tool_ids,
        used_by,
        probe_tool_ids,
    }
}

pub(super) fn build_report(
    identity: ReportIdentity,
    facts: ProjectFacts,
    profiles: &[RunProfile],
    discovered: &DiscoveredTools,
    probes: &BTreeMap<&'static str, ProbeResult>,
    hints: &ProjectHints,
) -> ProjectEnvironmentReport {
    let tools = TOOL_CATALOG
        .iter()
        .map(|spec| {
            let candidates = discovered
                .get(spec.id)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let probe = probes.get(spec.id);
            let status = if candidates.is_empty() {
                ProjectToolStatus::Missing
            } else if probe.and_then(|result| result.version.as_ref()).is_some() {
                ProjectToolStatus::Available
            } else {
                ProjectToolStatus::Unverified
            };
            ProjectTool {
                id: spec.id.into(),
                label: spec.label.into(),
                category: spec.category.into(),
                required: facts.required_tool_ids.contains(spec.id),
                status,
                canonical_path: candidates
                    .first()
                    .map(|candidate| path_text(candidate.canonical_path())),
                alternate_canonical_paths: candidates
                    .iter()
                    .skip(1)
                    .map(|candidate| path_text(candidate.canonical_path()))
                    .collect(),
                version: probe.and_then(|result| result.version.clone()),
                version_args: spec.version_args.iter().map(|arg| (*arg).into()).collect(),
                probe_error: probe.and_then(|result| result.error.clone()),
                used_by: facts.used_by.get(spec.id).cloned().unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let recommendations = build_recommendations(
        &facts.stacks,
        profiles,
        &tools,
        identity.version_probe_approved,
        hints,
    );
    ProjectEnvironmentReport {
        report_id: identity.report_id,
        workspace_id: identity.workspace_id,
        inspected_at: identity.inspected_at,
        version_probe_approved: identity.version_probe_approved,
        stacks: facts.stacks,
        tools,
        recommendations,
    }
}

fn profile_tool_id(executable: &str) -> Option<&'static str> {
    match executable {
        "npm" => Some("npm"),
        "pnpm" => Some("pnpm"),
        "yarn" => Some("yarn"),
        "bun" => Some("bun"),
        "cargo" => Some("cargo"),
        "go" => Some("go"),
        "dotnet" => Some("dotnet"),
        "docker" => Some("docker-compose"),
        "python" | "python3" => Some("python"),
        "mvn" => Some("maven"),
        "gradle" => Some("gradle"),
        _ => None,
    }
}

fn marker_tool_id(relative_path: &str) -> Option<&'static str> {
    let name = std::path::Path::new(relative_path)
        .file_name()
        .and_then(|value| value.to_str())?;
    match name {
        "package-lock.json" => Some("npm"),
        "pnpm-lock.yaml" => Some("pnpm"),
        "yarn.lock" => Some("yarn"),
        "bun.lock" | "bun.lockb" => Some("bun"),
        "requirements.txt" => Some("pip"),
        "poetry.lock" => Some("poetry"),
        "uv.lock" => Some("uv"),
        "Gemfile" | "Gemfile.lock" => Some("bundler"),
        "composer.json" | "composer.lock" => Some("composer"),
        "CMakeLists.txt" => Some("cmake"),
        "Makefile" => Some("make"),
        _ => None,
    }
}

fn path_text(path: &std::path::Path) -> String {
    path.to_str().unwrap_or_default().into()
}

fn safe_text(value: &str, max_bytes: usize) -> String {
    let mut result = String::new();
    for character in value.chars() {
        let normalized = if character.is_control() {
            ' '
        } else {
            character
        };
        if result.len() + normalized.len_utf8() > max_bytes {
            break;
        }
        result.push(normalized);
    }
    result
}
