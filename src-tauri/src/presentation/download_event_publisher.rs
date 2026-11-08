use tauri::{AppHandle, Emitter, LogicalSize, Manager};

use crate::{
    application::{
        download_event_publisher::DownloadEventPublisher,
        error::{AppError, AppResult},
    },
    domain::download::DownloadStatus,
    presentation::{app_state::AppState, window_manager::PROGRESS_WINDOW_PREFIX},
};

use super::event_policy::DOWNLOAD_CHANGE_EVENT;

pub struct TauriDownloadEventPublisher {
    app: AppHandle,
}

impl TauriDownloadEventPublisher {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl DownloadEventPublisher for TauriDownloadEventPublisher {
    fn publish_change(&self) -> AppResult<()> {
        self.app
            .emit(DOWNLOAD_CHANGE_EVENT, ())
            .map_err(|error| AppError::download(format!("Could not notify the renderer: {error}")))?;

        if let Some(state) = self.app.try_state::<AppState>() {
            let hide_completion = state
                .settings
                .lock()
                .ok()
                .map(|s| s.settings().hide_completion_dialog)
                .unwrap_or(false);

            let _ = state.download_history.with_downloads(|downloads| {
                for (label, window) in self.app.webview_windows() {
                    if let Some(id) = label.strip_prefix(PROGRESS_WINDOW_PREFIX) {
                        if let Some(d) = downloads.iter().find(|d| d.id == id) {
                            if d.status == DownloadStatus::Cancelled {
                                let _ = window.destroy();
                            } else if d.status == DownloadStatus::Completed {
                                if hide_completion {
                                    let _ = window.destroy();
                                } else {
                                    let _ = window.set_title("Download complete");
                                    let _ = window.set_min_size(Some(LogicalSize::new(470.0, 270.0)));
                                    let _ = window.set_size(LogicalSize::new(480.0, 280.0));
                                }
                            } else {
                                let title = if d.needs_review {
                                    "Download File Info".to_string()
                                } else {
                                    let pct = d
                                        .total
                                        .filter(|t| *t > 0)
                                        .map(|t| format!("{}% ", (d.downloaded * 100) / t))
                                        .unwrap_or_default();
                                    format!("{}{}", pct, d.filename)
                                };
                                let _ = window.set_title(&title);
                            }
                        } else {
                            let _ = window.destroy();
                        }
                    }
                }
            });
        }

        Ok(())
    }

    fn publish_complete(&self, download: &crate::domain::download::DownloadRecord) -> AppResult<()> {
        if let Some(state) = self.app.try_state::<AppState>() {
            let settings = state
                .settings
                .lock()
                .ok()
                .map(|s| s.settings().clone());

            if let Some(settings) = settings {
                if settings.notify_on_complete {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = self
                        .app
                        .notification()
                        .builder()
                        .title("Download complete")
                        .body(&download.filename)
                        .show();
                }
            }
        }
        Ok(())
    }
}
