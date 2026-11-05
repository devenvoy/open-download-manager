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
    commands::{
        add_download, browser_setup_action, download_action, export_downloads_to_file,
        get_integration_info, import_downloads_from_file, register_browser, remove_completed,
        review_download, show_about_dialog, snapshot, update_settings,
    },
    download_event_publisher::TauriDownloadEventPublisher,
    window_manager::{
        close_auxiliary_window, open_add_window, open_integration_window, open_options_window,
        open_progress_window, open_scheduler_window, resize_progress_window,
    },
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
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == "--browser-setup") {
                let _ = presentation::window_manager::open_integration_window(app.clone());
            } else if !argv.iter().any(|arg| arg == "--background" || arg == "--browser-download") {
                presentation::tray::show_main_window(app);
            }
        }))
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

            let bridge_data_dir = home_directory.join(USER_DATA_DIRECTORY_NAME);
            let integration_service =
                application::integration_service::IntegrationService::new(bridge_data_dir.clone());
            let bridge_transfer = download_transfer.clone();
            let bridge_settings = settings.settings().clone();
            let integration_for_bridge = integration_service.clone();
            tauri::async_runtime::spawn(async move {
                match crate::infrastructure::bridge::LoopbackBridge::start(
                    bridge_data_dir,
                    bridge_transfer,
                    bridge_settings,
                )
                .await
                {
                    Ok(bridge) => {
                        integration_for_bridge.set_bridge_port(bridge.config.port);
                    }
                    Err(e) => {
                        eprintln!("Could not start loopback bridge: {}", e.message);
                    }
                }
            });

            let _ = crate::infrastructure::autostart::sync_autostart(
                settings.settings().start_with_windows,
            );

            let is_quitting = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let _tray = presentation::tray::setup_system_tray(&app.handle(), is_quitting.clone())
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            let menu = presentation::menu::build_application_menu(&app.handle())
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let _ = app.set_menu(menu);

            app.on_menu_event(|app, event| {
                presentation::menu::handle_menu_event(app, event.id().as_ref());
            });

            let is_background = std::env::args().any(|arg| arg == "--background");
            let is_browser_setup = std::env::args().any(|arg| arg == "--browser-setup");

            if let Some(main_window) = app.get_webview_window("main") {
                if !is_background && !is_browser_setup {
                    let _ = main_window.show();
                }
                let is_quitting_window = is_quitting.clone();
                let main_window_clone = main_window.clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if !is_quitting_window.load(std::sync::atomic::Ordering::SeqCst) {
                            api.prevent_close();
                            let _ = main_window_clone.hide();
                        }
                    }
                });
            }

            if is_browser_setup {
                let _ = presentation::window_manager::open_integration_window(app.handle().clone());
            }

            app.manage(AppState::new(
                settings,
                download_history,
                download_transfer,
                integration_service,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            add_download,
            download_action,
            remove_completed,
            update_settings,
            open_progress_window,
            close_auxiliary_window,
            resize_progress_window,
            get_integration_info,
            register_browser,
            browser_setup_action,
            review_download,
            open_options_window,
            open_add_window,
            open_scheduler_window,
            open_integration_window,
            export_downloads_to_file,
            import_downloads_from_file,
            show_about_dialog
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Awesome Downloader");
}
