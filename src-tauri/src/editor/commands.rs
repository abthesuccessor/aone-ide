use std::{io::Write as _, time::Instant};

use tauri::{AppHandle, Manager, State};

use super::{
    external::{
        ExternalFormatOutcome, confirm_formatter_discovery, formatter_capabilities,
        resolve_formatter, run_external_formatter,
    },
    formatting::{BUILTIN_FORMATTER, formatter_plan, normalize_source},
    validation::{
        content_hash, validate_format_options, validate_source_content,
        verify_document_precondition,
    },
    writing::atomic_write_if_unchanged,
};
use crate::{
    analyzer::{analyze_source_with_deadline, language_support, stable_id},
    domain::{
        FormatDocumentRequest, FormatDocumentResult, FormatterCapability, SourceFile,
        WorkspaceFile, WriteWorkspaceFileRequest,
    },
    error::{AoneError, AoneResult},
    scanner::{IndexedFile, MAX_WORKSPACE_SCAN_DURATION, system_time_string},
    state::AppState,
};

#[tauri::command]
pub async fn write_workspace_file(
    request: WriteWorkspaceFileRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<SourceFile> {
    validate_source_content(&request.content)?;
    let context = workspace_for_request(&state, &request.workspace_id)?;
    let workspace_id = request.workspace_id.clone();
    let commit_workspace_id = workspace_id.clone();
    let (source, reindexed) = tauri::async_runtime::spawn_blocking(move || {
        let _reservation = context.wait_for_analysis();
        verify_document_precondition(
            &context,
            &request.relative_path,
            &request.expected_content_hash,
        )?;
        let hash = content_hash(&request.content);
        let deadline = Instant::now()
            .checked_add(MAX_WORKSPACE_SCAN_DURATION)
            .ok_or_else(|| AoneError::Task("source analysis deadline is unavailable".into()))?;
        let analysis = analyze_source_with_deadline(
            &context.id,
            &request.relative_path,
            &request.content,
            &hash,
            Some(deadline),
        )?;
        app.state::<AppState>()
            .while_workspace_current(&commit_workspace_id, || {
                let outcome = atomic_write_if_unchanged(
                    &context.root,
                    &request.relative_path,
                    &request.expected_content_hash,
                    &request.content,
                )?;
                if let Some(warning) = &outcome.durability_warning {
                    report_durability_warning(&request.relative_path, warning);
                }
                let support = language_support(&outcome.path);
                let indexed = IndexedFile {
                    file: WorkspaceFile {
                        id: stable_id("file", &[&context.id, &request.relative_path]),
                        relative_path: request.relative_path.clone(),
                        language: support.name.into(),
                        capability: support.capability,
                        size_bytes: request.content.len() as u64,
                        content_hash: hash.clone(),
                        modified_at: outcome
                            .metadata
                            .modified()
                            .ok()
                            .map(system_time_string)
                            .unwrap_or_else(|| "unknown".into()),
                        parse_errors: analysis.parse_errors,
                    },
                    source: request.content.clone(),
                    analysis,
                };
                let source = SourceFile {
                    relative_path: request.relative_path,
                    language: support.name.into(),
                    content: request.content,
                    content_hash: hash,
                };
                Ok(complete_committed_save(&context, &indexed, source))
            })
    })
    .await
    .map_err(|error| AoneError::Task(error.to_string()))??;

    if state
        .workspace_if_open()
        .is_some_and(|current| current.id == workspace_id)
        && reindexed
    {
        accept_post_commit_result(
            &source.relative_path,
            "workspace summary refresh",
            state.refresh_summary(&workspace_id),
        );
    }
    Ok(source)
}

#[tauri::command]
pub fn get_formatter_capabilities() -> Vec<FormatterCapability> {
    formatter_capabilities()
}

#[tauri::command]
pub async fn format_document(
    request: FormatDocumentRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> AoneResult<FormatDocumentResult> {
    validate_source_content(&request.content)?;
    validate_format_options(request.tab_size, request.print_width)?;
    let _workspace_operation = state.try_reserve_workspace_operation()?;
    let context = workspace_for_request(&state, &request.workspace_id)?;
    let verified = verify_document_precondition(
        &context,
        &request.relative_path,
        &request.expected_content_hash,
    )?;
    let builtin = normalize_source(&request.content);
    let Some(plan) = formatter_plan(
        &verified.language,
        &verified.path,
        request.tab_size,
        request.insert_spaces,
        request.print_width,
    ) else {
        return Ok(format_result(
            &request.content,
            builtin,
            BUILTIN_FORMATTER,
            false,
        ));
    };
    if !confirm_formatter_discovery(&app, &verified.path, &plan).await? {
        return Ok(format_result(
            &request.content,
            builtin,
            BUILTIN_FORMATTER,
            false,
        ));
    }
    let Some(resolved) = resolve_formatter(plan.executable) else {
        return Ok(format_result(
            &request.content,
            builtin,
            BUILTIN_FORMATTER,
            false,
        ));
    };

    let (outcome, formatted) = run_external_formatter(
        &app,
        &context.root,
        &verified.path,
        &request.content,
        &plan,
        &resolved,
    )
    .await?;
    workspace_for_request(&state, &request.workspace_id)?;
    verify_document_precondition(
        &context,
        &request.relative_path,
        &request.expected_content_hash,
    )?;
    if outcome == ExternalFormatOutcome::Formatted
        && let Some(formatted) = formatted
    {
        return Ok(format_result(
            &request.content,
            formatted,
            plan.formatter,
            true,
        ));
    }
    Ok(format_result(
        &request.content,
        builtin,
        BUILTIN_FORMATTER,
        false,
    ))
}

fn workspace_for_request(
    state: &AppState,
    expected_workspace_id: &str,
) -> AoneResult<crate::state::WorkspaceContext> {
    let context = state.workspace()?;
    if expected_workspace_id.is_empty() || context.id != expected_workspace_id {
        return Err(AoneError::InvalidRequest(
            "workspace changed while the editor operation was running".into(),
        ));
    }
    Ok(context)
}

fn format_result(
    original: &str,
    content: String,
    formatter: &str,
    used_external_tool: bool,
) -> FormatDocumentResult {
    FormatDocumentResult {
        changed: content != original,
        content,
        formatter: formatter.into(),
        used_external_tool,
    }
}

pub(super) fn complete_committed_save(
    context: &crate::state::WorkspaceContext,
    indexed: &IndexedFile,
    source: SourceFile,
) -> (SourceFile, bool) {
    let relative_path = source.relative_path.clone();
    let reindexed = accept_post_commit_result(
        &relative_path,
        "immediate graph reindex",
        context.store.lock().replace_file(indexed),
    );
    (source, reindexed)
}

fn accept_post_commit_result<T>(relative_path: &str, stage: &str, result: AoneResult<T>) -> bool {
    match result {
        Ok(_) => true,
        Err(error) => {
            report_post_commit_issue(relative_path, stage, &error);
            false
        }
    }
}

fn report_post_commit_issue(relative_path: &str, stage: &str, error: &impl std::fmt::Display) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "Aone IDE committed save for {relative_path:?}, but {stage} failed: {error}. The returned content hash matches disk; the workspace watcher or Rescan Workspace can recover derived graph state."
    );
}

fn report_durability_warning(relative_path: &str, error: &str) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "Aone IDE committed save for {relative_path:?}, but parent-directory fsync failed: {error}. The new content is visible on disk, although crash durability could not be confirmed."
    );
}
