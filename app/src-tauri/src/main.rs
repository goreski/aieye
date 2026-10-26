//! AIEye's desktop app: what AI tools can reach on this device, and what changed
//! since the last scan.

// Release builds are windowed apps without a console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod inventory;

use std::path::PathBuf;

use anyhow::Context as _;
use tauri::menu::{Menu, MenuItem};
use tauri::path::BaseDirectory;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

/// Where the app finds definitions and keeps its data.
#[derive(Clone)]
pub struct Paths {
    pub definitions: PathBuf,
    pub data_dir: PathBuf,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let definitions = app.path().resolve("definitions", BaseDirectory::Resource)?;
            // AIEYE_DATA_DIR points the app at another data folder, for testing.
            let data_dir = match std::env::var_os("AIEYE_DATA_DIR") {
                Some(dir) => PathBuf::from(dir),
                None => aieye_store::default_data_dir()
                    .context("cannot find the local app data folder")?,
            };
            app.manage(Paths {
                definitions,
                data_dir,
            });
            tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::latest,
            commands::scan,
            commands::export
        ])
        .run(tauri::generate_context!())
        .expect("AIEye could not start");
}

/// The tray icon: open the window, scan, or quit.
fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open AIEye", true, None::<&str>)?;
    let scan = MenuItem::with_id(app, "scan", "Scan now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &scan, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("AIEye")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show(app),
            "scan" => {
                show(app);
                // The window runs the scan, so it shows progress and the result.
                let _ = app.emit("scan-requested", ());
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
