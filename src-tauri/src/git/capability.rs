//! Git version capability checks.
//!
//! Aone deliberately runs the fixed `/usr/bin/git` rather than searching `PATH`.
//! On macOS that is whatever Git the active developer tools ship, which lags
//! upstream considerably — Xcode 16.4 still provides Git 2.39.5. Options Aone
//! relies on for isolation are newer than that, and Git rejects an unknown
//! top-level option outright, so an unsupported build must be reported rather
//! than worked around silently.

use std::{process::Stdio, sync::OnceLock, time::Duration};

use tokio::process::Command;

use crate::error::{AoneError, AoneResult};

/// `--attr-source` arrived in Git 2.40, as did its `GIT_ATTR_SOURCE`
/// equivalent. It is how repository inspection prevents a repository's own
/// `.gitattributes` from installing a diff, textconv, or filter driver, so
/// there is no older fallback that preserves the guarantee.
pub(super) const MINIMUM_ATTR_SOURCE_VERSION: (u64, u64, u64) = (2, 40, 0);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

static DETECTED_VERSION: OnceLock<Option<(u64, u64, u64)>> = OnceLock::new();

/// Fails unless the fixed Git can isolate attribute lookup. Returning an error
/// keeps the guarantee intact: Aone declines to inspect rather than inspect
/// with repository-controlled attribute drivers in play.
pub(super) async fn require_attribute_isolation(git: &std::path::Path) -> AoneResult<()> {
    let version = detect_version(git).await;
    match version {
        Some(version) if version >= MINIMUM_ATTR_SOURCE_VERSION => Ok(()),
        Some((major, minor, patch)) => {
            let (want_major, want_minor, want_patch) = MINIMUM_ATTR_SOURCE_VERSION;
            Err(AoneError::InvalidRequest(format!(
                "repository inspection needs Git {want_major}.{want_minor}.{want_patch} or newer, \
                 but the fixed /usr/bin/git is {major}.{minor}.{patch}. Install a newer Git \
                 (for example with Homebrew) and point the developer tools at it."
            )))
        }
        None => Err(AoneError::InvalidRequest(
            "the version of the fixed /usr/bin/git could not be determined".into(),
        )),
    }
}

async fn detect_version(git: &std::path::Path) -> Option<(u64, u64, u64)> {
    if let Some(cached) = DETECTED_VERSION.get() {
        return *cached;
    }
    let detected = read_version(git).await;
    // A concurrent caller may win the race; either value is the same reading.
    let _ = DETECTED_VERSION.set(detected);
    detected
}

async fn read_version(git: &std::path::Path) -> Option<(u64, u64, u64)> {
    let mut command = Command::new(git);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    scrub_and_isolate(&mut command);
    let output = tokio::time::timeout(VERSION_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_version(&output.stdout)
}

fn scrub_and_isolate(command: &mut Command) {
    super::environment::scrub_git_environment(command);
    super::environment::isolate_configuration(command);
    command.env("GIT_TERMINAL_PROMPT", "0").env("TERM", "dumb");
}

/// `git --version` prints `git version 2.39.5 (Apple Git-154)`. Only the
/// leading numeric triple is trusted; vendor suffixes are ignored.
fn parse_version(bytes: &[u8]) -> Option<(u64, u64, u64)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let field = text.split_whitespace().nth(2)?;
    let mut parts = field.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    // Apple and Windows builds append vendor components; a missing or
    // non-numeric patch is treated as zero rather than rejected.
    let patch = parts
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::{MINIMUM_ATTR_SOURCE_VERSION, parse_version};

    #[test]
    fn version_parser_reads_upstream_and_vendor_builds() {
        assert_eq!(
            parse_version(b"git version 2.39.5 (Apple Git-154)\n"),
            Some((2, 39, 5))
        );
        assert_eq!(parse_version(b"git version 2.50.1\n"), Some((2, 50, 1)));
        assert_eq!(
            parse_version(b"git version 2.45.2.windows.1\n"),
            Some((2, 45, 2))
        );
        // A missing patch component is zero, not a parse failure.
        assert_eq!(parse_version(b"git version 2.40\n"), Some((2, 40, 0)));
        for rejected in [&b""[..], &b"git\n"[..], &b"git version x.y.z\n"[..]] {
            assert_eq!(parse_version(rejected), None);
        }
    }

    #[test]
    fn the_attribute_isolation_floor_matches_the_option_that_needs_it() {
        // --attr-source landed in Git 2.40; anything earlier cannot isolate
        // repository-controlled attributes at all.
        assert_eq!(MINIMUM_ATTR_SOURCE_VERSION, (2, 40, 0));
        assert!((2, 39, 5) < MINIMUM_ATTR_SOURCE_VERSION);
        assert!((2, 40, 0) >= MINIMUM_ATTR_SOURCE_VERSION);
    }
}
