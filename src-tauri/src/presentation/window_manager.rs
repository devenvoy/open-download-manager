// Created: 2026-11-05
// Reason: feat: migrate history and adopt Bun
// Last updated: 2026-11-08
// Reason: feat(extension): improve video format picker UI, desktop handoff focus, and media tools handling (fixes #5)
// Reason: fix(ui): expand review and add window dimensions to prevent button cut-off
// Reason: feat(window): dynamic runtime content fitting and compact initial dialog dimensions
// Description: Auxiliary window manager controlling progress dialogs, options windows, and dimensions.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tauri::{
    AppHandle, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    application::error::{AppError, AppResult},
    domain::download::{DownloadAction, DownloadStatus},
    presentation::app_state::AppState,
};

pub const PROGRESS_WINDOW_PREFIX: &str = "progress-";
pub const OPTIONS_WINDOW_LABEL: &str = "options";
pub const ADD_WINDOW_LABEL: &str = "add";
pub const SCHEDULER_WINDOW_LABEL: &str = "scheduler";
pub const INTEGRATION_WINDOW_LABEL: &str = "integration";
pub const TORRENT_WINDOW_LABEL: &str = "torrents";

// Sizing constants matching the legacy desktop shell exactly
pub const PROGRESS_WINDOW_WIDTH: f64 = 565.0;
pub const PROGRESS_WINDOW_EXPANDED_HEIGHT: f64 = 485.0;
pub const PROGRESS_WINDOW_COLLAPSED_HEIGHT: f64 = 340.0;
pub const PROGRESS_WINDOW_MIN_WIDTH: f64 = 540.0;
pub const PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT: f64 = 440.0;
pub const PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT: f64 = 325.0;

pub const REVIEW_WINDOW_HEIGHT: f64 = 370.0;
pub const REVIEW_WINDOW_MIN_HEIGHT: f64 = 300.0;

pub const COMPLETE_WINDOW_WIDTH: f64 = 480.0;
pub const COMPLETE_WINDOW_HEIGHT: f64 = 230.0;
pub const COMPLETE_WINDOW_MIN_WIDTH: f64 = 460.0;
pub const COMPLETE_WINDOW_MIN_HEIGHT: f64 = 190.0;

pub const OPTIONS_WINDOW_WIDTH: f64 = 720.0;
pub const OPTIONS_WINDOW_HEIGHT: f64 = 560.0;
pub const OPTIONS_WINDOW_MIN_WIDTH: f64 = 640.0;
pub const OPTIONS_WINDOW_MIN_HEIGHT: f64 = 480.0;

pub const ADD_WINDOW_WIDTH: f64 = 565.0;
pub const ADD_WINDOW_HEIGHT: f64 = 370.0;
pub const ADD_WINDOW_MIN_WIDTH: f64 = 520.0;
pub const ADD_WINDOW_MIN_HEIGHT: f64 = 300.0;

pub const SCHEDULER_WINDOW_WIDTH: f64 = 550.0;
pub const SCHEDULER_WINDOW_HEIGHT: f64 = 430.0;
pub const SCHEDULER_WINDOW_MIN_WIDTH: f64 = 500.0;
pub const SCHEDULER_WINDOW_MIN_HEIGHT: f64 = 360.0;

pub const INTEGRATION_WINDOW_WIDTH: f64 = 700.0;
pub const INTEGRATION_WINDOW_HEIGHT: f64 = 620.0;
pub const INTEGRATION_WINDOW_MIN_WIDTH: f64 = 620.0;
pub const INTEGRATION_WINDOW_MIN_HEIGHT: f64 = 480.0;

pub const TORRENT_WINDOW_WIDTH: f64 = 900.0;
pub const TORRENT_WINDOW_HEIGHT: f64 = 700.0;
pub const TORRENT_WINDOW_MIN_WIDTH: f64 = 700.0;
pub const TORRENT_WINDOW_MIN_HEIGHT: f64 = 500.0;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenAddOptions {
    #[serde(default)]
    pub paste: Option<bool>,
    pub scheduled: Option<String>,
}

pub fn progress_label(id: &str) -> String {
    format!("{PROGRESS_WINDOW_PREFIX}{id}")
}

static PENDING_ADD_OPTIONS: std::sync::Mutex<Option<OpenAddOptions>> = std::sync::Mutex::new(None);

#[tauri::command(async)]
pub fn take_add_window_options() -> Option<OpenAddOptions> {
    PENDING_ADD_OPTIONS.lock().unwrap().take()
}

pub fn progress_route(id: &str) -> AppResult<String> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::window("The download identifier is invalid."));
    }

    Ok("index.html".to_string())
}

pub fn options_route() -> &'static str {
    "index.html"
}

pub fn torrent_route() -> &'static str {
    "index.html"
}

pub fn scheduler_route() -> &'static str {
    "index.html"
}

pub fn integration_route() -> &'static str {
    "index.html"
}

pub fn add_route(_options: Option<&OpenAddOptions>) -> String {
    "index.html".to_string()
}

pub fn needs_close_choice(status: &DownloadStatus, needs_review: bool) -> bool {
    if needs_review {
        return false;
    }
    !matches!(status, DownloadStatus::Completed | DownloadStatus::Cancelled)
}

pub fn should_cancel_review_on_close(needs_review: bool) -> bool {
    needs_review
}

pub fn activate_existing_window(window: &WebviewWindow) -> AppResult<()> {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    if !crate::infrastructure::smoke::is_smoke_test() {
        window
            .set_focus()
            .map_err(|error| AppError::window(error.to_string()))?;
    }
    Ok(())
}

fn initial_aux_window_visibility() -> bool {
    false
}

pub async fn show_window_on_ready_or_fallback(
    window: WebviewWindow,
    label: String,
    start: std::time::Instant,
) {
    let fallback = std::time::Duration::from_millis(1500);
    let _ = crate::infrastructure::perf::wait_for_window_ui_ready(&label, fallback).await;
    let _ = window.show();
    if !crate::infrastructure::smoke::is_smoke_test() {
        let _ = window.set_focus();
    }
    crate::infrastructure::perf::record_window_shown(&label, start.elapsed());
}

#[tauri::command(async)]
pub fn open_progress_window(app: AppHandle, id: String) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_progress_window");
    let start = std::time::Instant::now();
    let label = progress_label(&id);

    if let Some(window) = app.get_webview_window(&label) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(&label, start.elapsed());
        return res;
    }

    let download_info = app.try_state::<AppState>().and_then(|state| {
        state.download_history.with_downloads(|list| {
            list.iter().find(|d| d.id == id).cloned()
        }).ok().flatten()
    });

    let (title, width, height, min_w, min_h) = if let Some(d) = &download_info {
        if d.needs_review {
            ("Download File Info".to_string(), PROGRESS_WINDOW_WIDTH, REVIEW_WINDOW_HEIGHT, PROGRESS_WINDOW_MIN_WIDTH, REVIEW_WINDOW_MIN_HEIGHT)
        } else if d.status == DownloadStatus::Completed {
            ("Download complete".to_string(), COMPLETE_WINDOW_WIDTH, COMPLETE_WINDOW_HEIGHT, COMPLETE_WINDOW_MIN_WIDTH, COMPLETE_WINDOW_MIN_HEIGHT)
        } else {
            (d.filename.clone(), PROGRESS_WINDOW_WIDTH, PROGRESS_WINDOW_EXPANDED_HEIGHT, PROGRESS_WINDOW_MIN_WIDTH, PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT)
        }
    } else {
        ("Download details".to_string(), PROGRESS_WINDOW_WIDTH, PROGRESS_WINDOW_EXPANDED_HEIGHT, PROGRESS_WINDOW_MIN_WIDTH, PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT)
    };

    let route = progress_route(&id)?;
    let window = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(PathBuf::from(route)),
    )
    .title(&title)
    .inner_size(width, height)
    .min_inner_size(min_w, min_h)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(&label);

    let win_clone = window.clone();
    let lbl_clone = label.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, lbl_clone, start).await;
    });

    let app_handle = app.clone();
    let download_id = id.clone();
    let prompt_active = Arc::new(AtomicBool::new(false));

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            handle_progress_close(&app_handle, &download_id, api, &prompt_active);
        }
    });

    Ok(())
}

fn handle_progress_close(
    app: &AppHandle,
    id: &str,
    api: &tauri::CloseRequestApi,
    prompt_active: &Arc<AtomicBool>,
) {
    let download = app.try_state::<AppState>().and_then(|state| {
        state.download_history.with_downloads(|list| {
            list.iter().find(|d| d.id == id).cloned()
        }).ok().flatten()
    });

    let Some(d) = download else {
        return;
    };

    if should_cancel_review_on_close(d.needs_review) {
        api.prevent_close();
        if let Some(state) = app.try_state::<AppState>() {
            let _ = state.download_transfer.action(id, DownloadAction::Remove);
        }
        if let Some(w) = app.get_webview_window(&progress_label(id)) {
            let _ = w.destroy();
        }
        return;
    }

    if !needs_close_choice(&d.status, d.needs_review) {
        return;
    }

    api.prevent_close();

    if prompt_active
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let app_clone = app.clone();
    let id_string = id.to_string();
    let prompt_flag = prompt_active.clone();

    app.dialog()
        .message("What would you like to do?\n\nMinimize keeps the download running. Cancel download stops it and closes this window.")
        .title("Close download window")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Minimize".to_string(),
            "Cancel download".to_string(),
        ))
        .show(move |minimize| {
            prompt_flag.store(false, Ordering::SeqCst);
            if let Some(w) = app_clone.get_webview_window(&progress_label(&id_string)) {
                if minimize {
                    let _ = w.minimize();
                } else {
                    if let Some(state) = app_clone.try_state::<AppState>() {
                        let _ = state.download_transfer.action(&id_string, DownloadAction::Cancel);
                    }
                    let _ = w.destroy();
                }
            }
        });
}

#[tauri::command(async)]
pub fn close_auxiliary_window(window: WebviewWindow) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("close_auxiliary_window");
    if window.label() == "main" {
        return Err(AppError::window(
            "The main window cannot be closed with this command.",
        ));
    }

    window
        .close()
        .map_err(|error| AppError::window(error.to_string()))
}

#[tauri::command(async)]
pub fn resize_progress_window(window: WebviewWindow, expanded: bool) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("resize_progress_window");
    if !window.label().starts_with(PROGRESS_WINDOW_PREFIX) && window.label() != "progress" {
        return Err(AppError::window(
            "Only the download details window can be resized with this command.",
        ));
    }

    let (width, height, min_w, min_h) = if expanded {
        (
            PROGRESS_WINDOW_WIDTH,
            PROGRESS_WINDOW_EXPANDED_HEIGHT,
            PROGRESS_WINDOW_MIN_WIDTH,
            PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT,
        )
    } else {
        (
            PROGRESS_WINDOW_WIDTH,
            PROGRESS_WINDOW_COLLAPSED_HEIGHT,
            PROGRESS_WINDOW_MIN_WIDTH,
            PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT,
        )
    };

    let _ = window.set_min_size(Some(LogicalSize::new(min_w, min_h)));
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|error| AppError::window(error.to_string()))
}

#[tauri::command(async)]
pub fn resize_auxiliary_window(
    window: WebviewWindow,
    width: Option<f64>,
    height: Option<f64>,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("resize_auxiliary_window");
    if window.label() == "main" {
        return Err(AppError::window(
            "The main window cannot be resized with this command.",
        ));
    }

    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let current_size = window
        .inner_size()
        .map_err(|error| AppError::window(error.to_string()))?
        .to_logical::<f64>(scale_factor);

    let target_width = width.unwrap_or(current_size.width).clamp(380.0, 960.0);
    let target_height = height.unwrap_or(current_size.height).clamp(180.0, 850.0);

    let _ = window.set_min_size(Some(LogicalSize::new(
        target_width.min(460.0),
        target_height.min(190.0),
    )));

    window
        .set_size(LogicalSize::new(target_width, target_height))
        .map_err(|error| AppError::window(error.to_string()))
}

#[tauri::command(async)]
pub fn open_options_window(app: AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_options_window");
    let start = std::time::Instant::now();
    if let Some(window) = app.get_webview_window(OPTIONS_WINDOW_LABEL) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(OPTIONS_WINDOW_LABEL, start.elapsed());
        return res;
    }

    let window = WebviewWindowBuilder::new(
        &app,
        OPTIONS_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(options_route())),
    )
    .title("Options")
    .inner_size(OPTIONS_WINDOW_WIDTH, OPTIONS_WINDOW_HEIGHT)
    .min_inner_size(OPTIONS_WINDOW_MIN_WIDTH, OPTIONS_WINDOW_MIN_HEIGHT)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(OPTIONS_WINDOW_LABEL);

    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, OPTIONS_WINDOW_LABEL.to_string(), start).await;
    });
    Ok(())
}

#[tauri::command(async)]
pub fn open_add_window(
    app: AppHandle,
    options: Option<OpenAddOptions>,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_add_window");
    let start = std::time::Instant::now();
    if let Some(scheduler) = app.get_webview_window(SCHEDULER_WINDOW_LABEL) {
        let _ = scheduler.close();
    }

    if let Some(ref opts) = options {
        *PENDING_ADD_OPTIONS.lock().unwrap() = Some(opts.clone());
    }

    if let Some(window) = app.get_webview_window(ADD_WINDOW_LABEL) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(ADD_WINDOW_LABEL, start.elapsed());
        return res;
    }

    let route = add_route(options.as_ref());
    let window = WebviewWindowBuilder::new(
        &app,
        ADD_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(route)),
    )
    .title("Download File Info")
    .inner_size(ADD_WINDOW_WIDTH, ADD_WINDOW_HEIGHT)
    .min_inner_size(ADD_WINDOW_MIN_WIDTH, ADD_WINDOW_MIN_HEIGHT)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(ADD_WINDOW_LABEL);

    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, ADD_WINDOW_LABEL.to_string(), start).await;
    });
    Ok(())
}

#[tauri::command(async)]
pub fn open_scheduler_window(app: AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_scheduler_window");
    let start = std::time::Instant::now();
    if let Some(window) = app.get_webview_window(SCHEDULER_WINDOW_LABEL) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(SCHEDULER_WINDOW_LABEL, start.elapsed());
        return res;
    }

    let window = WebviewWindowBuilder::new(
        &app,
        SCHEDULER_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(scheduler_route())),
    )
    .title("Scheduler")
    .inner_size(SCHEDULER_WINDOW_WIDTH, SCHEDULER_WINDOW_HEIGHT)
    .min_inner_size(SCHEDULER_WINDOW_MIN_WIDTH, SCHEDULER_WINDOW_MIN_HEIGHT)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(SCHEDULER_WINDOW_LABEL);

    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, SCHEDULER_WINDOW_LABEL.to_string(), start).await;
    });
    Ok(())
}

#[tauri::command(async)]
pub fn open_integration_window(app: AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_integration_window");
    let start = std::time::Instant::now();
    if let Some(window) = app.get_webview_window(INTEGRATION_WINDOW_LABEL) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(INTEGRATION_WINDOW_LABEL, start.elapsed());
        return res;
    }

    let window = WebviewWindowBuilder::new(
        &app,
        INTEGRATION_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(integration_route())),
    )
    .title("Browser integration")
    .inner_size(INTEGRATION_WINDOW_WIDTH, INTEGRATION_WINDOW_HEIGHT)
    .min_inner_size(INTEGRATION_WINDOW_MIN_WIDTH, INTEGRATION_WINDOW_MIN_HEIGHT)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(INTEGRATION_WINDOW_LABEL);

    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, INTEGRATION_WINDOW_LABEL.to_string(), start).await;
    });
    Ok(())
}

#[tauri::command(async)]
pub fn open_torrent_window(app: AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("open_torrent_window");
    let start = std::time::Instant::now();
    if let Some(window) = app.get_webview_window(TORRENT_WINDOW_LABEL) {
        let res = activate_existing_window(&window);
        crate::infrastructure::perf::record_window_shown(TORRENT_WINDOW_LABEL, start.elapsed());
        return res;
    }

    let window = WebviewWindowBuilder::new(
        &app,
        TORRENT_WINDOW_LABEL,
        WebviewUrl::App(PathBuf::from(torrent_route())),
    )
    .title("Torrents")
    .inner_size(TORRENT_WINDOW_WIDTH, TORRENT_WINDOW_HEIGHT)
    .min_inner_size(TORRENT_WINDOW_MIN_WIDTH, TORRENT_WINDOW_MIN_HEIGHT)
    .visible(initial_aux_window_visibility())
    .background_color(tauri::webview::Color(30, 30, 30, 255))
    .build()
    .map_err(|error| AppError::window(error.to_string()))?;

    let watcher = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            if let Some(state) = watcher.app_handle().try_state::<AppState>() {
                state.torrents.set_watching(false);
            }
        }
    });

    let _ = window.remove_menu();
    crate::infrastructure::perf::record_window_open(TORRENT_WINDOW_LABEL);

    let win_clone = window.clone();
    tauri::async_runtime::spawn(async move {
        show_window_on_ready_or_fallback(win_clone, TORRENT_WINDOW_LABEL.to_string(), start).await;
    });
    Ok(())
}
