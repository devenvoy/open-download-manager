mod legacy_settings;
mod legacy_state;
mod persisted_settings;
mod policy;

use std::{fs, path::PathBuf};

use crate::{
    application::{
        error::{AppError, AppResult},
        settings_service::SettingsRepository,
    },
    domain::settings::Settings,
    infrastructure::{
        atomic_file::write_replace,
        data_file_policy::{LEGACY_HISTORY_FILE_NAME, SETTINGS_FILE_NAME},
    },
};

use self::{
    legacy_state::LegacyState,
    persisted_settings::PersistedSettings,
    policy::{LEGACY_STATE_VERSION, SETTINGS_VERSION},
};

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    legacy_history_path: PathBuf,
}

impl SettingsStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        let data_directory = data_directory.into();
        Self {
            path: data_directory.join(SETTINGS_FILE_NAME),
            legacy_history_path: data_directory.join(LEGACY_HISTORY_FILE_NAME),
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
        write_replace(&self.path, &contents)
            .map_err(|error| AppError::storage(format!("Could not replace settings file: {error}")))
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
