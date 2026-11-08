use serde::Deserialize;

use crate::domain::{category::DownloadCategory, settings::SettingsPatch};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LegacySettings {
    default_directory: Option<String>,
    max_concurrent: Option<u8>,
    connections: Option<u8>,
    speed_limit_kbps: Option<u32>,
    category_directories: Option<std::collections::BTreeMap<DownloadCategory, String>>,
    temporary_directory: Option<String>,
    notify_on_complete: Option<bool>,
    reveal_on_complete: Option<bool>,
    confirm_browser_downloads: Option<bool>,
    start_with_windows: Option<bool>,
    hide_completion_dialog: Option<bool>,
}

impl LegacySettings {
    pub(super) fn into_patch(self) -> SettingsPatch {
        SettingsPatch {
            default_directory: self.default_directory,
            max_concurrent: self.max_concurrent,
            connections: self.connections,
            speed_limit_kbps: self.speed_limit_kbps,
            category_directories: self.category_directories,
            temporary_directory: self.temporary_directory,
            notify_on_complete: self.notify_on_complete,
            reveal_on_complete: self.reveal_on_complete,
            confirm_browser_downloads: self.confirm_browser_downloads,
            start_with_windows: self.start_with_windows,
            hide_completion_dialog: self.hide_completion_dialog,
            proxy: None,
            capture_policy: None,
            completion_actions: None,
        }
    }
}
