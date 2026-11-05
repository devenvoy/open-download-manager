use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    application::updater_service::UpdateService,
    domain::{
        download::DownloadStatus,
        updater::{InstallDecision, UpdateCheckMode, UpdateCheckOutcome, UpdateReleaseInfo},
    },
    AppState,
};

pub async fn run_update_check(
    app: &AppHandle,
    update_service: &UpdateService,
    mode: UpdateCheckMode,
) {
    let outcome = match update_service.check(mode).await {
        Ok(outcome) => outcome,
        Err(err) => {
            if mode == UpdateCheckMode::Manual {
                show_error_dialog(
                    app,
                    "Open Download Manager could not check for updates.",
                    crate::domain::updater::friendly_update_error(&err.message),
                );
            }
            return;
        }
    };

    match outcome {
        UpdateCheckOutcome::DevModeUnavailable => {
            app.dialog()
                .message("Update checks run in installed builds.")
                .title("Updates are unavailable in development")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
        }
        UpdateCheckOutcome::UpToDate { version } => {
            app.dialog()
                .message("Open Download Manager is up to date.")
                .title("No updates available")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
            let _ = version;
        }
        UpdateCheckOutcome::PromptToDownload(info) => {
            prompt_for_download(app, update_service, info);
        }
        UpdateCheckOutcome::NotifyAvailable(info) => {
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("Open Download Manager update available")
                .body(format!("Version {} is ready to download.", info.version))
                .show();
        }
        UpdateCheckOutcome::Error { friendly_detail, .. } => {
            show_error_dialog(
                app,
                "Open Download Manager could not check for updates.",
                &friendly_detail,
            );
        }
        UpdateCheckOutcome::SilentNoop => {}
    }
}

pub fn prompt_for_download(app: &AppHandle, update_service: &UpdateService, info: UpdateReleaseInfo) {
    let app_clone = app.clone();
    let service_clone = update_service.clone();
    let info_clone = info.clone();

    app.dialog()
        .message(format!("Open Download Manager {} is available.\n\nDownload it now? You can continue using the app while it downloads.", info.version))
        .title("Update available")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Download update".to_string(),
            "Later".to_string(),
        ))
        .show(move |download_approved| {
            if download_approved {
                if let Some(ref download_url) = info_clone.download_url {
                    let download_url = download_url.clone();
                    let dest = std::env::temp_dir()
                        .join("open-download-manager-update")
                        .join(format!("Open-Download-Manager-Setup-{}.exe", info_clone.version));
                    let app_for_dl = app_clone.clone();
                    let service_for_dl = service_clone.clone();
                    let info_for_dl = info_clone.clone();

                    use tauri_plugin_notification::NotificationExt;
                    let _ = app_for_dl
                        .notification()
                        .builder()
                        .title("Downloading update")
                        .body(format!("Downloading Open Download Manager {} in background...", info_for_dl.version))
                        .show();

                    tauri::async_runtime::spawn(async move {
                        match service_for_dl.download_update_payload(&download_url, &dest).await {
                            Ok(()) => {
                                prompt_for_install(&app_for_dl, &service_for_dl, info_for_dl, Some(dest));
                            }
                            Err(err) => {
                                show_error_dialog(
                                    &app_for_dl,
                                    "Could not download update installer.",
                                    &err.message,
                                );
                            }
                        }
                    });
                } else {
                    prompt_for_install(&app_clone, &service_clone, info_clone, None);
                }
            }
        });
}

pub fn prompt_for_install(
    app: &AppHandle,
    update_service: &UpdateService,
    info: UpdateReleaseInfo,
    installer_path: Option<std::path::PathBuf>,
) {
    let active = if let Some(state) = app.try_state::<AppState>() {
        state
            .download_history
            .with_downloads(|list| {
                list.iter()
                    .filter(|d| {
                        matches!(
                            d.status,
                            DownloadStatus::Connecting
                                | DownloadStatus::Downloading
                                | DownloadStatus::Assembling
                        )
                    })
                    .count()
            })
            .unwrap_or(0)
    } else {
        0
    };

    let decision = update_service.evaluate_install_decision(active);
    let detail = match decision {
        InstallDecision::WarnActiveDownloads(count) => format!(
            "{count} download{} active. Choose Later to let them finish before restarting.",
            if count == 1 { " is" } else { "s are" }
        ),
        InstallDecision::ReadyToInstall => {
            "Restart Open Download Manager to install the update. Your download history and settings will be kept.".to_string()
        }
    };

    let app_clone = app.clone();
    app.dialog()
        .message(format!("Open Download Manager {} is ready to install.\n\n{}", info.version, detail))
        .title("Update ready")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restart and install".to_string(),
            "Later".to_string(),
        ))
        .show(move |restart_approved| {
            if restart_approved {
                if let Some(ref path) = installer_path {
                    if path.exists() {
                        #[cfg(windows)]
                        {
                            let _ = std::process::Command::new(path)
                                .arg("/S")
                                .spawn();
                        }
                        #[cfg(not(windows))]
                        {
                            let _ = opener::open(path);
                        }
                        app_clone.exit(0);
                        return;
                    }
                }
                app_clone.restart();
            }
        });
}

fn show_error_dialog(app: &AppHandle, message: &str, detail: &str) {
    app.dialog()
        .message(format!("{message}\n\n{detail}"))
        .title("Could not check for updates")
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| {});
}
