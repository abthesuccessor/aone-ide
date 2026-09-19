use std::collections::HashMap;

use zeroize::Zeroize;

use crate::error::{AoneError, AoneResult};

pub(super) fn parse_env(content: &str) -> AoneResult<HashMap<String, String>> {
    let mut values = HashMap::new();
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((raw_name, raw_value)) = line.split_once('=') else {
            return Err(invalid_after_zeroize(
                &mut values,
                format!("invalid environment assignment on line {}", index + 1),
            ));
        };
        let name = raw_name.trim();
        if validate_name(name).is_err() {
            return Err(invalid_after_zeroize(
                &mut values,
                format!("invalid environment variable name on line {}", index + 1),
            ));
        }
        let mut value = parse_value(raw_value.trim());
        if value.contains('\0') {
            value.zeroize();
            return Err(invalid_after_zeroize(
                &mut values,
                format!(
                    "environment value contains a NUL byte on line {}",
                    index + 1
                ),
            ));
        }
        if let Some(existing) = values.get(name) {
            if existing != &value {
                value.zeroize();
                return Err(invalid_after_zeroize(
                    &mut values,
                    format!(
                        "conflicting duplicate environment variable {name} on line {}",
                        index + 1
                    ),
                ));
            }
            value.zeroize();
            continue;
        }
        values.insert(name.to_owned(), value);
    }
    Ok(values)
}

fn invalid_after_zeroize(values: &mut HashMap<String, String>, message: String) -> AoneError {
    for value in values.values_mut() {
        value.zeroize();
    }
    AoneError::InvalidRequest(message)
}

fn parse_value(raw: &str) -> String {
    if raw.len() >= 2 {
        let first = raw.as_bytes()[0] as char;
        let last = raw.as_bytes()[raw.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            let inner = &raw[1..raw.len() - 1];
            return if first == '"' {
                inner
                    .replace("\\n", "\n")
                    .replace("\\r", "\r")
                    .replace("\\t", "\t")
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\")
            } else {
                inner.to_owned()
            };
        }
    }
    let comment = raw
        .char_indices()
        .find(|(index, character)| {
            *character == '#'
                && raw[..*index]
                    .chars()
                    .last()
                    .is_some_and(char::is_whitespace)
        })
        .map_or(raw.len(), |(index, _)| index);
    raw[..comment].trim_end().to_owned()
}

fn validate_name(name: &str) -> Result<(), ()> {
    let mut characters = name.chars();
    if !characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(());
    }
    Ok(())
}
