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

/// Builds the engine. Network defaults are deliberately conservative:
/// TCP only, UPnP off, random port unless configured.
pub async fn create_engine(
    settings: &TorrentSettings,
    data_directory: &Path,
    default_download_directory: &Path,
) -> anyhow::Result<TorrentEngine> {
    let download_directory = settings.effective_download_directory(default_download_directory);
    std::fs::create_dir_all(&download_directory)?;

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
