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
        add_download, add_playlist_downloads, browser_setup_action, check_for_updates, download_action,
        export_downloads_to_file, get_integration_info, get_media_tools_status, get_playlist_info,
        get_video_formats, import_downloads_from_file, install_media_tool, register_browser, remove_completed,
        review_download, show_about_dialog, snapshot, test_proxy_connection, update_settings,
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
        .plugin(tauri_plugin_notification::init())
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
                SettingsStore::new(data_directory.clone()),
                default_download_directory.clone(),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let max_concurrent = settings.settings().max_concurrent;
            let download_history = DownloadHistoryService::with_events(
                DownloadHistoryStore::new(data_directory.clone()),
                std::sync::Arc::new(TauriDownloadEventPublisher::new(app.handle().clone())),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let initial_client = crate::infrastructure::http_download_executor::create_http_client(
                Some(&settings.settings().proxy),
            )
            .map_err(|error| std::io::Error::other(error.message))?;
            let shared_client = std::sync::Arc::new(std::sync::RwLock::new(initial_client));
            let http_executor = HttpDownloadExecutor::with_shared_client(shared_client.clone());
            let download_executor = crate::infrastructure::media::CompositeDownloadExecutor::with_shared(
                http_executor,
                shared_client,
            );
            let download_transfer = DownloadTransferService::new(
                download_history.clone(),
                download_executor,
                max_concurrent,
            );
            download_transfer.set_speed_limit(settings.settings().speed_limit_kbps);
            download_transfer.start_scheduler();

            let bridge_data_dir = data_directory.clone();
            let integration_service =
                application::integration_service::IntegrationService::new(bridge_data_dir.clone());
            let bridge_transfer = download_transfer.clone();
            let bridge_settings = settings.settings().clone();
            let integration_for_bridge = integration_service.clone();
            let app_handle_for_bridge = app.handle().clone();
            let on_download = std::sync::Arc::new(move |id: String| {
                let app = app_handle_for_bridge.clone();
                let app_for_thread = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let _ = presentation::window_manager::open_progress_window(app_for_thread, id);
                });
            });

            tauri::async_runtime::spawn(async move {
                match crate::infrastructure::bridge::LoopbackBridge::start(
                    bridge_data_dir,
                    bridge_transfer,
                    bridge_settings,
                    Some(on_download),
                )
                .await
                {
                    Ok(bridge) => {
                        integration_for_bridge.set_bridge_port(bridge.config.port);
                        // Keep loopback bridge alive for the lifetime of the application
                        std::future::pending::<()>().await;
                        drop(bridge);
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

            let file_icons = crate::infrastructure::file_icons::FileIconProvider::new(
                data_directory.join("FileTypeIcons"),
            );

            let version = app.package_info().version.to_string();
            let is_dev_mode = cfg!(debug_assertions);
            let update_client = std::sync::Arc::new(
                crate::infrastructure::updater::GitHubReleaseClient::default_client(),
            );
            let update_service = application::updater_service::UpdateService::new(
                update_client,
                version,
                is_dev_mode,
            );

            let update_service_bg = update_service.clone();
            let app_handle_bg = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                    crate::domain::updater::CHECK_INTERVAL_SECONDS,
                ));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    presentation::updater_controller::run_update_check(
                        &app_handle_bg,
                        &update_service_bg,
                        crate::domain::updater::UpdateCheckMode::Background,
                    )
                    .await;
                }
            });

            app.manage(AppState::new(
                settings,
                download_history,
                download_transfer,
                integration_service,
                file_icons,
                update_service,
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
            show_about_dialog,
            check_for_updates,
            get_playlist_info,
            get_video_formats,
            add_playlist_downloads,
            get_media_tools_status,
            install_media_tool,
            test_proxy_connection
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Open Download Manager");
}
