// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Description: Domain settings entity defining download directories, category mappings, and connection limits.

mod patch;
mod policy;
pub mod proxy;
mod validation_error;

pub use patch::SettingsPatch;
pub use proxy::{ProxyProtocol, ProxySettings, ProxyTestResult, DEFAULT_PROXY_PORT};
pub use validation_error::SettingsValidationError;

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::category::DownloadCategory;

use self::policy::{
    DEFAULT_CONFIRM_BROWSER_DOWNLOADS, DEFAULT_CONNECTIONS_PER_DOWNLOAD,
    DEFAULT_HIDE_COMPLETION_DIALOG, DEFAULT_MAX_CONCURRENT_DOWNLOADS, DEFAULT_NOTIFY_ON_COMPLETE,
    DEFAULT_REVEAL_ON_COMPLETE, DEFAULT_SPEED_LIMIT_KBPS, DEFAULT_START_WITH_WINDOWS,
    MAX_CONCURRENT_DOWNLOADS, MAX_CONNECTIONS_PER_DOWNLOAD, MAX_SPEED_LIMIT_KBPS,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub default_directory: PathBuf,
    pub max_concurrent: u8,
    pub connections: u8,
    pub speed_limit_kbps: u32,
    pub category_directories: BTreeMap<DownloadCategory, PathBuf>,
    pub temporary_directory: Option<PathBuf>,
    pub notify_on_complete: bool,
    pub reveal_on_complete: bool,
    pub confirm_browser_downloads: bool,
    pub start_with_windows: bool,
    pub hide_completion_dialog: bool,
    #[serde(default)]
    pub proxy: ProxySettings,
    #[serde(default)]
    pub capture_policy: crate::domain::browser::CapturePolicy,
    #[serde(default)]
    pub completion_actions: Vec<crate::domain::completion::CompletionAction>,
    #[serde(default)]
    pub clipboard_monitoring: crate::domain::clipboard::ClipboardMonitoringSettings,
}

impl Settings {
    pub fn new(default_directory: PathBuf) -> Self {
        Self {
            default_directory,
            max_concurrent: DEFAULT_MAX_CONCURRENT_DOWNLOADS,
            connections: DEFAULT_CONNECTIONS_PER_DOWNLOAD,
            speed_limit_kbps: DEFAULT_SPEED_LIMIT_KBPS,
            category_directories: BTreeMap::new(),
            temporary_directory: None,
            notify_on_complete: DEFAULT_NOTIFY_ON_COMPLETE,
            reveal_on_complete: DEFAULT_REVEAL_ON_COMPLETE,
            confirm_browser_downloads: DEFAULT_CONFIRM_BROWSER_DOWNLOADS,
            start_with_windows: DEFAULT_START_WITH_WINDOWS,
            hide_completion_dialog: DEFAULT_HIDE_COMPLETION_DIALOG,
            proxy: ProxySettings::default(),
            capture_policy: crate::domain::browser::CapturePolicy::default(),
            completion_actions: Vec::new(),
            clipboard_monitoring: crate::domain::clipboard::ClipboardMonitoringSettings::default(),
        }
    }

    pub fn updated(&self, patch: SettingsPatch) -> Result<Self, SettingsValidationError> {
        let mut next = self.clone();

        if let Some(directory) = patch.default_directory {
            next.default_directory = PathBuf::from(directory);
        }
        if let Some(max_concurrent) = patch.max_concurrent {
            next.max_concurrent = max_concurrent;
        }
        if let Some(connections) = patch.connections {
            next.connections = connections;
        }
        if let Some(speed_limit_kbps) = patch.speed_limit_kbps {
            next.speed_limit_kbps = speed_limit_kbps;
        }
        if let Some(category_directories) = patch.category_directories {
            next.category_directories = category_directories
                .into_iter()
                .filter(|(_, path)| !path.trim().is_empty())
                .map(|(category, path)| (category, PathBuf::from(path.trim())))
                .collect();
        }
        if let Some(temporary_directory) = patch.temporary_directory {
            let trimmed = temporary_directory.trim();
            if trimmed.is_empty() {
                next.temporary_directory = None;
            } else {
                next.temporary_directory = Some(PathBuf::from(trimmed));
            }
        }

        apply_if_some(&mut next.notify_on_complete, patch.notify_on_complete);
        apply_if_some(&mut next.reveal_on_complete, patch.reveal_on_complete);
        apply_if_some(
            &mut next.confirm_browser_downloads,
            patch.confirm_browser_downloads,
        );
        apply_if_some(&mut next.start_with_windows, patch.start_with_windows);
        apply_if_some(
            &mut next.hide_completion_dialog,
            patch.hide_completion_dialog,
        );
        apply_if_some(&mut next.proxy, patch.proxy);
        apply_if_some(&mut next.capture_policy, patch.capture_policy);
        apply_if_some(&mut next.completion_actions, patch.completion_actions);
        apply_if_some(&mut next.clipboard_monitoring, patch.clipboard_monitoring);

        next.validate()?;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if !self.default_directory.is_absolute() {
            return Err(SettingsValidationError::new(
                "defaultDirectory must be an absolute path.",
            ));
        }
        if let Some(ref temp_dir) = self.temporary_directory {
            if !temp_dir.is_absolute() {
                return Err(SettingsValidationError::new(
                    "temporaryDirectory must be an absolute path.",
                ));
            }
        }
        if !(1..=MAX_CONCURRENT_DOWNLOADS).contains(&self.max_concurrent) {
            return Err(SettingsValidationError::new(format!(
                "maxConcurrent must be between 1 and {MAX_CONCURRENT_DOWNLOADS}."
            )));
        }
        if !(1..=MAX_CONNECTIONS_PER_DOWNLOAD).contains(&self.connections) {
            return Err(SettingsValidationError::new(format!(
                "connections must be between 1 and {MAX_CONNECTIONS_PER_DOWNLOAD}."
            )));
        }
        if self.speed_limit_kbps > MAX_SPEED_LIMIT_KBPS {
            return Err(SettingsValidationError::new(format!(
                "speedLimitKbps must not exceed {MAX_SPEED_LIMIT_KBPS}."
            )));
        }
        if self
            .category_directories
            .values()
            .any(|directory| !directory.is_absolute())
        {
            return Err(SettingsValidationError::new(
                "Stored category directories must be absolute paths.",
            ));
        }
        self.proxy.validate()?;
        for action in &self.completion_actions {
            if let Err(msg) = action.validate() {
                return Err(SettingsValidationError::new(msg));
            }
        }
        Ok(())
    }
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}
