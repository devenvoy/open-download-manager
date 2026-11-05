use serde::Serialize;

#[derive(Debug, Serialize)]
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
}

pub type AppResult<T> = Result<T, AppError>;
