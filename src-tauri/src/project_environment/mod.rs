mod analysis;
mod catalog;
mod command;
mod config_hints;
mod confirmation;
mod discovery;
mod hint_parsing;
mod identity;
mod markers;
mod probe;
mod recommendations;
mod state;
mod time;

#[cfg(test)]
mod tests;

pub(crate) use command::{
    __cmd__get_project_environment_report, __cmd__inspect_project_environment,
    __tauri_command_name_get_project_environment_report,
    __tauri_command_name_inspect_project_environment,
};
pub use command::{get_project_environment_report, inspect_project_environment};
pub(crate) use config_hints::detected_environment_names;
pub use state::ProjectEnvironmentState;
