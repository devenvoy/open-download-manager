// Created: 2026-11-12
// Reason: test(torrent): prove the review flow really transfers bytes
// Description: End-to-end test of the review flow against a local seeder.
//
// The window adds a torrent paused with an empty file selection, the user
// picks files, and the torrent is resumed. The engine accepts that sequence,
// but accepting it is not the same as downloading: the earlier engine
// contract test only checked the reported state, so a torrent could sit
// "live" forever without a byte moving.
//
// This test runs a real seeder on localhost and drives the exact service
// calls the Torrents window makes. It fails if the confirmed torrent does not
// actually receive the payload.

#![cfg(all(test, feature = "torrent"))]

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use librtbit::{
    create_torrent, spawn_utils::BlockingSpawner, AddTorrent, AddTorrentOptions,
    CreateTorrentOptions, ListenerMode, ListenerOptions, Session, SessionOptions,
};

use crate::{
    application::torrent_service::TorrentService,
    domain::torrent::{TorrentAction, TorrentAddOptions, TorrentSettings},
};

const FILE_SIZE: usize = 128 * 1024;
const PIECE_LENGTH: u32 = 16 * 1024;

/// Deterministic pseudo random bytes, so every run uses the same content.
fn pseudo_random_bytes(seed: u32, length: usize) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(length);
    for _ in 0..length {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push((state >> 16) as u8);
    }
    out
}

/// Writes the payload and builds a torrent for it.
///
/// The seed makes every torrent unique, so a test can add a second torrent
/// that is a different download.
async fn build_torrent(payload_directory: &Path) -> Vec<u8> {
    build_torrent_with_seed(payload_directory, 0xC0FFEE).await
}

async fn build_torrent_with_seed(payload_directory: &Path, seed: u32) -> Vec<u8> {
    std::fs::create_dir_all(payload_directory).expect("the payload folder must exist");
    for (index, name) in ["first.bin", "second.bin"].iter().enumerate() {
        std::fs::write(
            payload_directory.join(name),
            pseudo_random_bytes(seed + index as u32, FILE_SIZE),
        )
        .expect("the payload file must be written");
    }

    let created = create_torrent(
        payload_directory,
        CreateTorrentOptions {
            piece_length: Some(PIECE_LENGTH),
            ..Default::default()
        },
        &BlockingSpawner::new(1),
    )
    .await
    .expect("the torrent must be created");

    created
        .as_bytes()
        .expect("the torrent bytes must be readable")
        .to_vec()
}

/// Starts a seeder that serves the torrent and returns its address.
async fn start_seeder(
    torrent_bytes: Vec<u8>,
    payload_directory: &Path,
) -> (Arc<Session>, SocketAddr) {
    let session = Session::new_with_opts(
        payload_directory.to_path_buf(),
        SessionOptions {
            disable_dht: true,
            disable_local_service_discovery: true,
            persistence: None,
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpOnly,
                listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .expect("the seeder session must start");

    let handle = session
        .add_torrent(
            AddTorrent::TorrentFileBytes(torrent_bytes.into()),
            Some(AddTorrentOptions {
                overwrite: true,
                output_folder: Some(payload_directory.to_string_lossy().into_owned()),
                ..Default::default()
            }),
        )
        .await
        .expect("the seeder must accept the torrent")
        .into_handle()
        .expect("the seeder must get a handle");

    // The seeder is only useful once it is live and holds every piece.
    for _ in 0..400 {
        let stats = handle.stats();
        let ready = stats.state.to_string() == "live" && stats.finished;
        if ready {
            let address = session
                .listen_addr()
                .expect("the seeder must have a listen address");
            return (session, address);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the seeder must finish checking the payload");
}

/// Builds the service the Torrents window talks to, without the network.
async fn service(directory: &Path, downloads: &Path) -> Arc<TorrentService> {
    let settings = TorrentSettings {
        enabled: true,
        enable_dht: false,
        enable_upnp: false,
        enable_utp: false,
        ..Default::default()
    };
    std::fs::create_dir_all(directory).expect("the data folder must exist");
    std::fs::write(
        directory.join("torrent-settings.json"),
        serde_json::to_vec(&settings).expect("the settings must serialize"),
    )
    .expect("the settings must be written");

    let engine = crate::infrastructure::torrent::engine::create_engine(
        &settings,
        directory,
        downloads,
    )
    .await
    .expect("the engine must start");

    TorrentService::with_engine(
        directory.to_path_buf(),
        downloads.to_path_buf(),
        engine,
    )
}

fn temp_directory(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "odm-review-flow-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the test directory must exist");
    directory
}

/// The whole point of the review flow: after the user confirms, the payload
/// must arrive.
#[tokio::test(flavor = "multi_thread")]
async fn a_confirmed_review_downloads_the_payload() {
    let directory = temp_directory("confirm");
    let payload = directory.join("payload");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let torrent_bytes = build_torrent(&payload).await;
    let (_seeder, seeder_address) = start_seeder(torrent_bytes.clone(), &payload).await;

    let service = service(&directory.join("data"), &downloads).await;

    // 1. The window adds the torrent in review mode: paused, no files picked.
    let added = service
        .add_file_bytes(
            torrent_bytes,
            TorrentAddOptions {
                review: true,
                initial_peers: Some(vec![seeder_address.to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("the torrent must be added");
    let id = added.id.expect("the torrent must get an id");

    // The torrent waits for the user.
    let mut state = String::new();
    for _ in 0..200 {
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            state = item.state.clone();
            if state != "initializing" {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(state, "paused", "a review torrent must wait for the user");

    // 2. The user picks the files and confirms.
    service
        .set_files(id, vec![0])
        .await
        .expect("the file selection must be applied");
    service
        .action(id, TorrentAction::Resume)
        .await
        .expect("the torrent must start");

    // 3. The payload must arrive.
    let total = FILE_SIZE as u64;
    let mut downloaded = 0u64;
    let mut last_state = String::new();
    // Generous: the test drives real sockets over localhost, and a busy
    // machine slows the seeder down.
    for _ in 0..1800 {
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            last_state = item.state.clone();
            downloaded = item.progress_bytes;
            if downloaded >= total {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        downloaded, total,
        "the confirmed torrent must download the payload (state was {last_state})"
    );

    // The file must also be on disk with the right content.
    let written = find_file(&downloads, "first.bin").expect("the file must be on disk");
    assert_eq!(
        written.len(),
        FILE_SIZE,
        "the downloaded file must have the full length"
    );
    assert!(
        written == pseudo_random_bytes(0xC0FFEE, FILE_SIZE),
        "the downloaded file must match the payload"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// Finds a file by name below a folder.
fn find_file(root: &Path, name: &str) -> Option<Vec<u8>> {
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return std::fs::read(&path).ok();
        }
    }
    None
}

/// The same torrent added without the review must download too.
///
/// The review adds the torrent paused and starts it after the confirmation.
/// If that path were broken while the plain path worked, the engine would be
/// at fault; this test pins both paths to the same result.
#[tokio::test(flavor = "multi_thread")]
async fn a_torrent_added_without_the_review_downloads_the_payload() {
    let directory = temp_directory("live");
    let payload = directory.join("payload");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let torrent_bytes = build_torrent(&payload).await;
    let (_seeder, seeder_address) = start_seeder(torrent_bytes.clone(), &payload).await;

    let service = service(&directory.join("data"), &downloads).await;

    let added = service
        .add_file_bytes(
            torrent_bytes,
            TorrentAddOptions {
                start_paused: false,
                only_files: Some(vec![0]),
                initial_peers: Some(vec![seeder_address.to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("the torrent must be added");
    let id = added.id.expect("the torrent must get an id");

    let total = FILE_SIZE as u64;
    let mut downloaded = 0u64;
    // Generous: the test drives real sockets over localhost, and a busy
    // machine slows the seeder down.
    for _ in 0..1800 {
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            downloaded = item.progress_bytes;
            if downloaded >= total {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        downloaded, total,
        "a torrent added without the review must download the payload"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// The same flow against the real swarm.
///
/// This test needs the internet and is therefore skipped by default. It is
/// the only way to tell a broken engine apart from a torrent nobody seeds:
/// the local tests prove the flow works, this one proves a real swarm still
/// delivers.
///
/// Run it with:
///
/// ```text
/// cargo test --features torrent --lib -- --ignored a_confirmed_review_downloads_from_the_real_swarm
/// ```
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the internet"]
async fn a_confirmed_review_downloads_from_the_real_swarm() {
    // A well seeded, still supported release with a public tracker.
    const MAGNET: &str = "magnet:?xt=urn:btih:01c137287d6f0ed05a56742dae794f632c79ff3d&dn=ubuntu-24.04.4-desktop-amd64.iso&tr=https%3A%2F%2Ftorrent.ubuntu.com%2Fannounce";

    let directory = temp_directory("swarm");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let settings = TorrentSettings {
        enabled: true,
        enable_dht: true,
        ..Default::default()
    };
    std::fs::create_dir_all(directory.join("data")).expect("the data folder must exist");
    std::fs::write(
        directory.join("data").join("torrent-settings.json"),
        serde_json::to_vec(&settings).expect("the settings must serialize"),
    )
    .expect("the settings must be written");

    let engine = crate::infrastructure::torrent::engine::create_engine(
        &settings,
        &directory.join("data"),
        &downloads,
    )
    .await
    .expect("the engine must start");
    let service = TorrentService::with_engine(
        directory.join("data"),
        downloads.clone(),
        engine,
    );

    let added = service
        .add_magnet(
            MAGNET.to_owned(),
            TorrentAddOptions {
                review: true,
                ..Default::default()
            },
        )
        .await
        .expect("the magnet must be added");
    let id = added.id.expect("the torrent must get an id");

    service
        .set_files(id, vec![0])
        .await
        .expect("the file selection must be applied");
    service
        .action(id, TorrentAction::Resume)
        .await
        .expect("the torrent must start");

    let mut downloaded = 0u64;
    let mut seeds = 0u32;
    let mut total = 0u64;
    for _ in 0..1800 {
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            downloaded = item.progress_bytes;
            seeds = item.seeds;
            total = item.total_bytes;
            if downloaded > 0 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert!(
        downloaded > 0,
        "the real swarm must deliver bytes (seeds seen: {seeds}, total: {total})"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// The file selection must survive a torrent that is still being set up.
///
/// A magnet resolves its metadata in the background. The window can confirm
/// while the engine is still initializing, and the engine refuses to change
/// the selection in that state ("can't update initializing torrent"). The
/// service has to wait for the engine instead of failing.
#[tokio::test(flavor = "multi_thread")]
async fn a_file_selection_waits_for_the_engine_to_finish_the_setup() {
    let directory = temp_directory("race");
    let payload = directory.join("payload");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let torrent_bytes = build_torrent(&payload).await;
    let (_seeder, seeder_address) = start_seeder(torrent_bytes.clone(), &payload).await;

    let service = service(&directory.join("data"), &downloads).await;

    let added = service
        .add_file_bytes(
            torrent_bytes,
            TorrentAddOptions {
                review: true,
                initial_peers: Some(vec![seeder_address.to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("the torrent must be added");
    let id = added.id.expect("the torrent must get an id");

    // No waiting: the selection goes in while the engine is still busy.
    service
        .set_files(id, vec![0])
        .await
        .expect("the file selection must be applied even during the setup");
    service
        .action(id, TorrentAction::Resume)
        .await
        .expect("the torrent must start");

    let total = FILE_SIZE as u64;
    let mut downloaded = 0u64;
    // Generous: the test drives real sockets over localhost, and a busy
    // machine slows the seeder down.
    for _ in 0..1800 {
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            downloaded = item.progress_bytes;
            if downloaded >= total {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert_eq!(
        downloaded, total,
        "the confirmed torrent must download the payload"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// A pending review must not be handed out for a torrent that is gone.
///
/// The engine hands the same id to the next torrent once the list is empty.
/// A review that was waiting for a deleted torrent would otherwise show up
/// on a different download.
#[tokio::test(flavor = "multi_thread")]
async fn a_pending_review_for_a_deleted_torrent_is_dropped() {
    let directory = temp_directory("stale");
    let payload = directory.join("payload");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let torrent_bytes = build_torrent(&payload).await;
    let service = service(&directory.join("data"), &downloads).await;

    let added = service
        .add_file_bytes(
            torrent_bytes.clone(),
            TorrentAddOptions {
                review: true,
                ..Default::default()
            },
        )
        .await
        .expect("the torrent must be added");
    let id = added.id.expect("the torrent must get an id");

    // The window has not picked the review up yet.
    service.set_pending_review(Some(crate::domain::torrent::TorrentReviewRequest {
        id,
        name: added.name.clone(),
        info_hash: added.info_hash.clone(),
    }));

    // The user deletes the torrent instead of confirming it.
    service
        .action(id, TorrentAction::Forget)
        .await
        .expect("the torrent must be removed");

    // The next torrent takes the same id, but it is a different download.
    let other_payload = directory.join("other-payload");
    let other_bytes = build_torrent_with_seed(&other_payload, 0x1234_5678).await;
    let next = service
        .add_file_bytes(
            other_bytes,
            TorrentAddOptions {
                review: true,
                ..Default::default()
            },
        )
        .await
        .expect("the next torrent must be added");
    assert_eq!(next.id, Some(id), "the engine reuses the id");
    assert_ne!(
        next.info_hash, added.info_hash,
        "the next torrent is a different download"
    );

    assert!(
        service.take_pending_review().is_none(),
        "a review that belongs to a deleted torrent must not be handed out"
    );

    let _ = std::fs::remove_dir_all(&directory);
}

/// A review torrent must not move a single byte before the user confirms.
#[tokio::test(flavor = "multi_thread")]
async fn a_review_torrent_does_not_download_before_the_confirmation() {
    let directory = temp_directory("wait");
    let payload = directory.join("payload");
    let downloads = directory.join("downloads");
    std::fs::create_dir_all(&downloads).expect("the download folder must exist");

    let torrent_bytes = build_torrent(&payload).await;
    let (_seeder, seeder_address) = start_seeder(torrent_bytes.clone(), &payload).await;

    let service = service(&directory.join("data"), &downloads).await;

    let added = service
        .add_file_bytes(
            torrent_bytes,
            TorrentAddOptions {
                review: true,
                initial_peers: Some(vec![seeder_address.to_string()]),
                ..Default::default()
            },
        )
        .await
        .expect("the torrent must be added");
    let id = added.id.expect("the torrent must get an id");

    // Give the engine plenty of time to do the wrong thing.
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let list = service.list().expect("the list must be available");
        if let Some(item) = list.iter().find(|item| item.id == id) {
            assert_eq!(
                item.progress_bytes, 0,
                "a torrent waiting for the confirmation must not download"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&directory);
}
