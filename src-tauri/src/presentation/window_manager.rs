use std::path::PathBuf;

use tauri::{AppHandle, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::application::error::{AppError, AppResult};

const PROGRESS_WINDOW_LABEL: &str = "progress";
const PROGRESS_WINDOW_WIDTH: f64 = 805.0;
const PROGRESS_WINDOW_COLLAPSED_HEIGHT: f64 = 465.0;
const PROGRESS_WINDOW_EXPANDED_HEIGHT: f64 = 650.0;

pub fn progress_route(id: &str) -> AppResult<String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::window("The download identifier is invalid."));
    }

    Ok(format!("index.html?progress={id}"))
}

#[tauri::command]
pub fn open_progress_window(app: AppHandle, id: String) -> AppResult<()> {
    let route = progress_route(&id)?;

    if let Some(window) = app.get_webview_window(PROGRESS_WINDOW_LABEL) {
        let query = route
            .strip_prefix("index.html")
            .ok_or_else(|| AppError::window("The progress window route is invalid."))?;
        let query = serde_json::to_string(query).map_err(|error| {
            AppError::window(format!("Could not encode the download identifier: {error}"))
        })?;
        window
            .eval(&format!("window.location.search = {query};"))
            .map_err(|error| AppError::window(error.to_string()))?;
        window
            .set_focus()
            .map_err(|error| AppError::window(error.to_string()))?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app,
        PROGRESS_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(route)),
    )
    .title("Download details")
    .inner_size(PROGRESS_WINDOW_WIDTH, PROGRESS_WINDOW_COLLAPSED_HEIGHT)
    .min_inner_size(600.0, 390.0)
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    Ok(())
}

#[tauri::command]
pub fn close_auxiliary_window(window: WebviewWindow) -> AppResult<()> {
    if window.label() == "main" {
        return Err(AppError::window(
            "The main window cannot be closed with this command.",
        ));
    }

    window
        .close()
        .map_err(|error| AppError::window(error.to_string()))
}

#[tauri::command]
pub fn resize_progress_window(window: WebviewWindow, expanded: bool) -> AppResult<()> {
    if window.label() != PROGRESS_WINDOW_LABEL {
        return Err(AppError::window(
            "Only the download details window can be resized with this command.",
        ));
    }

    let height = if expanded {
        PROGRESS_WINDOW_EXPANDED_HEIGHT
    } else {
        PROGRESS_WINDOW_COLLAPSED_HEIGHT
    };
    window
        .set_size(LogicalSize::new(PROGRESS_WINDOW_WIDTH, height))
        .map_err(|error| AppError::window(error.to_string()))
}
