use std::{fs, path::Path, time::Duration};

use tempfile::tempdir;

use crate::{error::AoneError, store::GraphStore};

use super::{
    canonicalize_relative_file, is_discoverable_file, is_hard_denied,
    limits::{DEFAULT_SCAN_LIMITS, ScanLimits},
    scan::{analyze_file, scan_workspace_with_limits},
    scan_workspace,
    secure_read::open_verified_regular_file,
};

#[test]
fn hard_denies_secrets_dependencies_and_build_output() {
    for path in [
        ".env",
        ".env.local",
        ".env.example",
        "private.pem",
        "node_modules/pkg/index.js",
        "target/debug/app",
        ".git/config",
        "credentials.json",
        ".docker/config.json",
        ".kube/config",
        ".aws/credentials",
        ".azure/accessTokens.json",
        ".ssh/id_ed25519.pub",
        ".gnupg/private-keys-v1.d/key.key",
        ".git-credentials",
        ".config/gcloud/credentials.db",
        "application_default_credentials.json",
        ".cargo/credentials.toml",
        ".terraform/terraform.tfstate",
        "infra/terraform.tfstate.backup",
        "infra/dev.tfstate",
    ] {
        assert!(is_hard_denied(Path::new(path)), "{path}");
    }
    for path in [
        "src/environment.ts",
        "src/docker/client.rs",
        "src/aws.rs",
        "src/credentials_service.rs",
        "src/terraform_state.rs",
        "docs/kube-architecture.md",
    ] {
        assert!(!is_hard_denied(Path::new(path)), "{path}");
    }
}

#[test]
fn canonicalization_rejects_escape_and_symlink_escape() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), "secret").unwrap();
    assert!(matches!(
        canonicalize_relative_file(root.path(), Path::new("../secret.txt")),
        Err(AoneError::PathEscape)
    ));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), root.path().join("link"))
            .unwrap();
        assert!(matches!(
            canonicalize_relative_file(root.path(), Path::new("link")),
            Err(AoneError::PathEscape)
        ));
    }
}

#[cfg(unix)]
#[test]
fn secure_file_open_rejects_final_symlinks_and_identity_swaps() {
    use std::os::unix::fs::symlink;

    let root = tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    let source = root_path.join("source.rs");
    let alias = root_path.join("alias.rs");
    fs::write(&source, "fn source() {}\n").unwrap();
    symlink(&source, &alias).unwrap();

    assert!(matches!(
        canonicalize_relative_file(&root_path, Path::new("alias.rs")),
        Err(AoneError::SensitivePath)
    ));
    assert!(matches!(
        analyze_file(&root_path, "workspace", &alias),
        Err(AoneError::SensitivePath)
    ));
    assert!(!is_discoverable_file(root.path(), &alias));

    let before = source.symlink_metadata().unwrap();
    let canonical = source.metadata().unwrap();
    let replaced = root_path.join("replaced.rs");
    fs::rename(&source, &replaced).unwrap();
    fs::write(&source, "fn attacker() {}\n").unwrap();
    let error = open_verified_regular_file(&source, &before, &canonical)
        .err()
        .unwrap();
    assert!(error.to_string().contains("changed during secure open"));
}

#[test]
fn indexed_size_and_hash_describe_bytes_read_from_the_open_file() {
    let root = tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    let path = root_path.join("lib.rs");
    let source = "fn measured() { work(); }\n";
    fs::write(&path, source).unwrap();

    let indexed = analyze_file(&root_path, "workspace", &path)
        .unwrap()
        .unwrap();
    assert_eq!(indexed.file.size_bytes, source.len() as u64);
    assert_eq!(
        indexed.file.content_hash,
        blake3::hash(source.as_bytes()).to_hex().to_string()
    );
}

#[test]
fn scanner_honors_gitignore_and_secret_rules() {
    let root = tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::create_dir(root.path().join("node_modules")).unwrap();
    fs::write(root.path().join("src/lib.rs"), "fn main() {}\n").unwrap();
    fs::write(root.path().join("ignored.rs"), "fn hidden() {}\n").unwrap();
    fs::write(root.path().join(".gitignore"), "ignored.rs\n").unwrap();
    fs::write(root.path().join(".env.example"), "OPENAI_API_KEY=x\n").unwrap();
    fs::write(root.path().join("node_modules/pkg.js"), "export {}\n").unwrap();

    let result = scan_workspace(root.path(), "workspace", |_| {}).unwrap();
    let paths = result
        .files
        .iter()
        .map(|file| file.file.relative_path.as_str())
        .collect::<Vec<_>>();
    assert!(paths.contains(&"src/lib.rs"));
    assert!(!paths.contains(&"ignored.rs"));
    assert!(!paths.iter().any(|path| path.contains(".env")));
    assert!(!paths.iter().any(|path| path.contains("node_modules")));
}

#[test]
fn scanner_enforces_file_and_byte_budgets_before_retention() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(root.path().join("b.rs"), "fn b() {}\n").unwrap();

    let file_error = scan_workspace_with_limits(
        root.path(),
        "workspace",
        &mut |_| {},
        ScanLimits {
            max_files: 1,
            ..DEFAULT_SCAN_LIMITS
        },
    )
    .unwrap_err();
    assert!(
        file_error
            .to_string()
            .contains("maximum of 1 qualifying files")
    );

    let byte_error = scan_workspace_with_limits(
        root.path(),
        "workspace",
        &mut |_| {},
        ScanLimits {
            max_source_bytes: 1,
            ..DEFAULT_SCAN_LIMITS
        },
    )
    .unwrap_err();
    assert!(
        byte_error
            .to_string()
            .contains("aggregate source byte budget of 1 bytes")
    );
}

#[test]
fn scanner_enforces_aggregate_fact_and_wall_clock_budgets() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();
    fs::write(root.path().join("b.rs"), "fn b() {}\n").unwrap();

    let fact_error = scan_workspace_with_limits(
        root.path(),
        "workspace",
        &mut |_| {},
        ScanLimits {
            max_facts: 1,
            ..DEFAULT_SCAN_LIMITS
        },
    )
    .unwrap_err();
    assert!(
        fact_error
            .to_string()
            .contains("aggregate extracted fact budget of 1 facts")
    );

    let time_error = scan_workspace_with_limits(
        root.path(),
        "workspace",
        &mut |_| {},
        ScanLimits {
            max_duration: Duration::ZERO,
            ..DEFAULT_SCAN_LIMITS
        },
    )
    .unwrap_err();
    assert!(time_error.to_string().contains("wall-clock budget"));
}

#[test]
fn scanner_returns_files_in_deterministic_path_order() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("z.rs"), "fn z() {}\n").unwrap();
    fs::write(root.path().join("a.rs"), "fn a() {}\n").unwrap();

    let result = scan_workspace(root.path(), "workspace", |_| {}).unwrap();
    let paths = result
        .files
        .iter()
        .map(|file| file.file.relative_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["a.rs", "z.rs"]);
}

#[test]
fn repeated_declarations_persist_without_graph_id_collisions() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("repeated.test.tsx"),
        r#"
            test("one", () => {
                const wrapper = ({ children }: Props) => <Provider>{children}</Provider>;
            });
            test("two", () => {
                const wrapper = ({ children }: Props) => <Provider>{children}</Provider>;
            });
        "#,
    )
    .unwrap();
    fs::write(
        root.path().join("extractor.rs"),
        r#"
            impl FromRequestParts<AppState> for AuthUser {
                type Rejection = AppError;
            }
            impl OptionalFromRequestParts<AppState> for AuthUser {
                type Rejection = AppError;
            }
        "#,
    )
    .unwrap();

    let result = scan_workspace(root.path(), "workspace", |_| {}).unwrap();
    let database = tempdir().unwrap();
    let mut store = GraphStore::open(&database.path().join("aone.sqlite")).unwrap();

    store.replace_all(&result.files).unwrap();
    let (files, nodes, edges) = store.counts().unwrap();
    assert_eq!(files, result.files.len());
    assert_eq!(
        nodes,
        result
            .files
            .iter()
            .map(|file| file.analysis.nodes.len())
            .sum::<usize>()
    );
    assert_eq!(
        edges,
        result
            .files
            .iter()
            .map(|file| file.analysis.edges.len())
            .sum::<usize>()
    );
}
