// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for browser extension integration, registration, and advanced media capabilities.

use tauri::State;

use crate::{
    application::{error::AppResult, integration_service::BrowserSetupResult},
    domain::{browser::AdvancedIntegrationCapability, snapshot::IntegrationStatus},
    presentation::app_state::AppState,
};

#[tauri::command(async)]
pub fn get_integration_info(state: State<'_, AppState>) -> AppResult<IntegrationStatus> {
    let _timer = crate::infrastructure::perf::track_command("get_integration_info");
    state.integration.get_integration_info()
}

#[tauri::command(async)]
pub async fn register_browser(
    state: State<'_, AppState>,
    browser: String,
    extension_id: String,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("register_browser");
    state
        .integration
        .register_browser(&browser, &extension_id)
        .await
}

#[tauri::command(async)]
pub async fn unregister_browser(
    state: State<'_, AppState>,
    browser: String,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("unregister_browser");
    state.integration.unregister_browser(&browser).await
}

#[tauri::command(async)]
pub async fn browser_setup_action(
    state: State<'_, AppState>,
    action: String,
    browser: String,
) -> AppResult<BrowserSetupResult> {
    let _timer = crate::infrastructure::perf::track_command("browser_setup_action");
    state.integration.browser_setup(&action, &browser).await
}

#[tauri::command(async)]
pub fn get_advanced_integration_capability(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    let _timer = crate::infrastructure::perf::track_command("get_advanced_integration_capability");
    Ok(state.advanced_integration.capability())
}

#[tauri::command(async)]
pub fn enable_advanced_integration(
    state: State<'_, AppState>,
    consent: bool,
) -> AppResult<AdvancedIntegrationCapability> {
    let _timer = crate::infrastructure::perf::track_command("enable_advanced_integration");
    state.advanced_integration.enable(consent)
}

#[tauri::command(async)]
pub fn disable_advanced_integration(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    let _timer = crate::infrastructure::perf::track_command("disable_advanced_integration");
    state.advanced_integration.disable()
}

#[tauri::command(async)]
pub fn repair_advanced_integration(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    let _timer = crate::infrastructure::perf::track_command("repair_advanced_integration");
    state.advanced_integration.repair()
}
