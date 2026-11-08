// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Partial settings patch payload for atomic configuration updates from UI dialogs.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::domain::{category::DownloadCategory, settings::proxy::ProxySettings};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub default_directory: Option<String>,
    pub max_concurrent: Option<u8>,
    pub connections: Option<u8>,
    pub speed_limit_kbps: Option<u32>,
    pub category_directories: Option<BTreeMap<DownloadCategory, String>>,
    pub temporary_directory: Option<String>,
    pub notify_on_complete: Option<bool>,
    pub reveal_on_complete: Option<bool>,
    pub confirm_browser_downloads: Option<bool>,
    pub start_with_windows: Option<bool>,
    pub hide_completion_dialog: Option<bool>,
    pub proxy: Option<ProxySettings>,
    pub capture_policy: Option<crate::domain::browser::CapturePolicy>,
    pub completion_actions: Option<Vec<crate::domain::completion::CompletionAction>>,
}
