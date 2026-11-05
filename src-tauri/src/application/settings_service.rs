use std::path::PathBuf;

use crate::{
    application::error::{AppError, AppResult},
    domain::settings::{Settings, SettingsPatch},
};

pub trait SettingsRepository: Send + Sync {
    fn load(&self, default_directory: PathBuf) -> AppResult<Settings>;
    fn save(&self, settings: &Settings) -> AppResult<()>;
}

pub struct SettingsService {
    store: Box<dyn SettingsRepository>,
    settings: Settings,
}

impl SettingsService {
    pub fn new(
        store: impl SettingsRepository + 'static,
        default_directory: PathBuf,
    ) -> AppResult<Self> {
        let settings = store.load(default_directory)?;
        Ok(Self {
            store: Box::new(store),
            settings,
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn update(&mut self, patch: SettingsPatch) -> AppResult<Settings> {
        let next = self
            .settings
            .updated(patch)
            .map_err(|error| AppError::invalid_settings(error.message))?;
        self.store.save(&next)?;
        self.settings = next;
        Ok(self.settings.clone())
    }
}
