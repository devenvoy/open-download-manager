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

    pub fn window(message: impl Into<String>) -> Self {
        Self {
            code: "window_error",
            message: message.into(),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
