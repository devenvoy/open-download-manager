// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: fix(window): use window manager completion sizing constants to avoid wasted dialog space
// Description: Tauri IPC event publisher pushing real-time download updates to frontend windows.

use tauri::{AppHandle, Emitter, LogicalSize, Manager};

use crate::{
    application::{
        clipboard_service::ClipboardEventPublisher,
        download_event_publisher::DownloadEventPublisher,
        error::{AppError, AppResult},
    },
    domain::{clipboard::ClipboardCandidate, download::DownloadStatus},
    presentation::{
        app_state::AppState,
        window_manager::{
            COMPLETE_WINDOW_HEIGHT, COMPLETE_WINDOW_MIN_HEIGHT, COMPLETE_WINDOW_MIN_WIDTH,
            COMPLETE_WINDOW_WIDTH, PROGRESS_WINDOW_PREFIX,
        },
    },
};

use super::event_policy::{CLIPBOARD_CANDIDATE_EVENT, DOWNLOAD_CHANGE_EVENT};

pub struct TauriDownloadEventPublisher {
    app: AppHandle,
}

impl TauriDownloadEventPublisher {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

pub struct TauriClipboardEventPublisher {
    app: AppHandle,
}

impl TauriClipboardEventPublisher {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ClipboardEventPublisher for TauriClipboardEventPublisher {
    fn publish_candidate(&self, candidate: &ClipboardCandidate) -> Result<(), String> {
        self.app
            .emit(CLIPBOARD_CANDIDATE_EVENT, candidate)
            .map_err(|error| format!("Could not notify renderer of clipboard candidate: {error}"))
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
                                    let _ = window.set_min_size(Some(LogicalSize::new(
                                        COMPLETE_WINDOW_MIN_WIDTH,
                                        COMPLETE_WINDOW_MIN_HEIGHT,
                                    )));
                                    let _ = window.set_size(LogicalSize::new(
                                        COMPLETE_WINDOW_WIDTH,
                                        COMPLETE_WINDOW_HEIGHT,
                                    ));
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
