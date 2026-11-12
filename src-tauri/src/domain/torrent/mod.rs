// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Torrent domain module. Deliberately separate from the download entity,
// status machine, history, and queue model.

pub mod dto;
pub mod settings;

pub use dto::{
    TorrentAction, TorrentAddOptions, TorrentAddResult, TorrentEngineStatus, TorrentFileItem,
    TorrentListItem, TorrentReviewRequest, TorrentTrackerStatus,
};
pub use settings::{TorrentSettings, TORRENT_SETTINGS_VERSION};
