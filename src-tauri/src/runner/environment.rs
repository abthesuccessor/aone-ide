use serde_json::Value;
use zeroize::Zeroizing;

use crate::error::{AoneError, AoneResult};

const MAX_ENV_NAME_BYTES: usize = 128;
pub(super) const MAX_ENV_NAME_REQUESTS: usize = 256;
pub(super) const MAX_SELECTED_ENV_NAMES: usize = 128;

pub(super) const SAFE_PARENT_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "COLORTERM",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

pub fn is_sensitive_name(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "authorization",
        "proxyauthorization",
        "cookie",
        "setcookie",
        "token",
        "apikey",
        "key",
        "password",
        "passwd",
        "secret",
        "credential",
    ]
    .iter()
    .any(|sensitive| normalized.contains(sensitive))
}

pub fn redact_text(text: &str, secret_values: &[Zeroizing<String>]) -> String {
    let mut redacted = text.to_string();
    for secret in secret_values {
        if !secret.is_empty() {
            redacted = redacted.replace(secret.as_str(), "[REDACTED]");
        }
    }

    redacted
        .lines()
        .map(redact_assignment_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn redact_json(value: &Value, secret_values: &[Zeroizing<String>]) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let value = if is_sensitive_name(key) {
                        Value::String("[REDACTED]".into())
                    } else {
                        redact_json(value, secret_values)
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| redact_json(value, secret_values))
                .collect(),
        ),
        Value::String(value) => Value::String(redact_text(value, secret_values)),
        _ => value.clone(),
    }
}

pub(super) fn validate_args(args: &[String]) -> AoneResult<()> {
    if args.len() > 256 {
        return Err(AoneError::InvalidRequest(
            "too many executable arguments".into(),
        ));
    }
    for arg in args {
        if arg.len() > 16 * 1024 || arg.contains('\0') {
            return Err(AoneError::InvalidRequest(
                "invalid executable argument".into(),
            ));
        }
    }
    Ok(())
}

fn validate_env_name(name: &str) -> AoneResult<()> {
    if name.len() > MAX_ENV_NAME_BYTES {
        return Err(AoneError::InvalidRequest(format!(
            "environment variable name exceeds {MAX_ENV_NAME_BYTES} bytes"
        )));
    }
    let mut characters = name.chars();
    let valid_first = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !valid_first
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(AoneError::InvalidRequest(format!(
            "invalid environment variable name: {name}"
        )));
    }
    Ok(())
}

pub(super) fn validate_run_env_name(name: &str) -> AoneResult<()> {
    validate_env_name(name)?;
    if is_process_control_env(name) {
        return Err(AoneError::InvalidRequest(format!(
            "environment variable {name} can alter executable loading or parent process controls and is not allowed in a run profile"
        )));
    }
    Ok(())
}

fn is_process_control_env(name: &str) -> bool {
    SAFE_PARENT_ENV.contains(&name)
        || matches!(
            name,
            "BASH_ENV"
                | "ENV"
                | "ZDOTDIR"
                | "SHELLOPTS"
                | "NODE_OPTIONS"
                | "NODE_PATH"
                | "PYTHONHOME"
                | "PYTHONPATH"
                | "PYTHONSTARTUP"
                | "RUBYOPT"
                | "RUBYLIB"
                | "PERL5OPT"
                | "PERL5LIB"
                | "JAVA_TOOL_OPTIONS"
                | "JDK_JAVA_OPTIONS"
                | "_JAVA_OPTIONS"
                | "CLASSPATH"
                | "DOTNET_STARTUP_HOOKS"
                | "DOTNET_ADDITIONAL_DEPS"
                | "DOTNET_SHARED_STORE"
                | "RUSTC_WRAPPER"
                | "RUSTC_WORKSPACE_WRAPPER"
                | "GIT_SSH"
                | "GIT_SSH_COMMAND"
                | "GIT_ASKPASS"
                | "SSH_ASKPASS"
                | "GOENV"
                | "GOFLAGS"
        )
        || name.starts_with("LD_")
        || name.starts_with("DYLD_")
        || name.starts_with("GIT_CONFIG_")
        || (name.starts_with("CARGO_TARGET_") && name.ends_with("_RUNNER"))
}

pub(super) fn ensure_required_env(required: &[String], loaded: &[String]) -> AoneResult<()> {
    let missing = required
        .iter()
        .filter(|name| !loaded.contains(name))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AoneError::InvalidRequest(format!(
            "required environment variables are not loaded: {}",
            missing.join(", ")
        )))
    }
}

pub(super) fn redact_args(args: &[String], secret_values: &[Zeroizing<String>]) -> Vec<String> {
    let mut redacted = Vec::with_capacity(args.len());
    let mut redact_next = false;
    for arg in args {
        if redact_next {
            redacted.push("[REDACTED]".into());
            redact_next = false;
            continue;
        }

        if let Some((name, _)) = arg.split_once('=')
            && is_sensitive_name(name)
        {
            redacted.push(format!("{name}=[REDACTED]"));
            continue;
        }
        if arg.starts_with('-') && is_sensitive_name(arg.trim_start_matches('-')) {
            redacted.push(arg.clone());
            redact_next = true;
            continue;
        }
        redacted.push(redact_text(arg, secret_values));
    }
    redacted
}

fn redact_assignment_line(line: &str) -> String {
    let separator = line.find(':').or_else(|| line.find('='));
    let Some(index) = separator else {
        return line.to_string();
    };
    let (name, rest) = line.split_at(index);
    if is_sensitive_name(name.trim_matches(|character: char| {
        character.is_whitespace() || matches!(character, '"' | '\'' | '{' | '[')
    })) {
        let separator = rest.chars().next().unwrap_or(':');
        format!("{name}{separator} [REDACTED]")
    } else {
        line.to_string()
    }
}
