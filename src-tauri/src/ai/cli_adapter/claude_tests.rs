//! Claude Code adapter coverage: version floor, the isolation argument
//! surface, the result envelope, readiness, and the process environment.

use std::fs;

use zeroize::Zeroizing;

use super::super::{
    CliAdapterError, CliKind, JSON_SCHEMA, MAX_ANSWER_BYTES,
    claude::{
        execution_arguments as claude_execution_arguments, parse_answer as parse_claude_answer,
        parse_version as parse_claude_version,
    },
    detect_cli, execute_cli_with_limits, probe_cli_readiness,
};
use super::{FakeCodex, limits, quote};

#[test]
fn claude_version_parser_enforces_the_audited_minimum() {
    assert_eq!(
        parse_claude_version(b"2.1.137 (Claude Code)\n").unwrap(),
        "2.1.137"
    );
    // A newer major is accepted; the isolation flags are stable CLI contract.
    assert_eq!(
        parse_claude_version(b"3.0.0 (Claude Code)\n").unwrap(),
        "3.0.0"
    );
    assert!(matches!(
        parse_claude_version(b"2.0.9 (Claude Code)\n"),
        Err(CliAdapterError::UnsupportedVersion(..))
    ));
    for rejected in [
        &b"not-a-version\n"[..],
        &b"2.1\n"[..],
        &b"2.1.3.4\n"[..],
        &b"\n"[..],
        &b"2.1.137\x07 (Claude Code)\n"[..],
    ] {
        assert!(
            parse_claude_version(rejected).is_err(),
            "accepted {rejected:?}"
        );
    }
}

#[test]
fn claude_execution_arguments_disable_every_ambient_capability() {
    let args = claude_execution_arguments();
    let text = args
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    // Non-interactive, machine-readable, schema-constrained.
    assert!(
        text.windows(2)
            .any(|pair| pair == ["--output-format", "json"])
    );
    assert!(text.contains(&"--print".to_string()));
    let schema_index = text.iter().position(|a| a == "--json-schema").unwrap();
    assert_eq!(
        text[schema_index + 1].as_bytes(),
        JSON_SCHEMA,
        "the schema must be the same contract the Codex path writes to disk"
    );

    // Every ambient capability is switched off explicitly, not left at default.
    assert!(text.windows(2).any(|pair| pair == ["--tools", ""]));
    assert!(
        text.windows(2)
            .any(|pair| pair == ["--setting-sources", ""])
    );
    assert!(text.contains(&"--disable-slash-commands".to_string()));
    assert!(text.contains(&"--strict-mcp-config".to_string()));
    assert!(text.contains(&"--no-session-persistence".to_string()));
    assert!(
        text.windows(2)
            .any(|pair| pair == ["--permission-mode", "default"])
    );
    assert!(
        text.windows(2)
            .any(|pair| pair == ["--max-budget-usd", "0.50"])
    );

    // Nothing may re-enable a tool, widen the sandbox, or add a server.
    for forbidden in [
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        "--add-dir",
        "--mcp-config",
        "--plugin-dir",
        "--plugin-url",
        "--allowedTools",
        "--allowed-tools",
        "--chrome",
        "--ide",
        "--agents",
        "--continue",
        "--resume",
    ] {
        assert!(!text.iter().any(|a| a == forbidden), "found {forbidden}");
    }
}

#[test]
fn claude_response_parser_verifies_the_envelope_then_the_payload() {
    // Schema-constrained payload inside a successful envelope.
    assert_eq!(
        parse_claude_answer(
            br#"{"type":"result","subtype":"success","is_error":false,"result":"{\"answer\":\"bounded answer\"}"}"#
        )
        .unwrap(),
        "bounded answer"
    );
    // A plain-text payload is still bounded and still treated as inference.
    assert_eq!(
        parse_claude_answer(br#"{"type":"result","is_error":false,"result":"plain text"}"#)
            .unwrap(),
        "plain text"
    );
    // A failed run must never surface its text as an answer.
    assert!(matches!(
        parse_claude_answer(
            br#"{"type":"result","is_error":true,"result":"Not logged in - run /login"}"#
        ),
        Err(CliAdapterError::InvalidResponse(..))
    ));
    for rejected in [
        &br#"{"type":"error","is_error":false,"result":"x"}"#[..],
        &br#"{"type":"result","is_error":false}"#[..],
        &br#"{"type":"result","is_error":false,"result":"   "}"#[..],
        &b"not json"[..],
    ] {
        assert!(
            parse_claude_answer(rejected).is_err(),
            "accepted {}",
            String::from_utf8_lossy(rejected)
        );
    }
    let oversize = format!(
        r#"{{"type":"result","is_error":false,"result":"{}"}}"#,
        "a".repeat(MAX_ANSWER_BYTES + 1)
    );
    assert!(parse_claude_answer(oversize.as_bytes()).is_err());
}

#[tokio::test]
async fn claude_readiness_requires_both_exit_status_and_logged_in_json() {
    let ready = FakeCodex::of_kind(
        CliKind::Claude,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '2.1.137 (Claude Code)\\n'; exit 0; fi\nif [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ]; then printf '{\"loggedIn\":true}'; exit 0; fi\nexit 9\n",
    );
    assert_eq!(
        probe_cli_readiness(&ready.adapter).await.unwrap().version,
        "2.1.137"
    );

    // Logged out: non-zero exit and loggedIn:false.
    let logged_out = FakeCodex::of_kind(
        CliKind::Claude,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '2.1.137 (Claude Code)\\n'; exit 0; fi\nprintf '{\"loggedIn\":false}'\nexit 1\n",
    );
    assert!(matches!(
        probe_cli_readiness(&logged_out.adapter).await,
        Err(CliAdapterError::AuthenticationRequired(..))
    ));

    // A zero exit status must not override loggedIn:false.
    let inconsistent = FakeCodex::of_kind(
        CliKind::Claude,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '2.1.137 (Claude Code)\\n'; exit 0; fi\nprintf '{\"loggedIn\":false}'\nexit 0\n",
    );
    assert!(matches!(
        probe_cli_readiness(&inconsistent.adapter).await,
        Err(CliAdapterError::AuthenticationRequired(..))
    ));
}

#[tokio::test]
async fn claude_inference_sends_stdin_and_a_minimal_credential_only_environment() {
    let capture = tempfile::tempdir().unwrap();
    let arguments = capture.path().join("arguments");
    let environment = capture.path().join("environment");
    let prompt_file = capture.path().join("prompt");
    let source = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n/usr/bin/env > {}\n/bin/cat > {}\nprintf '%s' '{{\"type\":\"result\",\"is_error\":false,\"result\":\"{{\\\"answer\\\":\\\"bounded answer\\\"}}\"}}'\n",
        quote(&arguments),
        quote(&environment),
        quote(&prompt_file),
    );
    let fake = FakeCodex::of_kind(CliKind::Claude, &source);
    let prompt = "fixed instructions\n\nuntrusted evidence";
    let answer = execute_cli_with_limits(
        &fake.adapter,
        Zeroizing::new(prompt.into()),
        None,
        limits(64 * 1024),
    )
    .await
    .unwrap();
    assert_eq!(answer, "bounded answer");

    // The prompt travels on stdin, never as an argument.
    assert_eq!(fs::read_to_string(&prompt_file).unwrap(), prompt);
    let arguments = fs::read_to_string(&arguments).unwrap();
    assert!(!arguments.contains("untrusted evidence"));
    assert!(arguments.contains("--print"));
    assert!(arguments.contains("--strict-mcp-config"));

    let environment = fs::read_to_string(environment).unwrap();
    // Claude Code needs HOME to reach its own credentials; nothing else about
    // the invoking shell is inherited.
    for allowed in [
        "HOME=",
        "CLAUDE_CONFIG_DIR=",
        "PATH=/usr/bin:/bin",
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
    // Ambient provider configuration must never leak into the child.
    for forbidden in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_",
        "OPENAI_API_KEY",
        "AWS_",
        "SSH_AUTH_SOCK",
    ] {
        assert!(
            !environment.contains(forbidden),
            "leaked {forbidden}: {environment}"
        );
    }
}

#[test]
fn claude_adapter_ids_round_trip_and_reject_unknown_cli_names() {
    assert_eq!(CliKind::from_id("claude"), Some(CliKind::Claude));
    assert_eq!(CliKind::from_id("codex"), Some(CliKind::Codex));
    assert_eq!(CliKind::Claude.id(), "claude");
    assert_eq!(CliKind::Claude.label(), "Claude Code");
    for unknown in ["copilot", "", "CLAUDE", "claude-code", "../codex"] {
        assert_eq!(CliKind::from_id(unknown), None, "accepted {unknown}");
    }
}

/// Probes the CLI actually installed on this machine. Ignored by default
/// because it depends on local state; run it with
/// `cargo test -p aone-ide --lib local_claude -- --ignored --nocapture`
/// to confirm a real install is discovered, allow-listed, and version-checked.
#[tokio::test]
#[ignore = "depends on a local Claude Code installation"]
async fn local_claude_code_installation_is_detected_and_probed() {
    match detect_cli(CliKind::Claude, None) {
        Ok(Some(adapter)) => {
            println!(
                "detected Claude Code at {:?}",
                adapter.identity.canonical_path
            );
            match probe_cli_readiness(&adapter).await {
                Ok(readiness) => println!("READY, version {}", readiness.version),
                Err(error) => println!("detected but not ready: {error}"),
            }
        }
        Ok(None) => println!("no Claude Code installation found in a supported location"),
        Err(error) => println!("rejected: {error}"),
    }
}
