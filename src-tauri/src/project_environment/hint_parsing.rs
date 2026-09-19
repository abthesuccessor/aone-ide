use std::collections::BTreeSet;

const MAX_DEPENDENCIES_PER_FILE: usize = 256;

pub(super) fn extract_explicit_environment_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for marker in [
        "os.getenv(",
        "os.environ.get(",
        "os.environ[",
        "std::env::var(",
        "std::env::var_os(",
        "env::var(",
        "Deno.env.get(",
    ] {
        collect_quoted_after(source, marker, &mut names);
    }
    collect_identifier_after(source, "process.env.", &mut names);
    collect_placeholders(source, &mut names);
    names
}

fn collect_quoted_after(source: &str, marker: &str, names: &mut BTreeSet<String>) {
    let mut remainder = source;
    while let Some(index) = remainder.find(marker) {
        remainder = &remainder[index + marker.len()..];
        let trimmed = remainder.trim_start();
        let Some(quote @ ('\'' | '"')) = trimmed.chars().next() else {
            continue;
        };
        let value = &trimmed[quote.len_utf8()..];
        let Some(end) = value.find(quote) else {
            continue;
        };
        insert_environment_name(&value[..end], names);
        remainder = &value[end + quote.len_utf8()..];
    }
}

fn collect_identifier_after(source: &str, marker: &str, names: &mut BTreeSet<String>) {
    let mut remainder = source;
    while let Some(index) = remainder.find(marker) {
        remainder = &remainder[index + marker.len()..];
        let end = remainder
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(remainder.len());
        insert_environment_name(&remainder[..end], names);
        remainder = &remainder[end..];
    }
}

fn collect_placeholders(source: &str, names: &mut BTreeSet<String>) {
    let mut remainder = source;
    while let Some(index) = remainder.find("${") {
        remainder = &remainder[index + 2..];
        let end = remainder.find(['}', ':', '-', '?', '+']).unwrap_or(0);
        if end > 0 {
            insert_environment_name(&remainder[..end], names);
            remainder = &remainder[end..];
        }
    }
}

pub(super) fn extract_python_settings_names(source: &str) -> BTreeSet<String> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut names = BTreeSet::new();
    let mut index = 0_usize;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if !is_settings_class(trimmed) {
            index += 1;
            continue;
        }
        let class_indent = indentation(line);
        let mut prefix = None;
        let mut fields = Vec::new();
        index += 1;
        while index < lines.len() {
            let body_line = lines[index];
            let body = body_line.trim_start();
            if !body.is_empty() && !body.starts_with('#') && indentation(body_line) <= class_indent
            {
                break;
            }
            if body.contains("env_prefix") {
                prefix = extract_env_prefix(body).or(prefix);
            }
            if indentation(body_line) == class_indent + 4
                && let Some(field) = python_settings_field(body)
            {
                fields.push(field);
            }
            index += 1;
        }
        if let Some(prefix) = prefix {
            for field in fields {
                insert_environment_name(
                    &format!("{prefix}{}", field.to_ascii_uppercase()),
                    &mut names,
                );
            }
        }
    }
    names
}

fn is_settings_class(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("class ") else {
        return false;
    };
    let name = rest.split(['(', ':']).next().unwrap_or_default().trim();
    name.ends_with("Settings")
}

fn extract_env_prefix(line: &str) -> Option<String> {
    let remainder = line.split_once("env_prefix")?.1;
    let remainder = remainder.split_once('=')?.1.trim_start();
    let quote @ ('\'' | '"') = remainder.chars().next()? else {
        return None;
    };
    let value = &remainder[quote.len_utf8()..];
    let end = value.find(quote)?;
    let prefix = &value[..end];
    if prefix.len() <= 64
        && !prefix.is_empty()
        && prefix.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        Some(prefix.into())
    } else {
        None
    }
}

fn python_settings_field(line: &str) -> Option<&str> {
    let (candidate, _) = line.split_once(':')?;
    let candidate = candidate.trim();
    if candidate == "model_config"
        || candidate.starts_with('_')
        || !candidate.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return None;
    }
    Some(candidate)
}

pub(super) fn extract_container_environment_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut environment_indent = None;
    for line in source.lines() {
        let trimmed = line.trim();
        let indent = indentation(line);
        if matches!(environment_indent, Some(block) if !trimmed.is_empty() && indent <= block) {
            environment_indent = None;
        }
        if trimmed == "environment:" {
            environment_indent = Some(indent);
            continue;
        }
        if let Some(instruction) = trimmed
            .strip_prefix("ARG ")
            .or_else(|| trimmed.strip_prefix("ENV "))
        {
            let candidate = instruction
                .split(['=', ' ', '\t'])
                .next()
                .unwrap_or_default();
            insert_environment_name(candidate, &mut names);
        }
        if environment_indent.is_some() {
            let candidate = trimmed
                .trim_start_matches("- ")
                .split(['=', ':'])
                .next()
                .unwrap_or_default()
                .trim();
            insert_environment_name(candidate, &mut names);
        }
    }
    names
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_ascii_whitespace())
        .map(|character| if character == '\t' { 4 } else { 1 })
        .sum()
}

fn insert_environment_name(value: &str, names: &mut BTreeSet<String>) {
    if (2..=128).contains(&value.len())
        && value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase() || character == '_')
        && value.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        names.insert(value.into());
    }
}

pub(super) fn package_json_dependencies(source: &str) -> BTreeSet<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return BTreeSet::new();
    };
    [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
    ]
    .into_iter()
    .filter_map(|key| value.get(key)?.as_object())
    .flat_map(|dependencies| dependencies.keys())
    .map(|dependency| dependency.to_ascii_lowercase())
    .take(MAX_DEPENDENCIES_PER_FILE)
    .collect()
}

pub(super) fn toml_dependencies(source: &str) -> BTreeSet<String> {
    let Ok(value) = source.parse::<toml::Value>() else {
        return BTreeSet::new();
    };
    let mut dependencies = BTreeSet::new();
    collect_toml_dependencies(&value, None, &mut dependencies);
    dependencies
        .into_iter()
        .take(MAX_DEPENDENCIES_PER_FILE)
        .collect()
}

fn collect_toml_dependencies(
    value: &toml::Value,
    key: Option<&str>,
    dependencies: &mut BTreeSet<String>,
) {
    if dependencies.len() >= MAX_DEPENDENCIES_PER_FILE {
        return;
    }
    let dependency_section = key.is_some_and(|key| {
        key == "dependencies" || key.ends_with("-dependencies") || key == "dependency-groups"
    });
    match value {
        toml::Value::Table(table) => {
            if dependency_section {
                dependencies.extend(table.keys().map(|name| name.to_ascii_lowercase()));
            }
            for (child_key, child) in table {
                collect_toml_dependencies(child, Some(child_key), dependencies);
            }
        }
        toml::Value::Array(values) if dependency_section => {
            for value in values.iter().filter_map(toml::Value::as_str) {
                if let Some(name) = dependency_name(value) {
                    dependencies.insert(name);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn requirements_dependencies(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(['#', '-']))
        .filter_map(dependency_name)
        .take(MAX_DEPENDENCIES_PER_FILE)
        .collect()
}

fn dependency_name(value: &str) -> Option<String> {
    let end = value
        .find(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
        })
        .unwrap_or(value.len());
    let name = value[..end].trim().to_ascii_lowercase();
    (!name.is_empty()).then_some(name)
}

pub(super) fn dependency_is_postgresql(dependency: &str) -> bool {
    matches!(
        dependency,
        "asyncpg"
            | "pg"
            | "pg8000"
            | "postgres"
            | "postgresql"
            | "psycopg"
            | "psycopg2"
            | "psycopg2-binary"
            | "tokio-postgres"
    )
}

pub(super) fn dependency_is_redis(dependency: &str) -> bool {
    matches!(
        dependency,
        "deadpool-redis" | "ioredis" | "redis" | "redis-py"
    )
}
