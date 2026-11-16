// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for proxy configuration, detection, and connectivity testing.

use tauri::State;

use crate::{
    application::error::AppResult,
    domain::settings::{DetectedProxyConfig, ProxySettings, ProxyTestResult, Settings},
    presentation::app_state::AppState,
};

#[tauri::command]
pub async fn test_proxy_connection(
    proxy: ProxySettings,
    test_url: Option<String>,
) -> AppResult<ProxyTestResult> {
    let _timer = crate::infrastructure::perf::track_command("test_proxy_connection");
    Ok(crate::infrastructure::http_download_executor::test_proxy_connection(
        &proxy,
        test_url.as_deref(),
    )
    .await)
}

#[tauri::command(async)]
pub fn detect_proxy(
    state: State<'_, AppState>,
) -> AppResult<DetectedProxyConfig> {
    let _timer = crate::infrastructure::perf::track_command("detect_proxy");
    state.proxy_discovery.detect()
}

#[tauri::command(async)]
pub fn apply_detected_proxy(
    state: State<'_, AppState>,
    detected: DetectedProxyConfig,
) -> AppResult<Settings> {
    let _timer = crate::infrastructure::perf::track_command("apply_detected_proxy");
    state.apply_detected_proxy(detected)
}
