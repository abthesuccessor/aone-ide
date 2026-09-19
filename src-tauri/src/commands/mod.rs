mod api_inventory;
mod environment;
mod graph;
mod profiles;
mod snapshot;
mod workspace;

#[cfg(test)]
mod tests;

pub use api_inventory::list_api_endpoints;
pub(crate) use api_inventory::{
    __cmd__list_api_endpoints, __tauri_command_name_list_api_endpoints,
};
pub(crate) use environment::{
    __cmd__pick_and_load_env_file, __cmd__pick_and_load_run_env_file,
    __tauri_command_name_pick_and_load_env_file, __tauri_command_name_pick_and_load_run_env_file,
};
pub use environment::{pick_and_load_env_file, pick_and_load_run_env_file};
pub use graph::query_graph;
pub(crate) use graph::{__cmd__query_graph, __tauri_command_name_query_graph};
pub use profiles::detect_run_profiles;
pub(crate) use profiles::{__cmd__detect_run_profiles, __tauri_command_name_detect_run_profiles};
pub use snapshot::get_app_snapshot;
pub(crate) use snapshot::{__cmd__get_app_snapshot, __tauri_command_name_get_app_snapshot};
pub(crate) use workspace::open_workspace_at_path;
pub(crate) use workspace::{
    __cmd__get_workspace_files, __cmd__pick_and_open_file, __cmd__pick_and_open_workspace,
    __cmd__read_workspace_file, __cmd__rescan_workspace, __tauri_command_name_get_workspace_files,
    __tauri_command_name_pick_and_open_file, __tauri_command_name_pick_and_open_workspace,
    __tauri_command_name_read_workspace_file, __tauri_command_name_rescan_workspace,
};
pub use workspace::{
    get_workspace_files, pick_and_open_file, pick_and_open_workspace, read_workspace_file,
    rescan_workspace,
};
