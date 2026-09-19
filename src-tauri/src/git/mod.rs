mod capability;
mod commands;
mod confirmation;
mod environment;
mod inspection;
mod paths;
mod process;
mod status;

#[cfg(test)]
mod tests;

pub(crate) use commands::{
    __cmd__get_git_diff, __cmd__get_git_status, __cmd__git_commit, __cmd__git_initialize,
    __cmd__git_stage, __cmd__git_unstage, __tauri_command_name_get_git_diff,
    __tauri_command_name_get_git_status, __tauri_command_name_git_commit,
    __tauri_command_name_git_initialize, __tauri_command_name_git_stage,
    __tauri_command_name_git_unstage,
};
pub use commands::{
    get_git_diff, get_git_status, git_commit, git_initialize, git_stage, git_unstage,
};
