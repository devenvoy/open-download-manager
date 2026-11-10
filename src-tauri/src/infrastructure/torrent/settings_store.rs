// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Persists torrent-only settings to a dedicated JSON file. Never touches
// the download manager settings or history documents.

use std::path::{Path, PathBuf};

use crate::{
    domain::torrent::TorrentSettings, infrastructure::atomic_file::write_replace,
};

pub const TORRENT_SETTINGS_FILE_NAME: &str = "torrent-settings.json";

pub struct TorrentSettingsStore {
    path: PathBuf,
}

impl TorrentSettingsStore {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            path: data_directory.into().join(TORRENT_SETTINGS_FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Missing or unreadable settings fall back to defaults; torrent settings must
    /// never prevent the download manager from starting.
    pub fn load(&self) -> TorrentSettings {
        let Ok(contents) = std::fs::read(&self.path) else {
            return TorrentSettings::default();
        };
        let Ok(mut settings) = serde_json::from_slice::<TorrentSettings>(&contents) else {
            return TorrentSettings::default();
        };
        if settings.validate().is_err() {
            settings = TorrentSettings::default();
        }
        settings
    }

    pub fn save(&self, settings: &TorrentSettings) -> std::io::Result<()> {
        let contents = serde_json::to_vec_pretty(settings)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        write_replace(&self.path, &contents)
    }
}
