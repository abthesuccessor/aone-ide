use crate::domain::{
    ProjectEnvironmentEvidence, ProjectEnvironmentRecommendation,
    ProjectEnvironmentRecommendationKind, ProjectEnvironmentRecommendationSeverity, ProjectStack,
    ProjectTool, ProjectToolStatus, RunProfile,
};

use super::config_hints::ProjectHints;

pub(super) fn build_recommendations(
    stacks: &[ProjectStack],
    profiles: &[RunProfile],
    tools: &[ProjectTool],
    version_probe_approved: bool,
    hints: &ProjectHints,
) -> Vec<ProjectEnvironmentRecommendation> {
    let mut recommendations = Vec::new();
    if let Some(profile) = preferred_profile(profiles) {
        let args = serde_json::to_string(&profile.args).unwrap_or_else(|_| "[]".into());
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "run-profile:preferred".into(),
            title: format!("Run {}", safe_text(&profile.name, 120)),
            summary: format!(
                "Use the backend-detected profile `{}` with fixed arguments {}. Starting it still requires the separate native Run confirmation.",
                safe_text(&profile.executable, 80),
                safe_text(&args, 300),
            ),
            kind: ProjectEnvironmentRecommendationKind::RunProfile,
            severity: ProjectEnvironmentRecommendationSeverity::Recommended,
            evidence: vec![ProjectEnvironmentEvidence {
                relative_path: safe_text(&profile.source, 300),
                detail: "The backend derived this profile from a bounded project manifest.".into(),
            }],
            run_profile_id: Some(profile.id.clone()),
        });
    } else if !stacks.is_empty() {
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "environment:no-run-profile".into(),
            title: "No registered run profile".into(),
            summary: "A stack was detected, but no supported safe-argv run profile was found in the opened workspace root. Review project-owned docs or configuration; Aone did not infer, synthesize, or execute a command.".into(),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Warning,
            evidence: stack_evidence(stacks),
            run_profile_id: None,
        });
    }

    let dockerfile_evidence = stacks
        .iter()
        .filter(|stack| stack.id == "container-image")
        .flat_map(|stack| &stack.evidence)
        .filter(|item| {
            std::path::Path::new(&item.relative_path)
                .file_name()
                .and_then(|name| name.to_str())
                == Some("Dockerfile")
        })
        .take(8)
        .cloned()
        .collect::<Vec<_>>();
    if !dockerfile_evidence.is_empty()
        && !profiles
            .iter()
            .any(|profile| profile.kind == "dockerCompose")
    {
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "environment:dockerfile-without-compose".into(),
            title: "Dockerfile is packaging evidence only".into(),
            summary: "Hint only — a Dockerfile describes an image build, not a safe run command. No Docker Compose run profile was found in the opened workspace root; review project-owned deployment docs or open the deployment root before running containers.".into(),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Warning,
            evidence: dockerfile_evidence,
            run_profile_id: None,
        });
    }

    if !hints.environment_names.is_empty() {
        let names = hints.environment_names.join(", ");
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "environment:inferred-names".into(),
            title: "Inferred environment names (hint only)".into(),
            summary: safe_text(
                &format!(
                    "Hint only — bounded, recognized configuration syntax suggests these environment names may be understood by the project: {names}. No values were collected, and a name is not proof that it is required."
                ),
                1_000,
            ),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Optional,
            evidence: hints.environment_evidence.clone(),
            run_profile_id: None,
        });
    }
    recommendations.extend(hints.runtime_dependencies.iter().map(|hint| {
        ProjectEnvironmentRecommendation {
            id: format!("environment:runtime-hint:{}", hint.id),
            title: format!("{} runtime dependency (hint only)", hint.label),
            summary: format!(
                "Hint only — bounded dependency/config evidence suggests this project may connect to {}. This is not proof that the service is required, configured, reachable, or running.",
                hint.label,
            ),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Optional,
            evidence: hint.evidence.clone(),
            run_profile_id: None,
        }
    }));

    for tool in tools
        .iter()
        .filter(|tool| tool.required && tool.status == ProjectToolStatus::Missing)
    {
        recommendations.push(tool_recommendation(
            tool,
            stacks,
            "Required tool was not found in the approved search locations.",
            ProjectEnvironmentRecommendationSeverity::Warning,
        ));
    }
    for tool in tools
        .iter()
        .filter(|tool| tool.required && tool.probe_error.is_some())
    {
        recommendations.push(tool_recommendation(
            tool,
            stacks,
            "The executable path was found, but its fixed version check did not succeed.",
            ProjectEnvironmentRecommendationSeverity::Warning,
        ));
    }
    if !version_probe_approved
        && tools
            .iter()
            .any(|tool| tool.required && tool.canonical_path.is_some())
    {
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "environment:versions-skipped".into(),
            title: "Version checks were skipped".into(),
            summary: "Detected paths are shown as unverified. Inspect again when you want to approve the bounded version commands.".into(),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Optional,
            evidence: Vec::new(),
            run_profile_id: None,
        });
    }
    if stacks.is_empty() {
        recommendations.push(ProjectEnvironmentRecommendation {
            id: "environment:no-stack".into(),
            title: "No runnable stack was identified".into(),
            summary: "Aone found no supported language summary or bounded run manifest. No environment configuration was changed.".into(),
            kind: ProjectEnvironmentRecommendationKind::Environment,
            severity: ProjectEnvironmentRecommendationSeverity::Optional,
            evidence: Vec::new(),
            run_profile_id: None,
        });
    }
    recommendations
}

fn stack_evidence(stacks: &[ProjectStack]) -> Vec<ProjectEnvironmentEvidence> {
    stacks
        .iter()
        .flat_map(|stack| stack.evidence.iter().take(2).cloned())
        .take(8)
        .collect()
}

fn tool_recommendation(
    tool: &ProjectTool,
    stacks: &[ProjectStack],
    summary: &str,
    severity: ProjectEnvironmentRecommendationSeverity,
) -> ProjectEnvironmentRecommendation {
    let evidence = stacks
        .iter()
        .filter(|stack| tool.used_by.contains(&stack.id))
        .flat_map(|stack| stack.evidence.iter().take(2).cloned())
        .take(6)
        .collect();
    ProjectEnvironmentRecommendation {
        id: format!("tool:{}", tool.id),
        title: format!("Review {}", tool.label),
        summary: summary.into(),
        kind: ProjectEnvironmentRecommendationKind::Tool,
        severity,
        evidence,
        run_profile_id: None,
    }
}

fn preferred_profile(profiles: &[RunProfile]) -> Option<&RunProfile> {
    profiles.iter().min_by_key(|profile| {
        let name = profile.name.to_ascii_lowercase();
        let priority = if name.contains(" dev") || name.ends_with("dev") {
            0
        } else if name.contains("start") {
            1
        } else if name.contains("serve") || name.contains("run") {
            2
        } else {
            3
        };
        (priority, &profile.source, &profile.name, &profile.id)
    })
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
