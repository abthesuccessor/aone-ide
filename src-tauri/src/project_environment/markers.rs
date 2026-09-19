use std::{collections::BTreeSet, path::Path};

use crate::{domain::ProjectEnvironmentEvidence, error::AoneResult, store::GraphStore};

const MAX_RESULTS_PER_QUERY: usize = 40;
const MAX_MARKER_EVIDENCE: usize = 80;

#[derive(Debug, Clone)]
pub(super) struct MarkerEvidence {
    pub(super) stack_id: &'static str,
    pub(super) evidence: ProjectEnvironmentEvidence,
}

#[derive(Clone, Copy)]
enum MarkerMatch {
    Basename(&'static str),
    Extension(&'static str),
}

#[derive(Clone, Copy)]
struct MarkerRule {
    query: &'static str,
    stack_id: &'static str,
    marker: MarkerMatch,
}

const MARKER_RULES: &[MarkerRule] = &[
    basename("package.json", "javascript"),
    basename("package-lock.json", "javascript"),
    basename("pnpm-lock.yaml", "javascript"),
    basename("yarn.lock", "javascript"),
    basename("bun.lock", "javascript"),
    basename("bun.lockb", "javascript"),
    basename("pyproject.toml", "python"),
    basename("requirements.txt", "python"),
    basename("Pipfile", "python"),
    basename("Pipfile.lock", "python"),
    basename("poetry.lock", "python"),
    basename("uv.lock", "python"),
    basename("Cargo.toml", "rust"),
    basename("Cargo.lock", "rust"),
    basename("go.mod", "go"),
    basename("go.sum", "go"),
    basename("pom.xml", "jvm"),
    basename("build.gradle", "jvm"),
    basename("build.gradle.kts", "jvm"),
    basename("Gemfile", "ruby"),
    basename("Gemfile.lock", "ruby"),
    basename("composer.json", "php"),
    basename("composer.lock", "php"),
    basename("Package.swift", "swift"),
    basename("Package.resolved", "swift"),
    basename("CMakeLists.txt", "native"),
    basename("Makefile", "native"),
    basename("meson.build", "native"),
    basename("Dockerfile", "container-image"),
    extension(".tf", "tf", "terraform"),
    basename(".terraform.lock.hcl", "terraform"),
    basename("pubspec.yaml", "dart"),
    basename("pubspec.lock", "dart"),
];

const fn basename(query: &'static str, stack_id: &'static str) -> MarkerRule {
    MarkerRule {
        query,
        stack_id,
        marker: MarkerMatch::Basename(query),
    }
}

const fn extension(
    query: &'static str,
    extension: &'static str,
    stack_id: &'static str,
) -> MarkerRule {
    MarkerRule {
        query,
        stack_id,
        marker: MarkerMatch::Extension(extension),
    }
}

pub(super) fn collect_marker_evidence(store: &GraphStore) -> AoneResult<Vec<MarkerEvidence>> {
    let mut evidence = Vec::new();
    let mut seen = BTreeSet::new();
    for rule in MARKER_RULES {
        for file in store.list_files(Some(rule.query), MAX_RESULTS_PER_QUERY)? {
            if evidence.len() == MAX_MARKER_EVIDENCE {
                return Ok(evidence);
            }
            if !marker_matches(&file.relative_path, rule.marker)
                || !safe_relative_path(&file.relative_path)
                || !seen.insert((rule.stack_id, file.relative_path.clone()))
            {
                continue;
            }
            evidence.push(MarkerEvidence {
                stack_id: rule.stack_id,
                evidence: ProjectEnvironmentEvidence {
                    relative_path: file.relative_path,
                    detail: format!("Indexed project marker for {}.", rule.stack_id),
                },
            });
        }
    }
    evidence.sort_by(|left, right| {
        (left.stack_id, &left.evidence.relative_path)
            .cmp(&(right.stack_id, &right.evidence.relative_path))
    });
    Ok(evidence)
}

fn marker_matches(relative_path: &str, marker: MarkerMatch) -> bool {
    let path = Path::new(relative_path);
    match marker {
        MarkerMatch::Basename(expected) => {
            path.file_name().and_then(|name| name.to_str()) == Some(expected)
        }
        MarkerMatch::Extension(expected) => {
            path.extension().and_then(|value| value.to_str()) == Some(expected)
        }
    }
}

fn safe_relative_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 300 && !path.chars().any(char::is_control)
}

#[cfg(test)]
pub(super) fn marker_matches_for_test(
    relative_path: &str,
    extension: bool,
    value: &'static str,
) -> bool {
    marker_matches(
        relative_path,
        if extension {
            MarkerMatch::Extension(value)
        } else {
            MarkerMatch::Basename(value)
        },
    )
}
