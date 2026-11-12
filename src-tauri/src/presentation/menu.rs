// Created: 2026-11-05
// Reason: feat: implement Phase D window management, menus, and text file import/export
// Last updated: 2026-11-06
// Reason: perf: eliminate eager cloning across history queries and apply zero-copy serialization
// Description: Desktop application menu bar structure, accelerator shortcuts, and menu action dispatchers.

use std::path::PathBuf;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    domain::download::DownloadRequest,
    presentation::app_state::AppState,
};

pub const MAX_IMPORT_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024; // 2 MiB

pub fn parse_import_urls(content: &str, file_size: u64) -> Result<Vec<String>, String> {
    if file_size > MAX_IMPORT_FILE_SIZE_BYTES {
        return Err("URL list must be smaller than 2 MiB.".to_string());
    }

    let urls: Vec<String> = content
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    if urls.is_empty() {
        return Err("No valid URLs found in file.".to_string());
    }

    Ok(urls)
}

pub fn format_export_urls(urls: &[String]) -> String {
    urls.join("\r\n")
}

pub fn build_application_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, tauri::Error> {
    // Tasks submenu
    let add_item = MenuItem::with_id(app, "cmd_add", "Add new download…", true, Some("CmdOrCtrl+N"))?;
    let add_batch_item = MenuItem::with_id(app, "cmd_add_batch", "Add batch download…", true, None::<&str>)?;
    let paste_item = MenuItem::with_id(app, "cmd_paste", "Add batch download from clipboard", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    let export_txt = MenuItem::with_id(app, "export_txt", "To text file…", true, None::<&str>)?;
    let export_submenu = Submenu::with_items(app, "Export", true, &[&export_txt])?;

    let import_txt = MenuItem::with_id(app, "import_txt", "From text file…", true, None::<&str>)?;
    let import_submenu = Submenu::with_items(app, "Import", true, &[&import_txt])?;

    let sep2 = PredefinedMenuItem::separator(app)?;
    let exit_item = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;

    let tasks_menu = Submenu::with_items(
        app,
        "Tasks",
        true,
        &[
            &add_item,
            &add_batch_item,
            &paste_item,
            &sep1,
            &export_submenu,
            &import_submenu,
            &sep2,
            &exit_item,
        ],
    )?;

    // File submenu
    let details_item = MenuItem::with_id(app, "cmd_details", "Download properties…", true, None::<&str>)?;
    let remove_item = MenuItem::with_id(app, "cmd_remove", "Remove selected", true, Some("Delete"))?;
    let remove_completed_item = MenuItem::with_id(app, "cmd_remove_completed", "Remove completed downloads", true, None::<&str>)?;

    let file_menu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &details_item,
            &remove_item,
            &remove_completed_item,
        ],
    )?;

    // Downloads submenu
    let pause_all = MenuItem::with_id(app, "cmd_pause_all", "Pause all", true, None::<&str>)?;
    let stop_all = MenuItem::with_id(app, "cmd_stop_all", "Stop all", true, None::<&str>)?;
    let sep_d1 = PredefinedMenuItem::separator(app)?;
    let delete_completed = MenuItem::with_id(app, "cmd_delete_completed", "Delete all completed", true, None::<&str>)?;
    let sep_d2 = PredefinedMenuItem::separator(app)?;
    let find_item = MenuItem::with_id(app, "cmd_find", "Find", true, Some("CmdOrCtrl+F"))?;
    let sep_d3 = PredefinedMenuItem::separator(app)?;
    let scheduler_item = MenuItem::with_id(app, "cmd_scheduler", "Scheduler…", true, None::<&str>)?;
    let start_queue = MenuItem::with_id(app, "cmd_resume_all", "Start queue", true, None::<&str>)?;
    let stop_queue = MenuItem::with_id(app, "cmd_pause_all_queue", "Stop queue", true, None::<&str>)?;
    let speed_limiter = MenuItem::with_id(app, "cmd_settings_speed", "Speed limiter…", true, None::<&str>)?;
    let sep_d4 = PredefinedMenuItem::separator(app)?;
    let options_item = MenuItem::with_id(app, "cmd_settings_options", "Options…", true, None::<&str>)?;

    let downloads_menu = Submenu::with_items(
        app,
        "Downloads",
        true,
        &[
            &pause_all,
            &stop_all,
            &sep_d1,
            &delete_completed,
            &sep_d2,
            &find_item,
            &sep_d3,
            &scheduler_item,
            &start_queue,
            &stop_queue,
            &speed_limiter,
            &sep_d4,
            &options_item,
        ],
    )?;

    // View submenu
    let all_item = MenuItem::with_id(app, "cmd_all", "All downloads", true, None::<&str>)?;
    let torrents_item =
        MenuItem::with_id(app, "cmd_torrents", "Torrents", true, Some("CmdOrCtrl+Shift+T"))?;
    let sep_v1 = PredefinedMenuItem::separator(app)?;
    let fullscreen_item = PredefinedMenuItem::fullscreen(app, None)?;

    let view_menu = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &all_item,
            &torrents_item,
            &sep_v1,
            &fullscreen_item,
        ],
    )?;

    // Help submenu
    let integration_item = MenuItem::with_id(app, "cmd_integration", "Browser integration…", true, None::<&str>)?;
    let check_updates = MenuItem::with_id(app, "check_updates", "Check for updates…", true, None::<&str>)?;
    let sep_h1 = PredefinedMenuItem::separator(app)?;
    let about_item = MenuItem::with_id(app, "about", "About Open Download Manager", true, None::<&str>)?;

    let help_menu = Submenu::with_items(
        app,
        "Help",
        true,
        &[
            &integration_item,
            &check_updates,
            &sep_h1,
            &about_item,
        ],
    )?;

    Menu::with_items(
        app,
        &[
            &tasks_menu,
            &file_menu,
            &downloads_menu,
            &view_menu,
            &help_menu,
        ],
    )
}

pub fn menu_id_to_command(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        "cmd_add" | "cmd_add_batch" => Some("add"),
        "cmd_paste" => Some("paste"),
        "cmd_details" => Some("details"),
        "cmd_remove" => Some("remove"),
        "cmd_remove_completed" | "cmd_delete_completed" => Some("removeCompleted"),
        "cmd_pause_all" | "cmd_stop_all" | "cmd_pause_all_queue" => Some("pauseAll"),
        "cmd_resume_all" => Some("resumeAll"),
        "cmd_find" => Some("find"),
        "cmd_scheduler" => Some("scheduler"),
        "cmd_settings_speed" | "cmd_settings_options" => Some("settings"),
        "cmd_integration" => Some("integration"),
        "cmd_all" => Some("all"),
        _ => None,
    }
}

pub fn handle_menu_event(app: &AppHandle, menu_id: &str) {
    if let Some(cmd) = menu_id_to_command(menu_id) {
        let _ = app.emit("downloader:command", cmd);
        return;
    }

    match menu_id {
        "cmd_torrents" => {
            let _ = crate::presentation::window_manager::open_torrent_window(app.clone());
        }
        "export_txt" => {
            export_downloads_flow(app);
        }
        "import_txt" => {
            import_downloads_flow(app);
        }
        "about" => {
            show_about_dialog(app);
        }
        "check_updates" => {
            show_check_updates_dialog(app);
        }
        "exit" => {
            app.exit(0);
        }
        _ => {}
    }
}

pub fn show_error_dialog(app: &AppHandle, title: &str, message: &str) {
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| {});
}

pub fn show_about_dialog(app: &AppHandle) {
    let version = app.package_info().version.to_string();
    app.dialog()
        .message(format!("Open Download Manager {version}\n\nDownload manager with browser integration. Original implementation inspired by IDM. Full IDM feature parity is in development."))
        .title("About Open Download Manager")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| {});
}

pub fn show_check_updates_dialog(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        let app_clone = app.clone();
        let service = state.update_service.clone();
        tauri::async_runtime::spawn(async move {
            crate::presentation::updater_controller::run_update_check(
                &app_clone,
                &service,
                crate::domain::updater::UpdateCheckMode::Manual,
            )
            .await;
        });
    }
}

pub fn export_downloads_flow(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .set_file_name("downloads.txt")
        .add_filter("URL list", &["txt"])
        .save_file(move |file_path| {
            let Some(file_path) = file_path else {
                return;
            };

            let path: PathBuf = match file_path.into_path() {
                Ok(p) => p,
                Err(error) => {
                    show_error_dialog(&app, "Could not export downloads", &error.to_string());
                    return;
                }
            };

            let urls = if let Some(state) = app.try_state::<AppState>() {
                match state.download_history.with_downloads(|downloads| {
                    downloads.iter().map(|d| d.url.clone()).collect::<Vec<_>>()
                }) {
                    Ok(urls) => urls,
                    Err(error) => {
                        show_error_dialog(&app, "Could not export downloads", &error.message);
                        return;
                    }
                }
            } else {
                return;
            };

            let content = format_export_urls(&urls);
            if let Err(error) = std::fs::write(&path, content) {
                show_error_dialog(&app, "Could not export downloads", &error.to_string());
            }
        });
}

pub fn import_downloads_flow(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("URL list", &["txt"])
        .pick_file(move |file_path| {
            let Some(file_path) = file_path else {
                return;
            };

            let path: PathBuf = match file_path.into_path() {
                Ok(p) => p,
                Err(error) => {
                    show_error_dialog(&app, "Could not import downloads", &error.to_string());
                    return;
                }
            };

            let metadata = match std::fs::metadata(&path) {
                Ok(meta) => meta,
                Err(error) => {
                    show_error_dialog(&app, "Could not import downloads", &error.to_string());
                    return;
                }
            };

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(error) => {
                    show_error_dialog(&app, "Could not import downloads", &error.to_string());
                    return;
                }
            };

            let urls = match parse_import_urls(&content, metadata.len()) {
                Ok(u) => u,
                Err(msg) => {
                    show_error_dialog(&app, "Could not import downloads", &msg);
                    return;
                }
            };

            let count = urls.len();
            if let Some(state) = app.try_state::<AppState>() {
                let settings = match state.settings.lock() {
                    Ok(guard) => guard.settings().clone(),
                    Err(_) => {
                        show_error_dialog(&app, "Could not import downloads", "Application settings are unavailable.");
                        return;
                    }
                };

                let request = DownloadRequest {
                    urls,
                    queue_only: true,
                    ..Default::default()
                };

                if let Err(error) = state.download_transfer.add(request, &settings) {
                    show_error_dialog(&app, "Could not import downloads", &error.message);
                    return;
                }

                app.dialog()
                    .message(format!("Imported {count} downloads.\n\nThe imported downloads are paused. Select Start Queue to begin."))
                    .title("Import downloads")
                    .kind(MessageDialogKind::Info)
                    .buttons(MessageDialogButtons::Ok)
                    .show(|_| {});
            }
        });
}
