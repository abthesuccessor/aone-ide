mod cli_adapter;
mod command;
mod configuration;
mod evidence;
mod graph_projection;
mod project_command;
mod project_evidence;
mod provider;
mod state;
mod status;
#[cfg(test)]
mod tests;

pub use command::ai_explain;
pub(crate) use command::{__cmd__ai_explain, __tauri_command_name_ai_explain};
pub(crate) use configuration::{
    __cmd__configure_ai_cli, __cmd__configure_hosted_ai, __cmd__configure_ollama,
    __cmd__list_ai_cli_adapters, __tauri_command_name_configure_ai_cli,
    __tauri_command_name_configure_hosted_ai, __tauri_command_name_configure_ollama,
    __tauri_command_name_list_ai_cli_adapters,
};
pub use configuration::{
    configure_ai_cli, configure_hosted_ai, configure_ollama, list_ai_cli_adapters,
};
pub(crate) use project_command::{
    __cmd__ai_ask_project_agent, __cmd__ai_explain_project_environment,
    __tauri_command_name_ai_ask_project_agent, __tauri_command_name_ai_explain_project_environment,
};
pub use project_command::{ai_ask_project_agent, ai_explain_project_environment};
pub use state::SecretState;
pub use status::get_ai_configuration_status;
pub(crate) use status::{
    __cmd__get_ai_configuration_status, __tauri_command_name_get_ai_configuration_status,
};
