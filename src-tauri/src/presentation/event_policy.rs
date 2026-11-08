// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: refactor: modularize native download architecture
// Description: Event throttling and debouncing policies preventing IPC message flood during high-speed transfers.

pub(super) const DOWNLOAD_CHANGE_EVENT: &str = "downloader:change";
