// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Default configuration constants and boundary values for application settings.

pub(super) const DEFAULT_MAX_CONCURRENT_DOWNLOADS: u8 = 3;
pub(super) const DEFAULT_CONNECTIONS_PER_DOWNLOAD: u8 = 8;
pub(super) const DEFAULT_SPEED_LIMIT_KBPS: u32 = 0;
pub(super) const DEFAULT_NOTIFY_ON_COMPLETE: bool = true;
pub(super) const DEFAULT_REVEAL_ON_COMPLETE: bool = false;
pub(super) const DEFAULT_CONFIRM_BROWSER_DOWNLOADS: bool = true;
pub(super) const DEFAULT_START_WITH_WINDOWS: bool = true;
pub(super) const DEFAULT_HIDE_COMPLETION_DIALOG: bool = false;
pub(super) const MAX_CONCURRENT_DOWNLOADS: u8 = 16;
pub(super) const MAX_CONNECTIONS_PER_DOWNLOAD: u8 = 32;
pub(super) const MAX_SPEED_LIMIT_KBPS: u32 = 1_048_576;
