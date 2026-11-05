use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    application::error::{AppError, AppResult},
    application::settings_service::SettingsRepository,
    domain::settings::{Settings, SettingsPatch},
};

const SETTINGS_VERSION: u8 = 1;
const LEGACY_STATE_VERSION: u8 = 1;

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    legacy_history_path: PathBuf,
}

impl SettingsStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        let data_directory = data_directory.into();
        Self {
            path: data_directory.join("settings.json"),
            legacy_history_path: data_directory.join("downloads.json"),
        }
    }

    fn load_settings(&self, default_directory: PathBuf) -> AppResult<Settings> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let state: PersistedSettings = serde_json::from_slice(&bytes).map_err(|error| {
                    AppError::storage(format!(
                        "Could not read settings at {}: {error}",
                        self.path.display()
                    ))
                })?;
                if state.version != SETTINGS_VERSION {
                    return Err(AppError::storage(format!(
                        "Unsupported settings version {} at {}.",
                        state.version,
                        self.path.display()
                    )));
                }
                state.settings.validate().map_err(|error| {
                    AppError::storage(format!("Stored settings are invalid: {}", error.message))
                })?;
                Ok(state.settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.load_legacy_settings(default_directory)
            }
            Err(error) => Err(AppError::storage(format!(
                "Could not read settings at {}: {error}",
                self.path.display()
            ))),
        }
    }

    fn load_legacy_settings(&self, default_directory: PathBuf) -> AppResult<Settings> {
        let bytes = match fs::read(&self.legacy_history_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::new(default_directory));
            }
            Err(error) => {
                return Err(AppError::storage(format!(
                    "Could not read legacy download history at {}: {error}",
                    self.legacy_history_path.display()
                )));
            }
        };

        let legacy: LegacyState = serde_json::from_slice(&bytes).map_err(|error| {
            AppError::storage(format!(
                "Could not read legacy download history at {}: {error}",
                self.legacy_history_path.display()
            ))
        })?;
        if legacy.version != LEGACY_STATE_VERSION {
            return Err(AppError::storage(format!(
                "Unsupported legacy history version {} at {}.",
                legacy.version,
                self.legacy_history_path.display()
            )));
        }

        let defaults = Settings::new(default_directory);
        let settings = match legacy.settings {
            Some(settings) => defaults.updated(settings.into_patch()).map_err(|error| {
                AppError::storage(format!(
                    "Legacy settings are invalid in {}: {}",
                    self.legacy_history_path.display(),
                    error.message
                ))
            })?,
            None => defaults,
        };

        settings.validate().map_err(|error| {
            AppError::storage(format!(
                "Legacy settings are invalid in {}: {}",
                self.legacy_history_path.display(),
                error.message
            ))
        })?;
        self.save_settings(&settings)?;
        Ok(settings)
    }

    fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| AppError::storage("Settings path does not have a parent directory."))?;
        fs::create_dir_all(parent).map_err(|error| {
            AppError::storage(format!(
                "Could not create settings directory {}: {error}",
                parent.display()
            ))
        })?;

        let state = PersistedSettings {
            version: SETTINGS_VERSION,
            settings: settings.clone(),
        };
        let contents = serde_json::to_vec_pretty(&state)
            .map_err(|error| AppError::storage(format!("Could not encode settings: {error}")))?;
        let temporary_path = temporary_path(&self.path);
        let result = write_and_replace(&temporary_path, &self.path, &contents);
        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result
    }
}

impl SettingsRepository for SettingsStore {
    fn load(&self, default_directory: PathBuf) -> AppResult<Settings> {
        self.load_settings(default_directory)
    }

    fn save(&self, settings: &Settings) -> AppResult<()> {
        self.save_settings(settings)
    }
}

#[derive(Deserialize, Serialize)]
struct PersistedSettings {
    version: u8,
    settings: Settings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyState {
    version: u8,
    settings: Option<LegacySettings>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySettings {
    default_directory: Option<String>,
    max_concurrent: Option<u8>,
    connections: Option<u8>,
    speed_limit_kbps: Option<u32>,
    category_directories: Option<std::collections::BTreeMap<String, String>>,
    notify_on_complete: Option<bool>,
    reveal_on_complete: Option<bool>,
    confirm_browser_downloads: Option<bool>,
    start_with_windows: Option<bool>,
    hide_completion_dialog: Option<bool>,
}

impl LegacySettings {
    fn into_patch(self) -> SettingsPatch {
        SettingsPatch {
            default_directory: self.default_directory,
            max_concurrent: self.max_concurrent,
            connections: self.connections,
            speed_limit_kbps: self.speed_limit_kbps,
            category_directories: self.category_directories,
            notify_on_complete: self.notify_on_complete,
            reveal_on_complete: self.reveal_on_complete,
            confirm_browser_downloads: self.confirm_browser_downloads,
            start_with_windows: self.start_with_windows,
            hide_completion_dialog: self.hide_completion_dialog,
        }
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()))
}

fn write_and_replace(temporary: &Path, destination: &Path, contents: &[u8]) -> AppResult<()> {
    let mut file = fs::File::create(temporary).map_err(|error| {
        AppError::storage(format!("Could not create temporary settings file: {error}"))
    })?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            AppError::storage(format!("Could not write temporary settings file: {error}"))
        })?;
    fs::rename(temporary, destination)
        .map_err(|error| AppError::storage(format!("Could not replace settings file: {error}")))
}
