use application::{
    download_history_service::DownloadHistoryService,
    download_transfer_service::DownloadTransferService, settings_service::SettingsService,
};
use infrastructure::{download_history_store::DownloadHistoryStore, settings_store::SettingsStore};
use presentation::{
    commands::{add_download, snapshot, update_settings, AppState, TauriDownloadEventPublisher},
    window_manager::{close_auxiliary_window, open_progress_window, resize_progress_window},
};
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
                default_download_directory.clone(),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let max_concurrent = settings.settings().max_concurrent;
            let download_history = DownloadHistoryService::with_events(
                DownloadHistoryStore::new(home_directory.join(".awesome-downloader")),
                std::sync::Arc::new(TauriDownloadEventPublisher::new(app.handle().clone())),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let download_transfer =
                DownloadTransferService::new(download_history.clone(), max_concurrent)
                    .map_err(|error| std::io::Error::other(error.message))?;

            app.manage(AppState::new(settings, download_history, download_transfer));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            add_download,
            update_settings,
            open_progress_window,
            close_auxiliary_window,
            resize_progress_window
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Awesome Downloader");
}
