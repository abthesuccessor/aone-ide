use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::{
    domain::ProjectEnvironmentEvidence,
    error::AoneResult,
    scanner::{canonicalize_relative_file, read_source_file_bounded},
    store::GraphStore,
};

use super::hint_parsing::{
    dependency_is_postgresql, dependency_is_redis, extract_container_environment_names,
    extract_explicit_environment_names, extract_python_settings_names, package_json_dependencies,
    requirements_dependencies, toml_dependencies,
};

const MAX_RESULTS_PER_QUERY: usize = 40;
pub(super) const MAX_HINT_FILES: usize = 24;
pub(super) const MAX_HINT_FILE_BYTES: u64 = 64 * 1024;
pub(super) const MAX_HINT_TOTAL_BYTES: u64 = 512 * 1024;
const MAX_ENVIRONMENT_NAMES: usize = 32;

#[derive(Debug, Clone, Default)]
pub(super) struct ProjectHints {
    pub(super) environment_names: Vec<String>,
    pub(super) environment_evidence: Vec<ProjectEnvironmentEvidence>,
    pub(super) runtime_dependencies: Vec<RuntimeDependencyHint>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeDependencyHint {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) evidence: Vec<ProjectEnvironmentEvidence>,
}

#[derive(Clone, Copy)]
enum SourceKind {
    PythonConfig,
    JavaScriptConfig,
    PackageJson,
    TomlManifest,
    Requirements,
    Container,
}

#[derive(Clone, Copy)]
struct SourceRule {
    basename: &'static str,
    kind: SourceKind,
}

const SOURCE_RULES: &[SourceRule] = &[
    source("pyproject.toml", SourceKind::TomlManifest),
    source("Cargo.toml", SourceKind::TomlManifest),
    source("requirements.txt", SourceKind::Requirements),
    source("package.json", SourceKind::PackageJson),
    source("config.py", SourceKind::PythonConfig),
    source("settings.py", SourceKind::PythonConfig),
    source("config.ts", SourceKind::JavaScriptConfig),
    source("config.js", SourceKind::JavaScriptConfig),
    source("config.mjs", SourceKind::JavaScriptConfig),
    source("config.cjs", SourceKind::JavaScriptConfig),
    source("Dockerfile", SourceKind::Container),
    source("compose.yaml", SourceKind::Container),
    source("compose.yml", SourceKind::Container),
    source("docker-compose.yaml", SourceKind::Container),
    source("docker-compose.yml", SourceKind::Container),
];

const fn source(basename: &'static str, kind: SourceKind) -> SourceRule {
    SourceRule { basename, kind }
}

#[derive(Default)]
struct HintCollector {
    environment_names: BTreeSet<String>,
    environment_paths: BTreeSet<String>,
    runtime_paths: BTreeMap<&'static str, BTreeSet<String>>,
}

pub(super) fn collect_project_hints(root: &Path, store: &GraphStore) -> AoneResult<ProjectHints> {
    let mut collector = HintCollector::default();
    let mut seen = BTreeSet::new();
    let mut files_read = 0_usize;
    let mut bytes_read = 0_u64;

    'rules: for rule in SOURCE_RULES {
        for file in store.list_files(Some(rule.basename), MAX_RESULTS_PER_QUERY)? {
            if files_read == MAX_HINT_FILES || bytes_read == MAX_HINT_TOTAL_BYTES {
                break 'rules;
            }
            let relative = Path::new(&file.relative_path);
            if relative.file_name().and_then(|name| name.to_str()) != Some(rule.basename)
                || !seen.insert(file.relative_path.clone())
                || file.size_bytes > MAX_HINT_FILE_BYTES
            {
                continue;
            }
            let remaining = MAX_HINT_TOTAL_BYTES - bytes_read;
            let read_limit = remaining.min(MAX_HINT_FILE_BYTES);
            let Some(contents) = read_hint_source(root, relative, read_limit) else {
                continue;
            };
            bytes_read += contents.len() as u64;
            files_read += 1;
            collector.inspect(rule.kind, &file.relative_path, &contents);
        }
    }
    Ok(collector.finish())
}

fn read_hint_source(root: &Path, relative: &Path, limit: u64) -> Option<String> {
    let canonical = canonicalize_relative_file(root, relative).ok()?;
    read_source_file_bounded(&canonical, limit).ok()
}

impl HintCollector {
    fn inspect(&mut self, kind: SourceKind, relative_path: &str, contents: &str) {
        let mut environment_names = extract_explicit_environment_names(contents);
        if matches!(kind, SourceKind::PythonConfig) {
            environment_names.extend(extract_python_settings_names(contents));
        }
        if matches!(kind, SourceKind::Container) {
            environment_names.extend(extract_container_environment_names(contents));
        }
        if !environment_names.is_empty() {
            self.environment_paths.insert(relative_path.into());
            self.environment_names.extend(environment_names);
        }

        let dependencies = match kind {
            SourceKind::PackageJson => package_json_dependencies(contents),
            SourceKind::TomlManifest => toml_dependencies(contents),
            SourceKind::Requirements => requirements_dependencies(contents),
            _ => BTreeSet::new(),
        };
        for dependency in &dependencies {
            if dependency_is_postgresql(dependency) {
                self.runtime_paths
                    .entry("postgresql")
                    .or_default()
                    .insert(relative_path.into());
            }
            if dependency_is_redis(dependency) {
                self.runtime_paths
                    .entry("redis")
                    .or_default()
                    .insert(relative_path.into());
            }
        }

        let lower = contents.to_ascii_lowercase();
        if lower.contains("postgresql://") || lower.contains("postgresql+") {
            self.runtime_paths
                .entry("postgresql")
                .or_default()
                .insert(relative_path.into());
        }
        if lower.contains("redis://") || lower.contains("rediss://") {
            self.runtime_paths
                .entry("redis")
                .or_default()
                .insert(relative_path.into());
        }
    }

    fn finish(self) -> ProjectHints {
        let environment_names = self
            .environment_names
            .into_iter()
            .take(MAX_ENVIRONMENT_NAMES)
            .collect::<Vec<_>>();
        let environment_evidence = if environment_names.is_empty() {
            Vec::new()
        } else {
            self.environment_paths
                .into_iter()
                .take(8)
                .map(|relative_path| ProjectEnvironmentEvidence {
                    relative_path,
                    detail: "Names-only hint inferred from recognized configuration syntax; no value is retained.".into(),
                })
                .collect()
        };
        let runtime_dependencies = [
            ("postgresql", "PostgreSQL"),
            ("redis", "Redis"),
        ]
        .into_iter()
        .filter_map(|(id, label)| {
            let paths = self.runtime_paths.get(id)?;
            Some(RuntimeDependencyHint {
                id,
                label,
                evidence: paths
                    .iter()
                    .take(8)
                    .map(|relative_path| ProjectEnvironmentEvidence {
                        relative_path: relative_path.clone(),
                        detail: format!(
                            "Bounded dependency/config evidence suggests a {label} client; this is not a required-service or readiness claim."
                        ),
                    })
                    .collect(),
            })
        })
        .collect();
        ProjectHints {
            environment_names,
            environment_evidence,
            runtime_dependencies,
        }
    }
}

#[cfg(test)]
pub(super) fn hint_source_limits_for_test() -> (usize, u64, u64) {
    (MAX_HINT_FILES, MAX_HINT_FILE_BYTES, MAX_HINT_TOTAL_BYTES)
}
