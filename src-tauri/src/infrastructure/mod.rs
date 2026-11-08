// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Description: Infrastructure layer implementations for filesystem, networking, media, and external tools.

pub mod action;
pub mod atomic_file;
pub mod autostart;
pub mod bridge;
pub mod browser;
pub mod clipboard;
pub mod credentials;
pub(crate) mod data_file_policy;
pub mod download_history_store;
pub mod file_icons;
pub mod fs_utils;
pub mod ftp;
pub mod http_download_executor;
pub mod media;
pub mod metrics;
pub mod settings_store;
pub mod system_shell;
pub mod updater;
