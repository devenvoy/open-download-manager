// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Description: Module definitions and domain abstractions for core business models and policies.

pub mod browser;
pub mod category;
pub mod completion;
pub mod credentials;
pub mod download;
pub mod file_type;
pub mod media;
pub mod media_tools;
pub mod settings;
pub mod snapshot;
pub mod updater;

pub use file_type::FileType;
