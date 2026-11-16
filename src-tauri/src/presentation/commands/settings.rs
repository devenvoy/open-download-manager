// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for application settings, snapshot queries, directory validation, and updater checks.

use tauri::State;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        settings::{Settings, SettingsPatch},
        snapshot::{DownloadSummary, IntegrationStatus, Snapshot},
        updater::UpdateCheckMode,
    },
    presentation::app_state::AppState,
};

#[tauri::command(async)]
pub fn snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    let _timer = crate::infrastructure::perf::track_command("snapshot");
    let start = std::time::Instant::now();
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::storage("Application settings are unavailable."))?;
    let integration = state.integration.get_integration_info().unwrap_or_else(|e| {
        IntegrationStatus::unavailable(e.message)
    });
    let (summaries, icons) = state.download_history.with_downloads(|records| {
        state.file_icons.load(records);
        let icons = state.file_icons.snapshot();
        let summaries: Vec<DownloadSummary> = records.iter().map(DownloadSummary::from).collect();
        (summaries, icons)
    })?;
    let item_count = summaries.len();
    let icon_count = icons.len();
    let snapshot_res = Snapshot::with_summaries(
        settings.settings().clone(),
        summaries,
        integration,
    ).with_file_icons(icons);
    let build_elapsed = start.elapsed();

    // Measure serialization overhead
    let serialize_start = std::time::Instant::now();
    let serialized_bytes = serde_json::to_vec(&snapshot_res).map(|v| v.len()).unwrap_or(0);
    let serialize_elapsed = serialize_start.elapsed();
    let total_elapsed = start.elapsed();

    crate::infrastructure::perf::log_perf(
        "SNAPSHOT",
        &format!(
            "Snapshot built in {:.2}ms, serialized in {:.2}ms (total: {:.2}ms) | items: {item_count}, icons: {icon_count}, size: {serialized_bytes} bytes",
            build_elapsed.as_secs_f64() * 1000.0,
            serialize_elapsed.as_secs_f64() * 1000.0,
            total_elapsed.as_secs_f64() * 1000.0,
        ),
    );

    Ok(snapshot_res)
}

#[tauri::command(async)]
pub fn update_settings(
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> AppResult<Settings> {
    let _timer = crate::infrastructure::perf::track_command("update_settings");
    state.update_settings(patch)
}

#[tauri::command(async)]
pub fn validate_directory(path: String) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("validate_directory");
    crate::infrastructure::fs_utils::validate_directory_writable(std::path::Path::new(&path))
        .map_err(AppError::invalid_settings)
}

#[tauri::command(async)]
pub fn show_about_dialog(app: tauri::AppHandle) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("show_about_dialog");
    crate::presentation::menu::show_about_dialog(&app);
    Ok(())
}

#[tauri::command(async)]
pub async fn check_for_updates(app: tauri::AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("check_for_updates");
    let service = state.update_service.clone();
    crate::presentation::updater_controller::run_update_check(
        &app,
        &service,
        UpdateCheckMode::Manual,
    )
    .await;
    Ok(())
}

#[tauri::command(async)]
pub fn mark_ui_ready(label: Option<String>) {
    let target = label.as_deref().unwrap_or("main");
    let _timer = crate::infrastructure::perf::track_command("mark_ui_ready");
    crate::infrastructure::perf::mark_window_ui_ready(target);
}

#[tauri::command(async)]
pub fn window_ready(label: Option<String>) {
    let target = label.as_deref().unwrap_or("main");
    let _timer = crate::infrastructure::perf::track_command("window_ready");
    crate::infrastructure::perf::mark_window_ui_ready(target);
}

#[tauri::command]
pub fn test_sleep_sync(seconds: Option<u64>) -> AppResult<()> {
    #[cfg(feature = "dev-tools")]
    {
        let _timer = crate::infrastructure::perf::track_command("test_sleep_sync");
        std::thread::sleep(std::time::Duration::from_secs(seconds.unwrap_or(3)));
    }
    #[cfg(not(feature = "dev-tools"))]
    {
        let _ = seconds;
    }
    Ok(())
}

#[tauri::command(async)]
pub fn test_sleep_async(seconds: Option<u64>) -> AppResult<()> {
    #[cfg(feature = "dev-tools")]
    {
        let _timer = crate::infrastructure::perf::track_command("test_sleep_async");
        std::thread::sleep(std::time::Duration::from_secs(seconds.unwrap_or(3)));
    }
    #[cfg(not(feature = "dev-tools"))]
    {
        let _ = seconds;
    }
    Ok(())
}
