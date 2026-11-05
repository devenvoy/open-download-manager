use application::settings_service::SettingsService;
use infrastructure::settings_store::SettingsStore;
use presentation::commands::{snapshot, update_settings, AppState};
use tauri::Manager;

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let home_directory = app
                .path()
                .home_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let default_download_directory = app
                .path()
                .download_dir()
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let data_directory = home_directory.join(".awesome-downloader");
            let settings = SettingsService::new(
                SettingsStore::new(data_directory),
                default_download_directory,
            )
            .map_err(|error| std::io::Error::other(error.message))?;

            app.manage(AppState::new(settings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![snapshot, update_settings])
        .run(tauri::generate_context!())
        .expect("failed to run Awesome Downloader");
}
