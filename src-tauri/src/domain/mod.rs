// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(actions): add configurable post-download actions (Closes #31)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Description: Module definitions and domain abstractions for core business models and policies.

pub mod browser;
pub mod category;
pub mod clipboard;
pub mod completion;
pub mod credentials;
pub mod download;
pub mod file_type;
pub mod media;
pub mod media_tools;
pub mod protocol;
pub mod settings;
pub mod snapshot;
pub mod updater;

pub use file_type::FileType;
