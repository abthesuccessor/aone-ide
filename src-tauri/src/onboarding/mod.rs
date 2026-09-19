mod commands;
mod confirmation;
mod inspection;
mod paths;
mod process;
mod state;
mod validation;

#[cfg(test)]
mod tests;

pub(crate) use commands::{
    __cmd__clone_github_repository, __cmd__create_documents_project, __cmd__inspect_git_onboarding,
    __tauri_command_name_clone_github_repository, __tauri_command_name_create_documents_project,
    __tauri_command_name_inspect_git_onboarding,
};
pub use commands::{clone_github_repository, create_documents_project, inspect_git_onboarding};
pub use state::OnboardingState;
