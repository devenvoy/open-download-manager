// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(transfer): offer explicit actions when destination file already exists (Closes #26) (#41)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Description: Centralized application error type with stable diagnostic error codes and messages.

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn invalid_settings(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_settings",
            message: message.into(),
        }
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self {
            code: "storage_error",
            message: message.into(),
        }
    }

    pub fn invalid_download(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_download",
            message: message.into(),
        }
    }

    pub fn download(message: impl Into<String>) -> Self {
        Self {
            code: "download_error",
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: "destination_conflict",
            message: message.into(),
        }
    }

    pub fn not_migrated(message: impl Into<String>) -> Self {
        Self {
            code: "feature_not_migrated",
            message: message.into(),
        }
    }

    pub fn window(message: impl Into<String>) -> Self {
        Self {
            code: "window_error",
            message: message.into(),
        }
    }

    pub fn bridge(message: impl Into<String>) -> Self {
        Self {
            code: "bridge_error",
            message: message.into(),
        }
    }

    pub fn browser(message: impl Into<String>) -> Self {
        Self {
            code: "browser_error",
            message: message.into(),
        }
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self {
            code: "network_error",
            message: message.into(),
        }
    }

    pub fn update(message: impl Into<String>) -> Self {
        Self {
            code: "update_error",
            message: message.into(),
        }
    }

    pub fn link_expired(message: impl Into<String>) -> Self {
        Self {
            code: "link_expired",
            message: message.into(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
