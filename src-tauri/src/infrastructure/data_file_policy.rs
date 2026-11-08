// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Filesystem path resolution for application support folders across Windows, macOS, and Linux.

pub const USER_DATA_DIRECTORY_NAME: &str = ".open-download-manager";
pub const SETTINGS_FILE_NAME: &str = "settings.json";
pub const LEGACY_HISTORY_FILE_NAME: &str = "downloads.json";
pub const TAURI_HISTORY_FILE_NAME: &str = "downloads-tauri.json";
