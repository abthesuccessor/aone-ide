use std::{fs, path::Path};

use tempfile::tempdir;

use super::{
    commands::complete_committed_save,
    external::{formatter_capabilities, resolve_formatter},
    formatting::{BUILTIN_FORMATTER, formatter_plan, normalize_source},
    validation::{
        content_hash, validate_format_options, validate_source_content,
        verify_document_precondition,
    },
    writing::atomic_write_if_unchanged,
};
use crate::{
    domain::{FormatDocumentRequest, SourceFile, WriteWorkspaceFileRequest},
    scanner::scan_workspace,
    state::new_workspace_context,
    store::GraphStore,
};

#[test]
fn editor_requests_use_camel_case_contracts() {
    let write = WriteWorkspaceFileRequest {
        workspace_id: "workspace:test".into(),
        relative_path: "src/main.rs".into(),
        content: "fn main() {}\n".into(),
        expected_content_hash: "a".repeat(64),
    };
    let value = serde_json::to_value(write).unwrap();
    assert_eq!(value["workspaceId"], "workspace:test");
    assert_eq!(value["relativePath"], "src/main.rs");
    assert!(value.get("expectedContentHash").is_some());

    let format = FormatDocumentRequest {
        workspace_id: "workspace:test".into(),
        relative_path: "src/main.rs".into(),
        content: "fn main() {}\n".into(),
        expected_content_hash: "b".repeat(64),
        tab_size: 4,
        insert_spaces: true,
        print_width: 100,
    };
    let value = serde_json::to_value(format).unwrap();
    assert_eq!(value["workspaceId"], "workspace:test");
    assert_eq!(value["tabSize"], 4);
    assert_eq!(value["insertSpaces"], true);
    assert_eq!(value["printWidth"], 100);
}

#[test]
fn safe_normalizer_is_deterministic_and_does_not_reindent() {
    let source = "\tfn main() {  \r\n\t\tprintln!(\"ok\");\t\r\n}\r\n\r\n";
    let expected = "\tfn main() {\n\t\tprintln!(\"ok\");\n}\n";
    assert_eq!(normalize_source(source), expected);
    assert_eq!(normalize_source(expected), expected);
    assert_eq!(normalize_source(""), "\n");
}

#[test]
fn formatter_settings_and_source_bounds_are_enforced() {
    validate_format_options(1, 40).unwrap();
    validate_format_options(16, 500).unwrap();
    assert!(validate_format_options(0, 100).is_err());
    assert!(validate_format_options(4, 39).is_err());
    assert!(validate_source_content("plain UTF-8 ✅").is_ok());
    assert!(validate_source_content("text\0binary").is_err());
    assert!(validate_source_content(&"x".repeat(2 * 1024 * 1024 + 1)).is_err());
}

#[test]
fn formatter_plans_are_backend_owned() {
    let plan = formatter_plan(
        "TypeScript",
        Path::new("/workspace/src/app.ts"),
        2,
        true,
        100,
    )
    .unwrap();
    assert_eq!(plan.executable, "prettier");
    assert!(plan.args.iter().any(|arg| arg == "--stdin-filepath"));
    assert!(plan.args.iter().any(|arg| arg == "--no-use-tabs"));
    assert!(formatter_plan("Text", Path::new("notes.txt"), 4, true, 80).is_none());
    assert!(resolve_formatter("aone-formatter-that-must-not-exist").is_none());
}

#[test]
fn capabilities_always_report_the_honest_builtin_fallback() {
    let capabilities = formatter_capabilities();
    let builtin = capabilities
        .iter()
        .find(|capability| capability.formatter == BUILTIN_FORMATTER)
        .unwrap();
    assert!(builtin.available);
    assert!(!builtin.external);
    let external = capabilities
        .iter()
        .filter(|capability| capability.external)
        .collect::<Vec<_>>();
    assert!(!external.is_empty());
    assert!(external.iter().all(|capability| !capability.available));
}

#[test]
fn precondition_requires_an_indexed_file_and_current_hash() {
    let workspace = tempdir().unwrap();
    fs::create_dir(workspace.path().join("src")).unwrap();
    fs::write(workspace.path().join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(workspace.path().join(".env"), "SECRET=value\n").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let indexed = scan_workspace(&root, "workspace:test", |_| {}).unwrap();
    let database = tempdir().unwrap();
    let mut store = GraphStore::open(&database.path().join("aone.sqlite")).unwrap();
    store.replace_all(&indexed.files).unwrap();
    let context = new_workspace_context("workspace:test".into(), root, store).unwrap();
    let hash = content_hash("fn main() {}\n");

    verify_document_precondition(&context, "src/main.rs", &hash).unwrap();
    assert!(verify_document_precondition(&context, "src/main.rs", &"0".repeat(64)).is_err());
    assert!(verify_document_precondition(&context, ".env", &hash).is_err());
}

#[test]
fn atomic_save_rejects_stale_content_and_preserves_the_file() {
    let workspace = tempdir().unwrap();
    let path = workspace.path().join("main.rs");
    fs::write(&path, "fn old() {}\n").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let stale = content_hash("different\n");

    assert!(atomic_write_if_unchanged(&root, "main.rs", &stale, "fn new() {}\n").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "fn old() {}\n");
}

#[test]
fn atomic_save_replaces_content_and_preserves_permissions() {
    let workspace = tempdir().unwrap();
    let path = workspace.path().join("script.sh");
    fs::write(&path, "echo old\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o744)).unwrap();
    }
    let root = workspace.path().canonicalize().unwrap();
    let outcome = atomic_write_if_unchanged(
        &root,
        "script.sh",
        &content_hash("echo old\n"),
        "echo new\n",
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&outcome.path).unwrap(), "echo new\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(outcome.metadata.permissions().mode() & 0o777, 0o744);
    }
}

#[test]
fn committed_save_stays_successful_when_immediate_reindex_fails() {
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("a.rs"), "fn old() {}\n").unwrap();
    fs::write(workspace.path().join("b.rs"), "fn other() {}\n").unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let scan = scan_workspace(&root, "workspace:test", |_| {}).unwrap();
    let database = tempdir().unwrap();
    let mut store = GraphStore::open(&database.path().join("aone.sqlite")).unwrap();
    store.replace_all(&scan.files).unwrap();
    let context = new_workspace_context("workspace:test".into(), root.clone(), store).unwrap();

    let old_hash = content_hash("fn old() {}\n");
    let new_content = "fn saved() {}\n";
    atomic_write_if_unchanged(&root, "a.rs", &old_hash, new_content).unwrap();

    let mut invalid_index = scan
        .files
        .iter()
        .find(|file| file.file.relative_path == "a.rs")
        .unwrap()
        .clone();
    invalid_index.file.id = scan
        .files
        .iter()
        .find(|file| file.file.relative_path == "b.rs")
        .unwrap()
        .file
        .id
        .clone();
    let saved = SourceFile {
        relative_path: "a.rs".into(),
        language: "Rust".into(),
        content: new_content.into(),
        content_hash: content_hash(new_content),
    };

    let (returned, reindexed) = complete_committed_save(&context, &invalid_index, saved);

    assert!(!reindexed);
    assert_eq!(returned.content, new_content);
    assert_eq!(returned.content_hash, content_hash(new_content));
    assert_eq!(fs::read_to_string(root.join("a.rs")).unwrap(), new_content);
    assert_eq!(
        context
            .store
            .lock()
            .get_file("a.rs")
            .unwrap()
            .unwrap()
            .content_hash,
        old_hash
    );
}

#[cfg(unix)]
#[test]
fn atomic_save_rejects_a_symlinked_parent_directory() {
    use std::os::unix::fs::symlink;

    let workspace = tempdir().unwrap();
    fs::create_dir(workspace.path().join("real")).unwrap();
    fs::write(workspace.path().join("real/main.rs"), "fn old() {}\n").unwrap();
    symlink(
        workspace.path().join("real"),
        workspace.path().join("alias"),
    )
    .unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let result = atomic_write_if_unchanged(
        &root,
        "alias/main.rs",
        &content_hash("fn old() {}\n"),
        "fn new() {}\n",
    );
    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(workspace.path().join("real/main.rs")).unwrap(),
        "fn old() {}\n"
    );
}
