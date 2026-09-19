use std::{io::Write as _, process::Command};

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager,
    menu::{Menu, MenuBuilder, MenuEvent, MenuItemBuilder, SubmenuBuilder},
};

const NEW_WINDOW: &str = "aone-new-window";
const NEW_FILE: &str = "aone-new-file";
const OPEN_FOLDER: &str = "aone-open-folder";
const ZOOM_IN: &str = "aone-zoom-in";
const ZOOM_OUT: &str = "aone-zoom-out";
const RESET_ZOOM: &str = "aone-reset-zoom";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MenuAction {
    action: &'static str,
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let new_window = MenuItemBuilder::with_id(NEW_WINDOW, "New Window")
        .accelerator("CmdOrCtrl+Shift+N")
        .build(app)?;
    let new_file = MenuItemBuilder::with_id(NEW_FILE, "New File")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_folder = MenuItemBuilder::with_id(OPEN_FOLDER, "Open Folder...")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let zoom_in = MenuItemBuilder::with_id(ZOOM_IN, "Zoom In")
        .accelerator("CmdOrCtrl+Shift+=")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id(ZOOM_OUT, "Zoom Out")
        .accelerator("CmdOrCtrl+-")
        .build(app)?;
    let reset_zoom = MenuItemBuilder::with_id(RESET_ZOOM, "Actual Size")
        .accelerator("CmdOrCtrl+0")
        .build(app)?;
    let application = SubmenuBuilder::new(app, "Aone IDE")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, "File")
        .items(&[&new_window, &new_file, &open_folder])
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let view = SubmenuBuilder::new(app, "View")
        .items(&[&zoom_in, &zoom_out, &reset_zoom])
        .build()?;
    MenuBuilder::new(app)
        .items(&[&application, &file, &edit, &view])
        .build()
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        NEW_WINDOW => launch_new_process(),
        NEW_FILE => emit_action(app, "newFile"),
        OPEN_FOLDER => emit_action(app, "openFolder"),
        ZOOM_IN => emit_action(app, "zoomIn"),
        ZOOM_OUT => emit_action(app, "zoomOut"),
        RESET_ZOOM => emit_action(app, "resetZoom"),
        _ => {}
    }
}

fn launch_new_process() {
    let result = std::env::current_exe().and_then(|executable| {
        let mut command = Command::new(executable);
        command.stdin(std::process::Stdio::null());
        command.stdout(std::process::Stdio::null());
        command.stderr(std::process::Stdio::null());
        command.spawn().map(|_| ())
    });
    if let Err(error) = result {
        let _ = writeln!(
            std::io::stderr().lock(),
            "Aone IDE could not open a new window process: {error}"
        );
    }
}

fn emit_action(app: &AppHandle, action: &'static str) {
    let window = app
        .webview_windows()
        .into_values()
        .find(|window| window.is_focused().unwrap_or(false))
        .or_else(|| app.get_webview_window("main"));
    if let Some(window) = window {
        let _ = window.emit("aone-menu-action", MenuAction { action });
    }
}
