// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for clipboard monitoring interactions.

use tauri::State;

use crate::{
    application::error::AppResult,
    presentation::app_state::AppState,
};

#[tauri::command]
pub fn dismiss_clipboard_candidate(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<()> {
    if let Some(ref clipboard) = state.clipboard {
        clipboard.dismiss(&url);
    }
    Ok(())
}
