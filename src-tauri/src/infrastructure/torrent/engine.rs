// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Creates the embedded librtbit session from torrent-only settings.

use std::{net::SocketAddr, num::NonZeroU32, path::Path, sync::Arc};

use librtbit::{
    limits::LimitsConfig, Api, ListenerMode, ListenerOptions, Session, SessionOptions,
    SessionPersistenceConfig,
};

use crate::domain::torrent::TorrentSettings;

pub struct TorrentEngine {
    pub session: Arc<Session>,
    pub api: Api,
}

/// Sanitizes the saved session directory before librtbit attempts to restore it.
/// If `session.json` is corrupt or references missing .torrent files, it cleans them
/// up atomically to prevent engine startup failure and crash loops.
pub fn prepare_saved_session(session_dir: &Path) {
    let db_path = session_dir.join("session.json");
    let text = match std::fs::read_to_string(&db_path) {
        Ok(t) => t,
        Err(_) => return,
    };

    let mut doc: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => {
            // Corrupted JSON: backup and remove so engine starts fresh without crashing
            let _ = std::fs::rename(&db_path, session_dir.join("session.json.corrupt"));
            return;
        }
    };

    let Some(torrents) = doc.get_mut("torrents").and_then(|t| t.as_object_mut()) else {
        return;
    };

    let mut changed = false;
    torrents.retain(|_, entry| {
        let Some(hash) = entry.get("info_hash").and_then(|h| h.as_str()) else {
            changed = true;
            return false;
        };
        let hash = hash.to_ascii_lowercase();
        let torrent_file = session_dir.join(format!("{hash}.torrent"));
        if !torrent_file.exists() || std::fs::metadata(&torrent_file).map(|m| m.len()).unwrap_or(0) == 0 {
            // Missing or empty .torrent file: remove invalid entry and clean up stale state
            for ext in &["torrent", "bitv"] {
                let _ = std::fs::remove_file(session_dir.join(format!("{hash}.{ext}")));
            }
            changed = true;
            false
        } else {
            true
        }
    });

    if changed {
        if let Ok(serialized) = serde_json::to_string_pretty(&doc) {
            let tmp_path = session_dir.join("session.json.tmp");
            if std::fs::write(&tmp_path, serialized.as_bytes()).is_ok() {
                let _ = std::fs::rename(&tmp_path, &db_path);
            } else {
                let _ = std::fs::remove_file(&tmp_path);
            }
        }
    }
}

/// Builds the engine. Network defaults are deliberately conservative:
/// TCP only, UPnP off, random port unless configured.
pub async fn create_engine(
    settings: &TorrentSettings,
    data_directory: &Path,
    default_download_directory: &Path,
) -> anyhow::Result<TorrentEngine> {
    let download_directory = settings.effective_download_directory(default_download_directory);
    std::fs::create_dir_all(&download_directory)?;

    let session_dir = data_directory.join("session");
    std::fs::create_dir_all(&session_dir)?;
    prepare_saved_session(&session_dir);

    let listen = ListenerOptions {
        mode: if settings.enable_utp {
            ListenerMode::TcpAndUtp
        } else {
            ListenerMode::TcpOnly
        },
        listen_addr: SocketAddr::from(([0, 0, 0, 0], settings.listen_port)),
        enable_upnp_port_forwarding: settings.enable_upnp,
        ..Default::default()
    };

    let options = SessionOptions {
        disable_dht: !settings.enable_dht,
        persistence: Some(SessionPersistenceConfig::Json {
            folder: Some(data_directory.join("session")),
        }),
        // The engine keeps its DHT routing table in its own configuration
        // folder by default, which every copy of the engine shares. A second
        // instance then reads the same file, tries the same UDP port and
        // fails to start the whole engine ("Address already in use"). Giving
        // the engine its own file inside the app folder keeps the port and
        // the routing table with the rest of the torrent state.
        dht_config: Some(librtbit::dht::PersistentDhtConfig {
            config_filename: Some(data_directory.join("dht.json")),
            ..Default::default()
        }),
        listen: Some(listen),
        fastresume: true,
        ratelimits: LimitsConfig {
            download_bps: NonZeroU32::new(settings.download_limit_kbps.saturating_mul(1024)),
            upload_bps: NonZeroU32::new(settings.upload_limit_kbps.saturating_mul(1024)),
            peer_limit: Some(settings.max_peers_per_torrent as usize),
            concurrent_init_limit: None,
        },
        peer_limit: Some(settings.max_peers_per_torrent as usize),
        ..Default::default()
    };

    let session = Session::new_with_opts(download_directory, options).await?;
    let api = Api::new(session.clone(), None);
    Ok(TorrentEngine { session, api })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_saved_session_corrupt_json() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("session.json");
        std::fs::write(&db_path, b"not valid json {{{").unwrap();

        prepare_saved_session(dir.path());

        assert!(!db_path.exists(), "corrupt session.json should be moved");
        assert!(dir.path().join("session.json.corrupt").exists(), "corrupt backup created");
    }

    #[test]
    fn test_prepare_saved_session_strips_missing_torrents() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("session.json");

        // Hash A has a valid .torrent file on disk
        let hash_a = "0123456789abcdef0123456789abcdef01234567";
        let file_a = dir.path().join(format!("{hash_a}.torrent"));
        std::fs::write(&file_a, b"dummy torrent content").unwrap();

        // Hash B is missing its .torrent file
        let hash_b = "fedcba9876543210fedcba9876543210fedcba98";

        let sample_json = serde_json::json!({
            "torrents": {
                "0": { "info_hash": hash_a },
                "1": { "info_hash": hash_b }
            }
        });
        std::fs::write(&db_path, serde_json::to_string(&sample_json).unwrap()).unwrap();

        prepare_saved_session(dir.path());

        let cleaned_text = std::fs::read_to_string(&db_path).unwrap();
        let cleaned: serde_json::Value = serde_json::from_str(&cleaned_text).unwrap();
        let torrents = cleaned.get("torrents").unwrap().as_object().unwrap();

        assert_eq!(torrents.len(), 1, "missing torrent should be stripped");
        assert!(torrents.values().any(|v| v.get("info_hash").unwrap().as_str().unwrap() == hash_a));
    }
}
