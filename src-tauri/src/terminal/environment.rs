use std::{env, ffi::OsString};

use portable_pty::CommandBuilder;

const SAFE_PARENT_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

pub(super) fn isolated_shell_command(shell: &std::path::Path) -> CommandBuilder {
    let mut command = CommandBuilder::new(shell);
    command.env_clear();
    for (name, value) in selected_parent_environment() {
        command.env(name, value);
    }
    command.env("SHELL", shell);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("TERM_PROGRAM", "Aone IDE");
    command.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    command
}

pub(super) fn selected_parent_environment() -> Vec<(&'static str, OsString)> {
    SAFE_PARENT_ENV
        .iter()
        .filter_map(|name| env::var_os(name).map(|value| (*name, value)))
        .collect()
}
