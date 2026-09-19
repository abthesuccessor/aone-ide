use std::path::{Component, Path};

use zeroize::Zeroizing;

use super::evidence::{
    BoundedEvidence, MAX_EVIDENCE_INPUT_BYTES, MAX_EVIDENCE_ITEMS, provider_safe_text,
};
use crate::{
    domain::{
        EvidenceKind, EvidenceReference, ProjectEnvironmentRecommendationKind,
        ProjectEnvironmentReport, ProjectToolStatus,
    },
    error::{AoneError, AoneResult},
    runner::RuntimeState,
};

pub(super) const MAX_PROJECT_AGENT_QUESTION_CHARS: usize = 1_200;
pub(super) const MAX_PROJECT_AGENT_QUESTION_BYTES: usize = 4 * 1_024;

pub(super) fn prepare_project_agent_question(
    question: &str,
    redaction_values: &[Zeroizing<String>],
) -> AoneResult<String> {
    let question = question.trim();
    if question.is_empty() {
        return Err(AoneError::InvalidRequest(
            "project agent question is required".into(),
        ));
    }
    if question.chars().count() > MAX_PROJECT_AGENT_QUESTION_CHARS
        || question.len() > MAX_PROJECT_AGENT_QUESTION_BYTES
    {
        return Err(AoneError::InvalidRequest(format!(
            "project agent question must be at most {MAX_PROJECT_AGENT_QUESTION_CHARS} characters and {MAX_PROJECT_AGENT_QUESTION_BYTES} bytes"
        )));
    }
    if question
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(AoneError::InvalidRequest(
            "project agent question contains unsupported control characters".into(),
        ));
    }
    Ok(provider_safe_text(
        question,
        MAX_PROJECT_AGENT_QUESTION_BYTES,
        redaction_values,
    ))
}

pub(super) fn build_project_environment_evidence(
    report: &ProjectEnvironmentReport,
    runtime: &RuntimeState,
) -> AoneResult<BoundedEvidence> {
    let redaction_values = runtime.secret_values_for_redaction();
    let mut input = String::new();
    let mut references = Vec::new();

    for stack in report.stacks.iter().take(8) {
        if references.len() == MAX_EVIDENCE_ITEMS {
            break;
        }
        let id = format!(
            "environment:stack:{}",
            provider_safe_text(&stack.id, 80, &redaction_values)
        );
        let label = provider_safe_text(&stack.label, 160, &redaction_values);
        let evidence = stack
            .evidence
            .iter()
            .take(4)
            .map(|item| {
                format!(
                    "{} ({})",
                    safe_relative_path(&item.relative_path, &redaction_values),
                    provider_safe_text(&item.detail, 240, &redaction_values)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let line = format!(
            "[{id}] declared project stack; label={label}; confidence={:?}; evidence={evidence}\n",
            stack.confidence
        );
        if !append_bounded(&mut input, &line) {
            break;
        }
        references.push(EvidenceReference {
            kind: "projectStack".into(),
            id,
            label,
            evidence: EvidenceKind::Declared,
        });
    }

    for tool in report
        .tools
        .iter()
        .filter(|tool| tool.required || !tool.used_by.is_empty())
        .take(12)
    {
        if references.len() == MAX_EVIDENCE_ITEMS {
            break;
        }
        let id = format!(
            "environment:tool:{}",
            provider_safe_text(&tool.id, 80, &redaction_values)
        );
        let label = provider_safe_text(&tool.label, 160, &redaction_values);
        let version = provider_safe_version(tool.version.as_deref(), &redaction_values)
            .unwrap_or_else(|| "not included".into());
        let used_by = tool
            .used_by
            .iter()
            .take(8)
            .map(|value| provider_safe_text(value, 80, &redaction_values))
            .collect::<Vec<_>>()
            .join(",");
        let line = format!(
            "[{id}] observed tool status; label={label}; category={}; required={}; status={}; version={version}; usedBy={used_by}\n",
            provider_safe_text(&tool.category, 80, &redaction_values),
            tool.required,
            tool_status(tool.status),
        );
        if !append_bounded(&mut input, &line) {
            break;
        }
        references.push(EvidenceReference {
            kind: "projectTool".into(),
            id,
            label,
            evidence: EvidenceKind::Observed,
        });
    }

    for recommendation in &report.recommendations {
        if references.len() == MAX_EVIDENCE_ITEMS {
            break;
        }
        let id = format!(
            "environment:recommendation:{}",
            provider_safe_text(&recommendation.id, 100, &redaction_values)
        );
        let label = provider_safe_text(&recommendation.title, 180, &redaction_values);
        let guidance = match recommendation.kind {
            ProjectEnvironmentRecommendationKind::RunProfile => {
                "Use only the registered profile named by this recommendation; executable details are withheld from AI."
                    .into()
            }
            ProjectEnvironmentRecommendationKind::Tool
            | ProjectEnvironmentRecommendationKind::Environment => {
                provider_safe_text(&recommendation.summary, 1_000, &redaction_values)
            }
        };
        let basis = recommendation
            .evidence
            .iter()
            .take(4)
            .map(|item| {
                format!(
                    "{} ({})",
                    safe_relative_path(&item.relative_path, &redaction_values),
                    provider_safe_text(&item.detail, 240, &redaction_values)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let line = format!(
            "[{id}] deterministic setup recommendation; title={label}; kind={:?}; severity={:?}; guidance={guidance}; evidence={basis}\n",
            recommendation.kind, recommendation.severity,
        );
        if !append_bounded(&mut input, &line) {
            break;
        }
        references.push(EvidenceReference {
            kind: "projectRecommendation".into(),
            id,
            label,
            evidence: EvidenceKind::Inferred,
        });
    }

    if references.is_empty() {
        return Err(AoneError::InvalidRequest(
            "inspect the project environment before requesting an AI explanation".into(),
        ));
    }
    Ok(BoundedEvidence { input, references })
}

fn provider_safe_version(
    version: Option<&str>,
    redaction_values: &[Zeroizing<String>],
) -> Option<String> {
    let version = version?.trim();
    if version.is_empty() || version.contains(['/', '\\']) || version.chars().any(char::is_control)
    {
        return None;
    }
    Some(provider_safe_text(version, 240, redaction_values))
}

fn safe_relative_path(value: &str, redaction_values: &[Zeroizing<String>]) -> String {
    if value == "." {
        return "workspace".into();
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return "[relative path hidden]".into();
    }
    provider_safe_text(value, 512, redaction_values)
}

fn tool_status(status: ProjectToolStatus) -> &'static str {
    match status {
        ProjectToolStatus::Available => "available",
        ProjectToolStatus::Missing => "missing",
        ProjectToolStatus::Unverified => "unverified",
    }
}

fn append_bounded(target: &mut String, value: &str) -> bool {
    if target.len().saturating_add(value.len()) > MAX_EVIDENCE_INPUT_BYTES {
        return false;
    }
    target.push_str(value);
    true
}
