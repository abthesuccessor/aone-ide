use std::fs;

use tempfile::tempdir;

use super::{loader::load_attested_env_values, parser::parse_env};

#[test]
fn parser_accepts_quotes_comments_and_export_without_expanding_values() {
    let values = parse_env(
        "export API_URL=https://localhost # local\nTOKEN='literal $VALUE'\nNAME=\"a\\nline\"\n",
    )
    .unwrap();
    assert_eq!(values["API_URL"], "https://localhost");
    assert_eq!(values["TOKEN"], "literal $VALUE");
    assert_eq!(values["NAME"], "a\nline");
}

#[test]
fn parser_accepts_utf8_bom_crlf_and_identical_duplicates() {
    let values =
        parse_env("\u{feff}OPENAI_API_KEY=backend-only\r\nOPENAI_API_KEY=backend-only\r\n")
            .unwrap();

    assert_eq!(values["OPENAI_API_KEY"], "backend-only");
}

#[test]
fn parser_rejects_conflicting_duplicate_values_without_echoing_them() {
    let error = parse_env("OPENAI_API_KEY=first\nOPENAI_API_KEY=second\n")
        .unwrap_err()
        .to_string();

    assert!(error.contains("conflicting duplicate environment variable OPENAI_API_KEY"));
    assert!(!error.contains("first"));
    assert!(!error.contains("second"));
}

#[test]
fn native_dialog_attested_env_can_be_outside_workspace() {
    let selected = tempdir().unwrap();
    let path = selected.path().join(".env.local");
    fs::write(&path, "OPENAI_API_KEY=backend-only\nPORT=3000\n").unwrap();
    let values = load_attested_env_values(&path).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values["OPENAI_API_KEY"], "backend-only");
}

#[test]
fn native_dialog_accepts_an_explicit_local_runtime_env_file() {
    let selected = tempdir().unwrap();
    let path = selected.path().join("local-runtime.env");
    fs::write(&path, "OPENAI_API_KEY=backend-only\n").unwrap();

    let values = load_attested_env_values(&path).unwrap();

    assert_eq!(values["OPENAI_API_KEY"], "backend-only");
}

#[cfg(unix)]
#[test]
fn native_dialog_attested_env_rejects_a_symlink() {
    use std::os::unix::fs::symlink;

    let selected = tempdir().unwrap();
    let target = selected.path().join(".env.real");
    let link = selected.path().join(".env.local");
    fs::write(&target, "OPENAI_API_KEY=backend-only\n").unwrap();
    symlink(&target, &link).unwrap();

    let error = load_attested_env_values(&link).unwrap_err();
    assert!(error.to_string().contains("non-symlink"));
}
