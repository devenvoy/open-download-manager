// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Serializable torrent DTOs exchanged with the renderer. These types are
// independent from DownloadRecord so torrent state never enters download history.

use serde::{Deserialize, Serialize};

/// Lifecycle of the embedded torrent engine.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentEngineStatus {
    /// True when this build contains the torrent feature.
    pub available: bool,
    /// True when the user has torrent support enabled in settings.
    pub enabled: bool,
    /// disabled | starting | ready | failed | stopped
    pub state: String,
    pub message: Option<String>,
    pub listen_port: Option<u16>,
    pub dht_enabled: bool,
    /// Settings changed that need an engine restart to take effect.
    pub restart_required: bool,
}

impl TorrentEngineStatus {
    pub fn disabled() -> Self {
        Self {
            available: false,
            enabled: false,
            state: "disabled".to_owned(),
            message: None,
            listen_port: None,
            dht_enabled: false,
            restart_required: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFileItem {
    pub index: usize,
    pub name: String,
    pub length: u64,
    pub included: bool,
    pub priority: u8,
    pub progress_bytes: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentListItem {
    pub id: usize,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    /// initializing | live | paused | error | unknown
    pub state: String,
    pub error: Option<String>,
    pub progress_bytes: u64,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
    pub finished: bool,
    /// Bytes per second.
    pub download_speed: u64,
    /// Bytes per second.
    pub upload_speed: u64,
    pub time_remaining_secs: Option<u64>,
    pub seeds: u32,
    pub leechers: u32,
    pub peers: u32,
    /// Uploaded / downloaded, 0 when nothing has been downloaded yet.
    pub ratio: f64,
    pub added_on: Option<u64>,
    pub trackers: Vec<String>,
    pub files: Vec<TorrentFileItem>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TorrentAddOptions {
    pub output_folder: Option<String>,
    pub start_paused: bool,
    pub only_files: Option<Vec<usize>>,
    /// Fetch metadata with no files selected. The window then shows the file
    /// list and starts the transfer only after the user confirms.
    pub review: bool,
    /// Peer addresses (`host:port`) to try first. Useful when the caller
    /// already knows where the payload is, and it is what makes the review
    /// flow testable against a local seeder.
    pub initial_peers: Option<Vec<String>>,
}

/// Pending torrent waiting for the user to pick files before downloading.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentReviewRequest {
    pub id: usize,
    pub name: String,
    pub info_hash: String,
}

/// What one tracker says about the swarm of a torrent.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentTrackerStatus {
    pub url: String,
    /// not_contacted | updating | working | error | disabled
    pub state: String,
    /// Seeders the tracker knows about.
    pub seeders: Option<u32>,
    /// Leechers the tracker knows about.
    pub leechers: Option<u32>,
    /// Peer addresses the tracker handed out.
    pub peers_returned: Option<u32>,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TorrentAction {
    Pause,
    Resume,
    Recheck,
    Forget,
    Delete,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentAddResult {
    pub id: Option<usize>,
    pub info_hash: String,
    pub name: String,
    pub already_managed: bool,
    pub output_folder: String,
}
