// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for browser extension integration, registration, and advanced media capabilities.

use tauri::State;

use crate::{
    application::{error::AppResult, integration_service::BrowserSetupResult},
    domain::{browser::AdvancedIntegrationCapability, snapshot::IntegrationStatus},
    presentation::app_state::AppState,
};

#[tauri::command]
pub fn get_integration_info(state: State<'_, AppState>) -> AppResult<IntegrationStatus> {
    state.integration.get_integration_info()
}

#[tauri::command]
pub async fn register_browser(
    state: State<'_, AppState>,
    browser: String,
    extension_id: String,
) -> AppResult<()> {
    state
        .integration
        .register_browser(&browser, &extension_id)
        .await
}

#[tauri::command]
pub async fn browser_setup_action(
    state: State<'_, AppState>,
    action: String,
    browser: String,
) -> AppResult<BrowserSetupResult> {
    state.integration.browser_setup(&action, &browser).await
}

#[tauri::command]
pub fn get_advanced_integration_capability(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    Ok(state.advanced_integration.capability())
}

#[tauri::command]
pub fn enable_advanced_integration(
    state: State<'_, AppState>,
    consent: bool,
) -> AppResult<AdvancedIntegrationCapability> {
    state.advanced_integration.enable(consent)
}

#[tauri::command]
pub fn disable_advanced_integration(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    state.advanced_integration.disable()
}

#[tauri::command]
pub fn repair_advanced_integration(
    state: State<'_, AppState>,
) -> AppResult<AdvancedIntegrationCapability> {
    state.advanced_integration.repair()
}
