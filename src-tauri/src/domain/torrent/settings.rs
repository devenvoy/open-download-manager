// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Torrent-only settings, stored separately from the download manager
// settings so the existing options schema stays untouched.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const TORRENT_SETTINGS_VERSION: u32 = 1;
pub const MAX_TORRENT_PEERS: u32 = 2000;
pub const MAX_SEED_RATIO: f64 = 1000.0;
pub const MAX_SEED_TIME_MINUTES: u64 = 525_600;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentSettings {
    pub version: u32,
    pub enabled: bool,
    /// None means "use <downloads>/Torrents".
    pub download_directory: Option<PathBuf>,
    /// 0 asks the OS for a random free port.
    pub listen_port: u16,
    pub enable_dht: bool,
    pub enable_upnp: bool,
    pub enable_utp: bool,
    pub download_limit_kbps: u32,
    pub upload_limit_kbps: u32,
    pub max_peers_per_torrent: u32,
    pub seed_ratio_limit: f64,
    pub seed_time_limit_minutes: u64,
    pub start_paused: bool,
    pub notify_on_complete: bool,
}

impl Default for TorrentSettings {
    fn default() -> Self {
        Self {
            version: TORRENT_SETTINGS_VERSION,
            enabled: true,
            download_directory: None,
            listen_port: 0,
            enable_dht: true,
            enable_upnp: false,
            enable_utp: false,
            download_limit_kbps: 0,
            upload_limit_kbps: 0,
            max_peers_per_torrent: 50,
            seed_ratio_limit: 0.0,
            seed_time_limit_minutes: 0,
            start_paused: false,
            notify_on_complete: true,
        }
    }
}

impl TorrentSettings {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(directory) = &self.download_directory {
            if !directory.is_absolute() {
                return Err("Choose an absolute torrent download folder.".to_owned());
            }
        }
        if self.max_peers_per_torrent == 0 || self.max_peers_per_torrent > MAX_TORRENT_PEERS {
            return Err(format!(
                "Peer limit must be between 1 and {MAX_TORRENT_PEERS}."
            ));
        }
        if !self.seed_ratio_limit.is_finite()
            || self.seed_ratio_limit < 0.0
            || self.seed_ratio_limit > MAX_SEED_RATIO
        {
            return Err(format!(
                "Seed ratio limit must be between 0 and {MAX_SEED_RATIO}."
            ));
        }
        if self.seed_time_limit_minutes > MAX_SEED_TIME_MINUTES {
            return Err(format!(
                "Seed time limit must be at most {MAX_SEED_TIME_MINUTES} minutes."
            ));
        }
        Ok(())
    }

    /// Directory the engine uses when a torrent does not override it.
    pub fn effective_download_directory(&self, fallback: &Path) -> PathBuf {
        self.download_directory
            .clone()
            .unwrap_or_else(|| fallback.join("Torrents"))
    }
}
