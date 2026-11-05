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
                    "Awesome Downloader could not check for updates.",
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
                .message("Awesome Downloader is up to date.")
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
                .title("Awesome Downloader update available")
                .body(format!("Version {} is ready to download.", info.version))
                .show();
        }
        UpdateCheckOutcome::Error { friendly_detail, .. } => {
            show_error_dialog(
                app,
                "Awesome Downloader could not check for updates.",
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
        .message(format!("Awesome Downloader {} is available.\n\nDownload it now? You can continue using the app while it downloads.", info.version))
        .title("Update available")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Download update".to_string(),
            "Later".to_string(),
        ))
        .show(move |download_approved| {
            if download_approved {
                prompt_for_install(&app_clone, &service_clone, info_clone);
            }
        });
}

pub fn prompt_for_install(app: &AppHandle, update_service: &UpdateService, info: UpdateReleaseInfo) {
    let active = if let Some(state) = app.try_state::<AppState>() {
        state
            .download_history
            .downloads()
            .map(|list| {
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
            "Restart Awesome Downloader to install the update. Your download history and settings will be kept.".to_string()
        }
    };

    let app_clone = app.clone();
    app.dialog()
        .message(format!("Awesome Downloader {} is ready to install.\n\n{}", info.version, detail))
        .title("Update ready")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restart and install".to_string(),
            "Later".to_string(),
        ))
        .show(move |restart_approved| {
            if restart_approved {
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
