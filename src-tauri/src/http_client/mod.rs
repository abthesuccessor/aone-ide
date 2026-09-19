mod confirmation;
mod destination;
mod request;
mod response;
#[cfg(test)]
mod tests;

pub use request::send_api_request;
pub(crate) use request::{__cmd__send_api_request, __tauri_command_name_send_api_request};
