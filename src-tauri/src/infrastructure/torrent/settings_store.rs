// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Persists torrent-only settings to a dedicated JSON file. Never touches
// the download manager settings or history documents.

use std::path::{Path, PathBuf};

use crate::{
    domain::torrent::{TorrentSettings, TORRENT_SETTINGS_VERSION},
    infrastructure::atomic_file::write_replace,
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
            return TorrentSettings::default();
        }
        if migrate(&mut settings) {
            // The file on disk still holds the old values. Write them back so
            // the file and the running app agree.
            let _ = self.save(&settings);
        }
        settings
    }

    pub fn save(&self, settings: &TorrentSettings) -> std::io::Result<()> {
        let contents = serde_json::to_vec_pretty(settings)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        write_replace(&self.path, &contents)
    }
}

/// Brings settings written by an older version up to date.
///
/// The peer limit was raised from 50 to 200: the trackers hand out hundreds
/// of peers, but only the connected ones can send pieces, so a torrent that
/// is connected to ten peers runs at the speed of ten peers. The old value
/// was the app's own default, so it is migrated rather than kept.
///
/// Returns whether anything changed.
fn migrate(settings: &mut TorrentSettings) -> bool {
    let mut changed = false;
    if settings.version < 2 && settings.max_peers_per_torrent == 50 {
        settings.max_peers_per_torrent = 200;
        changed = true;
    }
    if settings.version < TORRENT_SETTINGS_VERSION {
        settings.version = TORRENT_SETTINGS_VERSION;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_peer_limit_is_migrated() {
        let mut settings = TorrentSettings {
            version: 1,
            max_peers_per_torrent: 50,
            ..Default::default()
        };
        assert!(migrate(&mut settings));
        assert_eq!(settings.max_peers_per_torrent, 200);
        assert_eq!(settings.version, TORRENT_SETTINGS_VERSION);
    }

    #[test]
    fn a_peer_limit_the_user_chose_is_kept() {
        let mut settings = TorrentSettings {
            version: 1,
            max_peers_per_torrent: 40,
            ..Default::default()
        };
        migrate(&mut settings);
        assert_eq!(settings.max_peers_per_torrent, 40);
    }

    #[test]
    fn current_settings_are_left_alone() {
        let mut settings = TorrentSettings::default();
        let before = settings.clone();
        assert!(!migrate(&mut settings));
        assert_eq!(settings.max_peers_per_torrent, before.max_peers_per_torrent);
    }
}
