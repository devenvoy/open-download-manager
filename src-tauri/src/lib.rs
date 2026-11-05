use application::{
    download_history_service::DownloadHistoryService,
    download_transfer_service::DownloadTransferService, settings_service::SettingsService,
};
use infrastructure::http_download_executor::HttpDownloadExecutor;
use infrastructure::{
    data_file_policy::USER_DATA_DIRECTORY_NAME, download_history_store::DownloadHistoryStore,
    settings_store::SettingsStore,
};
use presentation::{
    app_state::AppState,
    commands::{add_download, download_action, snapshot, update_settings},
    download_event_publisher::TauriDownloadEventPublisher,
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
            let data_directory = home_directory.join(USER_DATA_DIRECTORY_NAME);
            let settings = SettingsService::new(
                SettingsStore::new(data_directory),
                default_download_directory.clone(),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let max_concurrent = settings.settings().max_concurrent;
            let download_history = DownloadHistoryService::with_events(
                DownloadHistoryStore::new(home_directory.join(USER_DATA_DIRECTORY_NAME)),
                std::sync::Arc::new(TauriDownloadEventPublisher::new(app.handle().clone())),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let http_executor = HttpDownloadExecutor::new()
                .map_err(|error| std::io::Error::other(error.message))?;
            let download_executor = crate::infrastructure::media::CompositeDownloadExecutor::new(
                http_executor,
                reqwest::Client::new(),
            );
            let download_transfer = DownloadTransferService::new(
                download_history.clone(),
                download_executor,
                max_concurrent,
            );
            download_transfer.set_speed_limit(settings.settings().speed_limit_kbps);
            download_transfer.start_scheduler();

            app.manage(AppState::new(settings, download_history, download_transfer));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            add_download,
            download_action,
            update_settings,
            open_progress_window,
            close_auxiliary_window,
            resize_progress_window
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Awesome Downloader");
}
