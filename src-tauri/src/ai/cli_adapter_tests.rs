#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt as _, path::Path, time::Duration};

use tempfile::TempDir;
use zeroize::Zeroizing;

use super::{
    CliAdapter, CliAdapterError, CliKind, MAX_PROMPT_BYTES, ProcessLimits,
    codex::{
        DISABLED_FEATURES, execution_arguments, parse_answer as parse_structured_answer,
        parse_version as parse_codex_version,
    },
    execute_cli_with_limits,
    identity::{capture_identity, capture_test_identity},
    probe_cli_readiness, validate_prompt,
};
use crate::ai::{
    provider::{provider_request, request_provider},
    state::{AiProviderKind, ProviderConfiguration, ProviderTransport, SecretState},
};

struct FakeCodex {
    directory: TempDir,
    adapter: CliAdapter,
    executable: std::path::PathBuf,
}

impl FakeCodex {
    fn new(source: &str) -> Self {
        Self::of_kind(CliKind::Codex, source)
    }

    fn of_kind(kind: CliKind, source: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join(kind.id());
        fs::write(&executable, source).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let agent_home = directory.path().join(match kind {
            CliKind::Codex => ".codex",
            CliKind::Claude => ".claude",
        });
        fs::create_dir(&agent_home).unwrap();
        let identity = capture_test_identity(&executable).unwrap();
        let adapter = CliAdapter {
            kind,
            launch_path: executable.clone(),
            identity,
            home: directory.path().to_path_buf(),
            agent_home,
            test_executable: true,
        };
        Self {
            directory,
            adapter,
            executable,
        }
    }
}

/// Limits for tests that assert on a *non-timeout* outcome. The timeout has to
/// stay clear of the behaviour under test: with only two seconds, a loaded
/// machine can hit the deadline before a spinning fixture exceeds its output
/// limit, turning an OutputLimit assertion into a spurious TimedOut. Tests that
/// deliberately assert the timeout set their own short deadline.
fn limits(stdout: usize) -> ProcessLimits {
    ProcessLimits {
        timeout: Duration::from_secs(10),
        stdout,
        stderr: 1024,
    }
}

#[test]
fn version_parser_accepts_only_the_audited_cli_version() {
    assert_eq!(
        parse_codex_version(b"codex-cli 0.144.6\n").unwrap(),
        "0.144.6"
    );
    assert!(matches!(
        parse_codex_version(b"codex-cli 0.144.7\n"),
        Err(CliAdapterError::UnsupportedVersion(..))
    ));
    assert!(matches!(
        parse_codex_version(b"codex-cli 1.2.3-beta.1\n"),
        Err(CliAdapterError::UnsupportedVersion(..))
    ));
    assert!(matches!(
        parse_codex_version(b"codex-cli 0.144.5\n"),
        Err(CliAdapterError::UnsupportedVersion(..))
    ));
    assert!(parse_codex_version(b"other-cli 9.9.9\n").is_err());
}

#[test]
fn execution_arguments_fix_approval_permissions_schema_and_stdin() {
    let args = execution_arguments(Path::new("/private/tmp/aone"), Path::new("/schema.json"));
    let args = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(&args[..3], ["--ask-for-approval", "never", "exec"]);
    assert!(
        args.windows(2)
            .any(|pair| pair == ["-C", "/private/tmp/aone"])
    );
    assert!(args.contains(&"--ephemeral".into()));
    assert!(args.contains(&"--ignore-user-config".into()));
    assert!(args.contains(&"--ignore-rules".into()));
    assert!(args.contains(&"default_permissions=\"aone_bounded\"".into()));
    assert!(args.contains(
        &"permissions.aone_bounded.filesystem={\":minimal\"=\"read\",\":project\"=\"read\"}".into()
    ));
    assert!(args.contains(&"permissions.aone_bounded.network.enabled=false".into()));
    for feature in DISABLED_FEATURES {
        assert!(args.windows(2).any(|pair| pair == ["--disable", *feature]));
    }
    assert_eq!(
        &args[args.len() - 3..],
        ["--output-schema", "/schema.json", "-"]
    );
}

#[tokio::test]
async fn readiness_uses_fixed_version_and_login_status_commands() {
    let fake = FakeCodex::new(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'codex-cli 0.144.6\\n'; exit 0; fi\nif [ \"$1\" = \"login\" ] && [ \"$2\" = \"status\" ]; then printf 'Logged in\\n'; exit 0; fi\nexit 9\n",
    );
    let readiness = probe_cli_readiness(&fake.adapter).await.unwrap();
    assert_eq!(readiness.version, "0.144.6");

    let unauthenticated = FakeCodex::new(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'codex-cli 0.144.6\\n'; exit 0; fi\nexit 1\n",
    );
    assert!(matches!(
        probe_cli_readiness(&unauthenticated.adapter).await,
        Err(CliAdapterError::AuthenticationRequired(..))
    ));
}

#[tokio::test]
async fn inference_uses_stdin_cleared_environment_and_structured_stdout() {
    let capture = tempfile::tempdir().unwrap();
    let arguments = capture.path().join("arguments");
    let environment = capture.path().join("environment");
    let prompt_file = capture.path().join("prompt");
    let source = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n/usr/bin/env > {}\n/bin/cat > {}\nprintf '%s' '{{\"answer\":\"bounded answer\"}}'\n",
        quote(&arguments),
        quote(&environment),
        quote(&prompt_file),
    );
    let fake = FakeCodex::new(&source);
    let prompt = "fixed instructions\n\nuntrusted evidence";
    let answer = execute_cli_with_limits(
        &fake.adapter,
        Zeroizing::new(prompt.into()),
        None,
        limits(16 * 1024),
    )
    .await
    .unwrap();

    assert_eq!(answer, "bounded answer");
    assert_eq!(fs::read_to_string(prompt_file).unwrap(), prompt);
    let args = fs::read_to_string(arguments).unwrap();
    assert!(args.starts_with("--ask-for-approval\nnever\nexec\n"));
    assert!(args.ends_with("-\n"));
    let environment = fs::read_to_string(environment).unwrap();
    for allowed in [
        "CODEX_HOME=",
        "TERM=dumb",
        "NO_COLOR=1",
        "CI=1",
        "LANG=C",
        "LC_ALL=C",
    ] {
        assert!(
            environment.contains(allowed),
            "missing {allowed}: {environment}"
        );
    }
    assert!(!environment.lines().any(|line| line.starts_with("PATH=")));
    assert!(!environment.lines().any(|line| line.starts_with("HOME=")));
}

#[tokio::test]
async fn provider_and_state_integration_use_the_attested_cli_transport() {
    let fake = FakeCodex::new(
        "#!/bin/sh\n/bin/cat >/dev/null\nprintf '%s' '{\"answer\":\"CLI integration works\"}'\n",
    );
    let state = SecretState::new();
    state.install_cli(CliKind::Codex, fake.adapter.clone(), "0.144.6".into());
    let status = state.configuration_status();
    assert_eq!(status.provider.as_deref(), Some("codex"));
    assert_eq!(status.transport.as_deref(), Some("cli"));
    assert!(status.inference_available);

    let configuration = ProviderConfiguration {
        kind: AiProviderKind::Codex,
        model: "codex-cli 0.144.6".into(),
        transport: ProviderTransport::Cli {
            adapter: fake.adapter,
        },
    };
    let request = provider_request(
        AiProviderKind::Codex,
        &configuration.model,
        "[node:one] bounded fact",
    );
    let workspace = tempfile::tempdir().unwrap();
    let answer = request_provider(&configuration, &request, &[], workspace.path())
        .await
        .unwrap();
    assert_eq!(answer, "CLI integration works");
}

#[tokio::test]
async fn executable_identity_is_rechecked_before_inference() {
    let fake = FakeCodex::new("#!/bin/sh\nprintf '%s' '{\"answer\":\"first\"}'\n");
    fs::write(
        &fake.executable,
        "#!/bin/sh\nprintf '%s' '{\"answer\":\"replacement\"}'\n# changed\n",
    )
    .unwrap();
    let result = execute_cli_with_limits(
        &fake.adapter,
        Zeroizing::new("prompt".into()),
        None,
        limits(1024),
    )
    .await;
    assert!(matches!(result, Err(CliAdapterError::IdentityChanged(..))));
}

#[tokio::test]
async fn inference_rejects_an_adapter_inside_the_current_workspace() {
    let fake = FakeCodex::new("#!/bin/sh\nprintf '%s' '{\"answer\":\"unsafe\"}'\n");
    let result = execute_cli_with_limits(
        &fake.adapter,
        Zeroizing::new("prompt".into()),
        Some(fake.directory.path()),
        limits(1024),
    )
    .await;
    assert!(matches!(result, Err(CliAdapterError::UnsafeExecutable(..))));
}

#[tokio::test]
async fn timeout_and_output_limits_kill_untrusted_cli_behavior() {
    let endless = FakeCodex::new("#!/bin/sh\nwhile :; do :; done\n");
    let timed_out = execute_cli_with_limits(
        &endless.adapter,
        Zeroizing::new("prompt".into()),
        None,
        ProcessLimits {
            timeout: Duration::from_millis(80),
            stdout: 1024,
            stderr: 1024,
        },
    )
    .await;
    assert!(matches!(timed_out, Err(CliAdapterError::TimedOut(..))));

    let noisy = FakeCodex::new("#!/bin/sh\nwhile :; do printf '0123456789'; done\n");
    let too_large = execute_cli_with_limits(
        &noisy.adapter,
        Zeroizing::new("prompt".into()),
        None,
        limits(32),
    )
    .await;
    assert!(matches!(too_large, Err(CliAdapterError::OutputLimit(..))));
}

#[test]
fn output_schema_parser_rejects_extra_empty_and_oversize_answers() {
    assert_eq!(
        parse_structured_answer(br#"{"answer":" okay "}"#).unwrap(),
        "okay"
    );
    assert!(parse_structured_answer(br#"{"answer":"x","extra":true}"#).is_err());
    assert!(parse_structured_answer(br#"{"answer":""}"#).is_err());
    let large = format!("{{\"answer\":\"{}\"}}", "x".repeat(12 * 1024 + 1));
    assert!(parse_structured_answer(large.as_bytes()).is_err());
    assert!(validate_prompt(CliKind::Codex, &"x".repeat(MAX_PROMPT_BYTES + 1)).is_err());
}

#[test]
fn scripts_are_not_accepted_as_production_native_adapters() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("codex");
    fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(capture_identity(CliKind::Codex, &executable, directory.path(), None).is_err());
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[path = "cli_adapter/claude_tests.rs"]
mod claude_tests;
