use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

const MAX_CONCURRENT_DOWNLOADS: u8 = 16;
const MAX_CONNECTIONS_PER_DOWNLOAD: u8 = 32;
const MAX_SPEED_LIMIT_KBPS: u32 = 1_048_576;
const CATEGORIES: [&str; 6] = [
    "Video",
    "Audio",
    "Archives",
    "Documents",
    "Programs",
    "Other",
];

#[derive(Debug)]
pub struct SettingsValidationError {
    pub message: String,
}

impl SettingsValidationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub default_directory: PathBuf,
    pub max_concurrent: u8,
    pub connections: u8,
    pub speed_limit_kbps: u32,
    pub category_directories: BTreeMap<String, PathBuf>,
    pub notify_on_complete: bool,
    pub reveal_on_complete: bool,
    pub confirm_browser_downloads: bool,
    pub start_with_windows: bool,
    pub hide_completion_dialog: bool,
}

impl Settings {
    pub fn new(default_directory: PathBuf) -> Self {
        Self {
            default_directory,
            max_concurrent: 3,
            connections: 8,
            speed_limit_kbps: 0,
            category_directories: BTreeMap::new(),
            notify_on_complete: true,
            reveal_on_complete: false,
            confirm_browser_downloads: true,
            start_with_windows: true,
            hide_completion_dialog: false,
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
                .map(|(category, path)| (category, PathBuf::from(path)))
                .collect();
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

        next.validate()?;
        Ok(next)
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if !self.default_directory.is_absolute() {
            return Err(SettingsValidationError::new(
                "defaultDirectory must be an absolute path.",
            ));
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
        for (category, directory) in &self.category_directories {
            if !CATEGORIES.contains(&category.as_str()) || !directory.is_absolute() {
                return Err(SettingsValidationError::new(
                    "Stored category directories must use a supported category and absolute path.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub default_directory: Option<String>,
    pub max_concurrent: Option<u8>,
    pub connections: Option<u8>,
    pub speed_limit_kbps: Option<u32>,
    pub category_directories: Option<BTreeMap<String, String>>,
    pub notify_on_complete: Option<bool>,
    pub reveal_on_complete: Option<bool>,
    pub confirm_browser_downloads: Option<bool>,
    pub start_with_windows: Option<bool>,
    pub hide_completion_dialog: Option<bool>,
}

fn apply_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}
