use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    application::error::{AppError, AppResult},
    application::settings_service::SettingsRepository,
    domain::settings::Settings,
};

const SETTINGS_VERSION: u8 = 1;

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            path: data_directory.into().join("settings.json"),
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
                Ok(Settings::new(default_directory))
            }
            Err(error) => Err(AppError::storage(format!(
                "Could not read settings at {}: {error}",
                self.path.display()
            ))),
        }
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
