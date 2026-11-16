// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for site credential rules management.

use tauri::State;

use crate::{
    application::error::AppResult,
    domain::credentials::CredentialRule,
    presentation::app_state::AppState,
};

#[tauri::command(async)]
pub fn list_credential_rules(
    state: State<'_, AppState>,
) -> AppResult<Vec<CredentialRule>> {
    let _timer = crate::infrastructure::perf::track_command("list_credential_rules");
    state.credentials.list_redacted()
}

#[tauri::command(async)]
pub fn save_credential_rule(
    state: State<'_, AppState>,
    rule: CredentialRule,
) -> AppResult<CredentialRule> {
    let _timer = crate::infrastructure::perf::track_command("save_credential_rule");
    state.credentials.save(rule)
}

#[tauri::command(async)]
pub fn delete_credential_rule(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let _timer = crate::infrastructure::perf::track_command("delete_credential_rule");
    state.credentials.delete(&id)
}
