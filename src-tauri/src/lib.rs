mod ai;
mod analyzer;
mod commands;
pub mod domain;
mod editor;
mod environment;
pub mod error;
mod git;
mod graph_algorithms;
mod http_client;
mod native_menu;
mod network_policy;
mod onboarding;
mod otlp;
mod project_environment;
mod run_profiles;
mod runner;
mod scanner;
mod search;
mod state;
mod store;
mod terminal;
mod tool_config;
mod watcher;
mod websocket;

use ai::SecretState;
use onboarding::OnboardingState;
use project_environment::ProjectEnvironmentState;
use runner::RuntimeState;
use state::AppState;
use tauri::Manager;
use terminal::TerminalState;
use websocket::WebSocketState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(native_menu::build)
        .on_menu_event(native_menu::handle)
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            app.manage(AppState::new(app_data_dir.clone())?);
            app.manage(RuntimeState::new(app_data_dir));
            app.manage(SecretState::new());
            app.manage(ProjectEnvironmentState::new());
            app.manage(OnboardingState::new());
            app.manage(WebSocketState::new());
            app.manage(TerminalState::new());
            app.manage(search::SearchState::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_snapshot,
            commands::pick_and_open_workspace,
            commands::pick_and_open_file,
            onboarding::clone_github_repository,
            onboarding::create_documents_project,
            onboarding::inspect_git_onboarding,
            commands::rescan_workspace,
            commands::get_workspace_files,
            commands::read_workspace_file,
            search::search_workspace,
            editor::write_workspace_file,
            editor::pick_and_create_workspace_file,
            editor::get_formatter_capabilities,
            editor::format_document,
            commands::query_graph,
            commands::list_api_endpoints,
            commands::detect_run_profiles,
            project_environment::get_project_environment_report,
            project_environment::inspect_project_environment,
            commands::pick_and_load_env_file,
            commands::pick_and_load_run_env_file,
            runner::start_run,
            runner::stop_run,
            runner::list_runtime_events,
            runner::get_debug_session,
            runner::control_debug_session,
            otlp::start_otlp_receiver,
            otlp::get_otlp_receiver,
            otlp::stop_otlp_receiver,
            otlp::delete_runtime_trace,
            http_client::send_api_request,
            ai::get_ai_configuration_status,
            ai::configure_hosted_ai,
            ai::configure_ollama,
            ai::list_ai_cli_adapters,
            ai::configure_ai_cli,
            ai::ai_explain,
            ai::ai_explain_project_environment,
            ai::ai_ask_project_agent,
            websocket::connect_websocket,
            websocket::send_websocket_message,
            websocket::disconnect_websocket,
            terminal::list_terminal_profiles,
            terminal::open_terminal,
            terminal::write_terminal,
            terminal::resize_terminal,
            terminal::close_terminal,
            git::get_git_status,
            git::get_git_diff,
            git::git_stage,
            git::git_unstage,
            git::git_commit,
            git::git_initialize,
            tool_config::list_tool_configurations,
            tool_config::inspect_tool_configurations,
            tool_config::open_tool_configuration,
        ])
        .run(tauri::generate_context!())
        .expect("Aone IDE failed to start");
}
