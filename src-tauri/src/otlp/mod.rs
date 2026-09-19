mod commands;
mod http;
mod ingest;
mod state;
mod wire;

#[cfg(test)]
mod tests;

pub use commands::{
    delete_runtime_trace, get_otlp_receiver, start_otlp_receiver, stop_otlp_receiver,
};
pub(crate) use state::OtlpReceiverRegistry;

pub(crate) use commands::{
    __cmd__delete_runtime_trace, __cmd__get_otlp_receiver, __cmd__start_otlp_receiver,
    __cmd__stop_otlp_receiver, __tauri_command_name_delete_runtime_trace,
    __tauri_command_name_get_otlp_receiver, __tauri_command_name_start_otlp_receiver,
    __tauri_command_name_stop_otlp_receiver,
};
