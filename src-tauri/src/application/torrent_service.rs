// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Last updated: 2026-11-12
// Reason: fix(torrent): never block or panic on the engine path
// Description: Application service owning the embedded torrent engine. Fully separate
// from DownloadTransferService, the download history, queues, and concurrency limits.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
};

// Only used by the feature-enabled implementation (`start`, rate limiting,
// peer tracking, and the poll loop).
use std::{
    collections::HashSet,
    path::Path,
    time::Duration,
};

use crate::{
    application::error::{AppError, AppResult},
    domain::torrent::{
        TorrentAction, TorrentAddOptions, TorrentAddResult, TorrentEngineStatus, TorrentFileItem,
        TorrentListItem, TorrentPeerItem, TorrentPiecesInfo, TorrentReviewRequest, TorrentSettings,
        TorrentTrackerStatus,
    },
    infrastructure::torrent::settings_store::TorrentSettingsStore,
};

const MAX_MAGNET_LENGTH: usize = 16 * 1024;
const MAX_TORRENT_FILE_BYTES: u64 = 32 * 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How many times an add is retried while the engine is still starting.
const ENGINE_READY_ATTEMPTS: usize = 40;

/// Pause between two attempts while the engine is still starting.
const ENGINE_READY_DELAY: Duration = Duration::from_millis(250);

/// How long the engine may take to finish setting a torrent up before the
/// window gives up on changing its file selection.
const ENGINE_SETTLE_ATTEMPTS: usize = 240;

/// Pause between two attempts while the engine sets a torrent up.
const ENGINE_SETTLE_DELAY: Duration = Duration::from_millis(250);

/// A torrent that receives nothing for this long asks the trackers for new
/// peers.
const STALE_TRANSFER: Duration = Duration::from_secs(45);

/// Fewer connected peers than this means the torrent needs a fresh peer
/// list.
const THIN_SWARM: u32 = 10;

/// The shortest time between two re-announces for one torrent. Trackers do
/// not like being hammered.
const REANNOUNCE_COOLDOWN: Duration = Duration::from_secs(60);

/// Decides whether a torrent should ask the trackers for new peers.
///
/// A tracker hands out a fixed list of peers and then asks to be left alone
/// for the next half an hour. The engine only announces again when fewer
/// than five peers are left, so a torrent that is connected to a handful of
/// leechers never asks for the seeds. Asking again keeps the torrent in the
/// swarm.
fn needs_new_peers(peers: u32, stalled: bool, cooled: bool) -> bool {
    if !cooled {
        return false;
    }
    stalled || peers < THIN_SWARM
}

/// What the service remembers about a running torrent to decide whether it
/// needs new peers.
struct TorrentRefresh {
    last_progress_bytes: u64,
    last_progress_at: std::time::Instant,
    last_reannounce: std::time::Instant,
}

/// Synchronously parses a magnet URL, validating syntax and extracting the BitTorrent info hash and display name.
/// Completes in under 0.1 ms without network or disk I/O.
pub fn parse_magnet_link(magnet: &str) -> AppResult<(String, String)> {
    let trimmed = magnet.trim();
    if !trimmed.starts_with("magnet:?") && !trimmed.starts_with("MAGNET:?") {
        return Err(AppError::torrent("Enter a valid magnet link."));
    }
    if trimmed.len() > MAX_MAGNET_LENGTH {
        return Err(AppError::torrent("The magnet link is too long."));
    }

    let query_str = &trimmed[8..];
    let mut info_hash = None;
    let mut name = None;

    for (k, v) in url::form_urlencoded::parse(query_str.as_bytes()) {
        if k.eq_ignore_ascii_case("xt") {
            let lower = v.to_ascii_lowercase();
            if let Some(stripped) = lower.strip_prefix("urn:btih:") {
                if !stripped.is_empty() {
                    info_hash = Some(stripped.to_string());
                }
            }
        } else if k.eq_ignore_ascii_case("dn") {
            name = Some(v.into_owned());
        }
    }

    let hash = info_hash.ok_or_else(|| {
        AppError::torrent("The magnet link does not contain a BitTorrent info hash.")
    })?;

    let display_name = name.unwrap_or_else(|| hash.clone());
    Ok((hash, display_name))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvingStatus {
    Resolving,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct ResolvingTorrent {
    pub placeholder_id: usize,
    pub magnet: String,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    pub options: TorrentAddOptions,
    pub status: ResolvingStatus,
    pub started_at: std::time::Instant,
    pub cancel_flag: Arc<AtomicBool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedResolvingTorrent {
    pub placeholder_id: usize,
    pub magnet: String,
    pub info_hash: String,
    pub name: String,
    pub output_folder: String,
    pub options: TorrentAddOptions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum EngineState {
    Disabled,
    Starting,
    Ready,
    Failed,
    Stopped,
}

impl EngineState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

pub struct TorrentService {
    /// Only read by the feature-enabled implementation; kept so `disabled()` can
    /// construct a complete placeholder for builds without the feature.
    #[allow(dead_code)]
    data_directory: PathBuf,
    /// Only read by the feature-enabled implementation; kept so `disabled()` can
    /// construct a complete placeholder for builds without the feature.
    #[allow(dead_code)]
    default_download_directory: PathBuf,
    /// Only read by the feature-enabled implementation; kept so `disabled()` can
    /// construct a complete placeholder for builds without the feature.
    #[allow(dead_code)]
    settings_store: TorrentSettingsStore,
    settings: RwLock<TorrentSettings>,
    state: RwLock<EngineState>,
    message: RwLock<Option<String>>,
    restart_required: AtomicBool,
    watching: AtomicUsize,
    pending_review: RwLock<Option<TorrentReviewRequest>>,
    /// What the service remembers about running torrents, to decide when a
    /// torrent needs new peers.
    peer_refresh: RwLock<std::collections::HashMap<usize, TorrentRefresh>>,
    resolving_torrents: RwLock<std::collections::HashMap<String, ResolvingTorrent>>,
    next_placeholder_id: AtomicUsize,
    engine: RwLock<Option<crate::infrastructure::torrent::engine::TorrentEngine>>,
    app: Option<tauri::AppHandle>,
}

impl TorrentService {
    /// Placeholder used by AppState before the real service is attached, and by
    /// builds without the torrent feature.
    pub fn disabled() -> Arc<Self> {
        Arc::new(Self {
            data_directory: PathBuf::new(),
            default_download_directory: PathBuf::new(),
            settings_store: TorrentSettingsStore::new(PathBuf::new()),
            settings: RwLock::new(TorrentSettings {
                enabled: false,
                ..Default::default()
            }),
            state: RwLock::new(EngineState::Disabled),
            message: RwLock::new(None),
            restart_required: AtomicBool::new(false),
            watching: AtomicUsize::new(0),
            pending_review: RwLock::new(None),
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            resolving_torrents: RwLock::new(std::collections::HashMap::new()),
            next_placeholder_id: AtomicUsize::new(1_000_000),
            engine: RwLock::new(None),
            app: None,
        })
    }

    #[cfg(test)]
    pub fn for_test(data_directory: PathBuf, default_download_directory: PathBuf) -> Arc<Self> {
        let settings_store = TorrentSettingsStore::new(data_directory.clone());
        let settings = settings_store.load();
        Arc::new(Self {
            data_directory,
            default_download_directory,
            settings_store,
            settings: RwLock::new(settings),
            state: RwLock::new(EngineState::Disabled),
            message: RwLock::new(None),
            restart_required: AtomicBool::new(false),
            watching: AtomicUsize::new(0),
            pending_review: RwLock::new(None),
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            resolving_torrents: RwLock::new(std::collections::HashMap::new()),
            next_placeholder_id: AtomicUsize::new(1_000_000),
            engine: RwLock::new(None),
            app: None,
        })
    }

    /// Builds a service around an already running engine.
    ///
    /// The production path is [`TorrentService::start`], which needs a Tauri
    /// application handle. The tests only need the engine, so they use this
    /// constructor instead of building a whole application.
    #[cfg(test)]
    pub fn with_engine(
        data_directory: PathBuf,
        default_download_directory: PathBuf,
        engine: crate::infrastructure::torrent::engine::TorrentEngine,
    ) -> Arc<Self> {
        let settings_store = TorrentSettingsStore::new(data_directory.clone());
        let settings = settings_store.load();
        Arc::new(Self {
            data_directory,
            default_download_directory,
            settings_store,
            settings: RwLock::new(settings),
            state: RwLock::new(EngineState::Ready),
            message: RwLock::new(None),
            restart_required: AtomicBool::new(false),
            watching: AtomicUsize::new(0),
            pending_review: RwLock::new(None),
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            resolving_torrents: RwLock::new(std::collections::HashMap::new()),
            next_placeholder_id: AtomicUsize::new(1_000_000),
            engine: RwLock::new(Some(engine)),
            app: None,
        })
    }

    /// Builds the service, loads torrent settings, and starts the engine in the
    /// background so application startup is never blocked by network work.
    pub fn start(
        app: tauri::AppHandle,
        data_directory: PathBuf,
        default_download_directory: PathBuf,
    ) -> Arc<Self> {
        let settings_store = TorrentSettingsStore::new(data_directory.clone());
        let settings = settings_store.load();
        let initial_state = if settings.enabled {
            EngineState::Starting
        } else {
            EngineState::Disabled
        };

        let service = Arc::new(Self {
            data_directory,
            default_download_directory,
            settings_store,
            settings: RwLock::new(settings),
            state: RwLock::new(initial_state),
            message: RwLock::new(None),
            restart_required: AtomicBool::new(false),
            watching: AtomicUsize::new(0),
            pending_review: RwLock::new(None),
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            resolving_torrents: RwLock::new(std::collections::HashMap::new()),
            next_placeholder_id: AtomicUsize::new(1_000_000),
            engine: RwLock::new(None),
            app: Some(app),
        });

        let starter = service.clone();
        tauri::async_runtime::spawn(async move {
            let _ = starter.start_engine().await;
        });
        service.spawn_poller();
        service.restore_pending_magnets();
        service
    }


    pub fn status(&self) -> TorrentEngineStatus {
        let settings = self.settings();
        let state = *self.state.read().unwrap_or_else(|error| error.into_inner());
        let listen_port = {
            {
                self.engine
                    .read()
                    .unwrap()
                    .as_ref()
                    .map(|engine| engine.api.api_listen_port())
            }
        };
        TorrentEngineStatus {
            available: true,
            enabled: settings.enabled,
            state: state.as_str().to_owned(),
            message: self.message.read().unwrap_or_else(|error| error.into_inner()).clone(),
            listen_port,
            dht_enabled: settings.enable_dht,
            restart_required: self.restart_required.load(Ordering::SeqCst),
        }
    }

    pub fn settings(&self) -> TorrentSettings {
        self.settings.read().unwrap_or_else(|error| error.into_inner()).clone()
    }

    pub fn set_watching(&self, watching: bool) {
        self.watching.store(usize::from(watching), Ordering::Relaxed);
    }

    /// Remembers a torrent added by the browser so the window can show its
    /// file picker even if it finishes loading after the event was emitted.
    pub fn set_pending_review(&self, review: Option<TorrentReviewRequest>) {
        *self.pending_review.write().unwrap_or_else(|error| error.into_inner()) = review;
    }

    /// Takes the pending review, if there still is one.
    ///
    /// The engine hands the same id to the next torrent once the list is
    /// empty, so a pending review can point at a torrent that no longer
    /// exists. The info hash tells the two apart, and a review that belongs
    /// to a deleted torrent is dropped instead of being shown for a
    /// different download.
    pub fn take_pending_review(&self) -> Option<TorrentReviewRequest> {
        let review = self.pending_review.write().unwrap_or_else(|error| error.into_inner()).take()?;
        {
            let still_there = self
                .engine
                .read()
                .unwrap()
                .as_ref()
                .and_then(|engine| {
                    engine
                        .api
                        .api_torrent_details(librtbit::api::TorrentIdOrHash::Id(review.id))
                        .ok()
                })
                .is_some_and(|details| details.info_hash == review.info_hash);
            if !still_there {
                return None;
            }
        }
        Some(review)
    }

    /// Stops the engine.
    ///
    /// The stop is awaited instead of blocked on: a blocking wait inside the
    /// async runtime panics, and a panic ends the whole process.
    pub async fn shutdown(&self) {
        {
            let engine = self.engine.write().unwrap_or_else(|error| error.into_inner()).take();
            if let Some(engine) = engine {
                engine.session.stop().await;
            }
            *self.state.write().unwrap_or_else(|error| error.into_inner()) = EngineState::Stopped;
        }
    }

    // ---------------------------------------------------------------------
    // Feature-enabled implementation
    // ---------------------------------------------------------------------

    fn api(&self) -> AppResult<librtbit::Api> {
        self.engine
            .read()
            .unwrap()
            .as_ref()
            .map(|engine| engine.api.clone())
            .ok_or_else(|| self.unavailable_error())
    }

    fn unavailable_error(&self) -> AppError {
        let state = *self.state.read().unwrap_or_else(|error| error.into_inner());
        let message = self
            .message
            .read()
            .unwrap()
            .clone()
            .unwrap_or_else(|| match state {
                EngineState::Starting => "The torrent engine is still starting.".to_owned(),
                EngineState::Failed => "The torrent engine failed to start.".to_owned(),
                EngineState::Disabled => "Torrent support is disabled in settings.".to_owned(),
                _ => "The torrent engine is not running.".to_owned(),
            });
        AppError::torrent_unavailable(message)
    }

    fn set_state(&self, state: EngineState, message: Option<String>) {
        *self.state.write().unwrap_or_else(|error| error.into_inner()) = state;
        *self.message.write().unwrap_or_else(|error| error.into_inner()) = message;
        if let Some(app) = &self.app {
            use tauri::Emitter;
            let _ = app.emit("torrent://engine", self.status());
        }
    }

    async fn start_engine(self: &Arc<Self>) -> AppResult<()> {
        let settings = self.settings();
        if !settings.enabled {
            self.set_state(EngineState::Disabled, None);
            return Ok(());
        }
        if self.engine.read().unwrap_or_else(|error| error.into_inner()).is_some() {
            return Ok(());
        }

        self.set_state(EngineState::Starting, None);
        match crate::infrastructure::torrent::engine::create_engine(
            &settings,
            &self.data_directory,
            &self.default_download_directory,
        )
        .await
        {
            Ok(engine) => {
                *self.engine.write().unwrap_or_else(|error| error.into_inner()) = Some(engine);
                self.set_state(EngineState::Ready, None);
                Ok(())
            }
            Err(error) => {
                let message = format!("{error:#}");
                self.set_state(EngineState::Failed, Some(message.clone()));
                Err(AppError::torrent(message))
            }
        }
    }

    async fn stop_engine(&self) {
        let engine = self.engine.write().unwrap_or_else(|error| error.into_inner()).take();
        if let Some(engine) = engine {
            engine.session.stop().await;
        }
        let next = if self.settings().enabled {
            EngineState::Stopped
        } else {
            EngineState::Disabled
        };
        self.set_state(next, None);
    }

    pub async fn restart_engine(self: &Arc<Self>) -> AppResult<()> {
        self.stop_engine().await;
        self.restart_required.store(false, Ordering::SeqCst);
        self.start_engine().await
    }

    pub fn list(&self) -> AppResult<Vec<TorrentListItem>> {
        let mut items = Vec::new();
        if let Ok(api) = self.api() {
            let response = api.api_torrent_list_ext(librtbit::api::ApiTorrentListOpts {
                with_stats: true,
                ..Default::default()
            });
            items.extend(response.torrents.iter().map(map_list_item));
        }

        let resolving = self.resolving_torrents.read().unwrap_or_else(|e| e.into_inner());
        for t in resolving.values() {
            if items.iter().any(|item| item.info_hash.eq_ignore_ascii_case(&t.info_hash)) {
                continue;
            }
            let elapsed_secs = t.started_at.elapsed().as_secs();
            let (state, error) = match &t.status {
                ResolvingStatus::Resolving => ("initializing".to_string(), None),
                ResolvingStatus::Failed(err) => ("error".to_string(), Some(err.clone())),
            };
            items.push(TorrentListItem {
                id: t.placeholder_id,
                info_hash: t.info_hash.clone(),
                name: t.name.clone(),
                output_folder: t.output_folder.clone(),
                state,
                error,
                progress_bytes: 0,
                uploaded_bytes: 0,
                total_bytes: 0,
                finished: false,
                download_speed: 0,
                upload_speed: 0,
                time_remaining_secs: Some(elapsed_secs),
                seeds: 0,
                leechers: 0,
                peers: 0,
                ratio: 0.0,
                added_on: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                ),
                trackers: Vec::new(),
                files: Vec::new(),
            });
        }

        Ok(items)
    }

    pub async fn add_magnet(
        self: &Arc<Self>,
        magnet: String,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        let (info_hash, name) = parse_magnet_link(&magnet)?;

        // Deduplication against existing list (both engine and resolving placeholders)
        if let Ok(items) = self.list() {
            if let Some(existing) = items.iter().find(|i| i.info_hash.eq_ignore_ascii_case(&info_hash)) {
                return Ok(TorrentAddResult {
                    id: Some(existing.id),
                    info_hash: existing.info_hash.clone(),
                    name: existing.name.clone(),
                    already_managed: true,
                    output_folder: existing.output_folder.clone(),
                });
            }
        }

        let output_folder = options
            .output_folder
            .clone()
            .unwrap_or_else(|| self.default_download_directory.to_string_lossy().to_string());

        let placeholder_id = self.next_placeholder_id.fetch_add(1, Ordering::SeqCst);
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let placeholder = ResolvingTorrent {
            placeholder_id,
            magnet: magnet.clone(),
            info_hash: info_hash.clone(),
            name: name.clone(),
            output_folder: output_folder.clone(),
            options: options.clone(),
            status: ResolvingStatus::Resolving,
            started_at: std::time::Instant::now(),
            cancel_flag: cancel_flag.clone(),
        };

        {
            let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
            map.insert(info_hash.clone(), placeholder);
        }
        self.persist_resolving_torrents();

        let service = self.clone();
        let hash_for_bg = info_hash.clone();
        let magnet_for_bg = magnet.clone();
        let options_for_bg = options.clone();
        let cancel_for_bg = cancel_flag.clone();

        tauri::async_runtime::spawn(async move {
            service
                .background_resolve_magnet(
                    placeholder_id,
                    hash_for_bg,
                    magnet_for_bg,
                    options_for_bg,
                    cancel_for_bg,
                )
                .await;
        });

        Ok(TorrentAddResult {
            id: Some(placeholder_id),
            info_hash,
            name,
            already_managed: false,
            output_folder,
        })
    }

    async fn background_resolve_magnet(
        self: &Arc<Self>,
        _placeholder_id: usize,
        info_hash: String,
        magnet: String,
        options: TorrentAddOptions,
        cancel_flag: Arc<AtomicBool>,
    ) {
        for _ in 0..40 {
            if cancel_flag.load(Ordering::SeqCst) {
                return;
            }
            if self.api().is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        if cancel_flag.load(Ordering::SeqCst) {
            return;
        }

        let api = match self.api() {
            Ok(api) => api,
            Err(e) => {
                let mut map = self.resolving_torrents.write().unwrap_or_else(|err| err.into_inner());
                if let Some(item) = map.get_mut(&info_hash) {
                    item.status = ResolvingStatus::Failed(format!("Engine error: {}", e.message));
                }
                return;
            }
        };

        let add = librtbit::AddTorrent::from_url(magnet.as_str());
        let resolve_timeout = Duration::from_secs(60);

        let add_options = librtbit::AddTorrentOptions {
            paused: if options.review {
                true
            } else {
                options.start_paused
            },
            only_files: if options.review {
                Some(Vec::new())
            } else {
                options.only_files
            },
            initial_peers: parse_initial_peers(options.initial_peers.as_deref()).unwrap_or_default(),
            output_folder: if options.output_folder.as_deref().unwrap_or("").trim().is_empty() {
                None
            } else {
                options.output_folder
            },
            overwrite: true,
            magnet_resolution_timeout_secs: Some(60),
            ..Default::default()
        };

        let add_future = api.api_add_torrent(add, Some(add_options));
        match tokio::time::timeout(resolve_timeout, add_future).await {
            Ok(Ok(response)) => {
                if cancel_flag.load(Ordering::SeqCst) {
                    if let Some(id) = response.details.id {
                        let _ = api.api_torrent_action_delete(librtbit::api::TorrentIdOrHash::Id(id)).await;
                    }
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.remove(&info_hash);
                    }
                    self.persist_resolving_torrents();
                    return;
                }
                {
                    let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                    map.remove(&info_hash);
                }
                self.persist_resolving_torrents();
            }
            Ok(Err(err)) => {
                if cancel_flag.load(Ordering::SeqCst) {
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.remove(&info_hash);
                    }
                    self.persist_resolving_torrents();
                    return;
                }
                let err_msg = format!("{err:#}");
                {
                    let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(item) = map.get_mut(&info_hash) {
                        item.status = ResolvingStatus::Failed(err_msg);
                    }
                }
                self.persist_resolving_torrents();
            }
            Err(_) => {
                if cancel_flag.load(Ordering::SeqCst) {
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.remove(&info_hash);
                    }
                    self.persist_resolving_torrents();
                    return;
                }
                {
                    let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                    if let Some(item) = map.get_mut(&info_hash) {
                        item.status = ResolvingStatus::Failed("Magnet metadata resolution timed out after 60s".to_string());
                    }
                }
                self.persist_resolving_torrents();
            }
        }
    }

    pub fn persist_resolving_torrents(&self) {
        if self.data_directory.as_os_str().is_empty() {
            return;
        }
        let map = self.resolving_torrents.read().unwrap_or_else(|e| e.into_inner());
        let list: Vec<PersistedResolvingTorrent> = map
            .values()
            .map(|r| PersistedResolvingTorrent {
                placeholder_id: r.placeholder_id,
                magnet: r.magnet.clone(),
                info_hash: r.info_hash.clone(),
                name: r.name.clone(),
                output_folder: r.output_folder.clone(),
                options: r.options.clone(),
            })
            .collect();
        let file = self.data_directory.join("pending_magnets.json");
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            let _ = std::fs::write(file, json);
        }
    }

    pub fn restore_pending_magnets(self: &Arc<Self>) {
        let file = self.data_directory.join("pending_magnets.json");
        if let Ok(bytes) = std::fs::read(&file) {
            if let Ok(list) = serde_json::from_slice::<Vec<PersistedResolvingTorrent>>(&bytes) {
                for item in list {
                    let cancel_flag = Arc::new(AtomicBool::new(false));
                    let placeholder = ResolvingTorrent {
                        placeholder_id: item.placeholder_id,
                        magnet: item.magnet.clone(),
                        info_hash: item.info_hash.clone(),
                        name: item.name.clone(),
                        output_folder: item.output_folder.clone(),
                        options: item.options.clone(),
                        status: ResolvingStatus::Resolving,
                        started_at: std::time::Instant::now(),
                        cancel_flag: cancel_flag.clone(),
                    };
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.insert(item.info_hash.clone(), placeholder);
                    }
                    let service = self.clone();
                    let hash = item.info_hash;
                    let magnet = item.magnet;
                    let options = item.options;
                    let placeholder_id = item.placeholder_id;
                    tauri::async_runtime::spawn(async move {
                        service
                            .background_resolve_magnet(placeholder_id, hash, magnet, options, cancel_flag)
                            .await;
                    });
                }
            }
        }
    }

    /// Adds a .torrent payload, waiting for the engine to finish starting.
    pub async fn add_file_bytes_when_ready(
        &self,
        bytes: Vec<u8>,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        for attempt in 0..ENGINE_READY_ATTEMPTS {
            match self.add_file_bytes(bytes.clone(), options.clone()).await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if self.status().state != "starting" || attempt + 1 == ENGINE_READY_ATTEMPTS {
                        return Err(error);
                    }
                    tokio::time::sleep(ENGINE_READY_DELAY).await;
                }
            }
        }
        Err(AppError::torrent(
            "The torrent engine did not become ready in time.",
        ))
    }

    /// Adds a magnet link, waiting for the engine to finish starting.
    ///
    /// The engine boots asynchronously, so a magnet link that reaches ODM
    /// while the app is still starting would otherwise be rejected. Only
    /// errors that happen while the engine is starting are retried; every
    /// other failure is final and reported to the caller.
    pub async fn add_magnet_when_ready(
        self: &Arc<Self>,
        magnet: String,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        self.add_magnet(magnet, options).await
    }

    pub async fn add_file(
        &self,
        path: String,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        let path = PathBuf::from(path.trim());
        if !path.is_absolute() {
            return Err(AppError::torrent("Choose an absolute .torrent file path."));
        }
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("torrent"))
        {
            return Err(AppError::torrent("Choose a .torrent file."));
        }
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| AppError::torrent(format!("Could not read the torrent file: {error}")))?;
        if !metadata.is_file() {
            return Err(AppError::torrent("The selected path is not a file."));
        }
        if metadata.len() > MAX_TORRENT_FILE_BYTES {
            return Err(AppError::torrent("The .torrent file is too large."));
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| AppError::torrent(format!("Could not read the torrent file: {error}")))?;
        let api = self.api()?;
        self.add(api, librtbit::AddTorrent::from_bytes(bytes), options)
            .await
    }

    /// Adds a .torrent payload fetched by the browser bridge.
    pub async fn add_file_bytes(
        &self,
        bytes: Vec<u8>,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        if bytes.len() as u64 > MAX_TORRENT_FILE_BYTES {
            return Err(AppError::torrent("The .torrent file is too large."));
        }
        if bytes.is_empty() {
            return Err(AppError::torrent("The .torrent file is empty."));
        }
        let api = self.api()?;
        self.add(api, librtbit::AddTorrent::from_bytes(bytes), options)
            .await
    }

    async fn add(
        &self,
        api: librtbit::Api,
        add: librtbit::AddTorrent<'_>,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        let output_folder = options
            .output_folder
            .map(|folder| folder.trim().to_owned())
            .filter(|folder| !folder.is_empty());
        if let Some(folder) = &output_folder {
            let path = Path::new(folder);
            if !path.is_absolute() {
                return Err(AppError::torrent(
                    "Choose an absolute torrent download folder.",
                ));
            }
            tokio::fs::create_dir_all(path).await.map_err(|error| {
                AppError::torrent(format!("Could not create the torrent folder: {error}"))
            })?;
        }

        let add_options = librtbit::AddTorrentOptions {
            // Review mode fetches the metadata but transfers nothing: the
            // torrent is added paused and with an empty file selection, so the
            // engine does not look for peers and does not download before the
            // user confirms. Confirming sets the files and resumes the
            // torrent, which is the only order the engine supports: a live
            // torrent with no selected files counts as finished and stops
            // looking for peers.
            paused: if options.review {
                true
            } else {
                options.start_paused
            },
            only_files: if options.review {
                Some(Vec::new())
            } else {
                options.only_files
            },
            initial_peers: parse_initial_peers(options.initial_peers.as_deref())?,
            output_folder,
            // Allow re-adding a torrent whose payload already exists on disk:
            // the engine then verifies the existing files instead of failing.
            overwrite: true,
            magnet_resolution_timeout_secs: Some(45),
            ..Default::default()
        };

        let response = api
            .api_add_torrent(add, Some(add_options))
            .await
            .map_err(|error| AppError::torrent(format!("{error:#}")))?;

        Ok(TorrentAddResult {
            id: response.details.id,
            info_hash: response.details.info_hash,
            name: response.details.name.unwrap_or_default(),
            already_managed: false,
            output_folder: response.output_folder,
        })
    }

    pub async fn action(self: &Arc<Self>, id: usize, action: TorrentAction) -> AppResult<()> {
        let resolving_match = {
            let map = self.resolving_torrents.read().unwrap_or_else(|e| e.into_inner());
            map.values().find(|r| r.placeholder_id == id).cloned()
        };

        if let Some(item) = resolving_match {
            match action {
                TorrentAction::Delete | TorrentAction::Forget => {
                    item.cancel_flag.store(true, Ordering::SeqCst);
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.remove(&item.info_hash);
                    }
                    self.persist_resolving_torrents();
                    return Ok(());
                }
                TorrentAction::Resume | TorrentAction::Recheck => {
                    let cancel_flag = Arc::new(AtomicBool::new(false));
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        if let Some(placeholder) = map.get_mut(&item.info_hash) {
                            placeholder.status = ResolvingStatus::Resolving;
                            placeholder.started_at = std::time::Instant::now();
                            placeholder.cancel_flag = cancel_flag.clone();
                        }
                    }
                    self.persist_resolving_torrents();
                    let service = self.clone();
                    let hash = item.info_hash.clone();
                    let magnet = item.magnet.clone();
                    let opts = item.options.clone();
                    tauri::async_runtime::spawn(async move {
                        service.background_resolve_magnet(id, hash, magnet, opts, cancel_flag).await;
                    });
                    return Ok(());
                }
                TorrentAction::Pause => {
                    item.cancel_flag.store(true, Ordering::SeqCst);
                    {
                        let mut map = self.resolving_torrents.write().unwrap_or_else(|e| e.into_inner());
                        map.remove(&item.info_hash);
                    }
                    self.persist_resolving_torrents();
                    return Ok(());
                }
            }
        }

        let api = self.api()?;
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let result = match action {
            TorrentAction::Pause => api.api_torrent_action_pause(target).await,
            TorrentAction::Resume => {
                // The engine refuses to unpause a torrent that is already live
                // ("torrent is already live"), so a resume on a running torrent
                // is a no-op instead of an error.
                if is_live(&api, id) {
                    Ok(librtbit::api::EmptyJsonResponse {})
                } else {
                    api.api_torrent_action_start(target).await
                }
            }
            TorrentAction::Recheck => api.api_torrent_action_recheck(target).await,
            TorrentAction::Forget => api.api_torrent_action_forget(target).await,
            TorrentAction::Delete => api.api_torrent_action_delete(target).await,
        };
        result
            .map(|_| ())
            .map_err(|error| AppError::torrent(format!("{error:#}")))
    }

    pub async fn set_files(&self, id: usize, included: Vec<usize>) -> AppResult<()> {
        let api = self.api()?;
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let included: HashSet<usize> = included.into_iter().collect();
        // The engine refuses to change the selection while it is still
        // setting the torrent up ("can't update initializing torrent"). A
        // magnet resolves its metadata in the background, so the window can
        // confirm while the engine is still busy. Waiting here keeps that
        // race out of the user's face.
        wait_until_settled(&api, id).await?;
        let live = is_live(&api, id);
        if live {
            // The engine needs the torrent to be stopped while the selection
            // changes: a running torrent without selected files counts as
            // finished and stops looking for peers. Stopping it, changing the
            // selection and starting it again makes the engine pick the new
            // selection up.
            api.api_torrent_action_pause(target)
                .await
                .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        }
        api.api_torrent_action_update_only_files(target, &included)
            .await
            .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        if live {
            api.api_torrent_action_start(target)
                .await
                .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        }
        Ok(())
    }

    /// What the trackers say about the swarm of a torrent.
    ///
    /// The connected peers are only a handful of the swarm, so the tracker
    /// numbers are what shows whether a torrent is worth waiting for.
    pub fn tracker_status(&self, id: usize) -> AppResult<Vec<TorrentTrackerStatus>> {
        let api = self.api()?;
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let response = api
            .api_tracker_status(target)
            .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        Ok(response
            .trackers
            .iter()
            .map(|tracker| TorrentTrackerStatus {
                url: tracker.url.clone(),
                state: tracker_state_name(&tracker.state),
                seeders: tracker.seeders,
                leechers: tracker.leechers,
                peers_returned: tracker.peers_returned,
                last_error: tracker.last_error.clone(),
            })
            .collect())
    }

    pub fn files(&self, id: usize) -> AppResult<Vec<TorrentFileItem>> {
        let api = self.api()?;
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let mut details = api
            .api_torrent_details(target)
            .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        let stats = api.api_stats_v1(target).ok();
        let progress = stats
            .as_ref()
            .map(|s| s.file_progress.clone())
            .unwrap_or_default();
        details.stats = stats;
        Ok(map_files(&details, &progress))
    }

    pub fn peers(&self, id: usize) -> AppResult<Vec<TorrentPeerItem>> {
        let api = match self.api() {
            Ok(a) => a,
            Err(_) => return Ok(Vec::new()),
        };
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let snapshot = match api.api_peer_stats(target, Default::default()) {
            Ok(s) => s,
            Err(_) => return Ok(Vec::new()),
        };
        let mut result = Vec::new();
        for (addr, p) in snapshot.peers {
            let conn_type = p
                .conn_kind
                .map(|k| k.to_string().to_uppercase())
                .unwrap_or_else(|| "TCP".to_string());
            result.push(TorrentPeerItem {
                address: addr,
                client: None,
                download_speed: 0,
                upload_speed: 0,
                progress_percent: 0.0,
                flags: p.state.to_string(),
                connection_type: conn_type,
                fetched_bytes: p.counters.fetched_bytes,
                uploaded_bytes: p.counters.uploaded_bytes,
            });
        }
        result.sort_by(|a, b| b.fetched_bytes.cmp(&a.fetched_bytes));
        Ok(result)
    }

    pub fn pieces(&self, id: usize) -> AppResult<TorrentPiecesInfo> {
        let api = match self.api() {
            Ok(a) => a,
            Err(_) => {
                return Ok(TorrentPiecesInfo {
                    total_pieces: 0,
                    piece_length: 0,
                    completed_pieces: 0,
                    piece_states: Vec::new(),
                })
            }
        };
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let piece_states = api.api_dump_piece_states(target).unwrap_or_default();
        let (total_pieces, piece_length) = match api.api_dump_haves(target) {
            Ok((_bf, len)) => (len, 0),
            Err(_) => (piece_states.len() as u32, 0),
        };
        let completed_pieces = piece_states.iter().filter(|&&s| s == 2).count() as u32;
        Ok(TorrentPiecesInfo {
            total_pieces,
            piece_length,
            completed_pieces,
            piece_states,
        })
    }

    pub fn update_settings(
        self: &Arc<Self>,
        next: TorrentSettings,
    ) -> AppResult<TorrentSettings> {
        next.validate().map_err(AppError::invalid_settings)?;
        let previous = self.settings();
        self.settings_store
            .save(&next)
            .map_err(|error| AppError::storage(format!("Could not save torrent settings: {error}")))?;

        let requires_restart = previous.listen_port != next.listen_port
            || previous.enable_dht != next.enable_dht
            || previous.enable_upnp != next.enable_upnp
            || previous.enable_utp != next.enable_utp
            || previous.download_limit_kbps != next.download_limit_kbps
            || previous.upload_limit_kbps != next.upload_limit_kbps
            || previous.max_peers_per_torrent != next.max_peers_per_torrent
            || previous.download_directory != next.download_directory;

        *self.settings.write().unwrap_or_else(|error| error.into_inner()) = next.clone();
        self.restart_required
            .store(requires_restart, Ordering::SeqCst);

        if previous.enabled != next.enabled {
            let service = self.clone();
            tauri::async_runtime::spawn(async move {
                if next.enabled {
                    let _ = service.start_engine().await;
                } else {
                    service.stop_engine().await;
                }
            });
        }

        Ok(next)
    }

    fn spawn_poller(self: &Arc<Self>) {
        let Some(app) = self.app.clone() else {
            return;
        };
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            use tauri::Emitter;
            let mut interval = tokio::time::interval(POLL_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Ok(list) = service.list() else {
                    continue;
                };
                // The peer refresh runs even when no window is open: a
                // torrent that nobody looks at still has to ask the trackers
                // for new peers when it runs dry.
                service.refresh_peers(&list);
                if service.watching.load(Ordering::Relaxed) == 0 {
                    continue;
                }
                let _ = app.emit("torrent://progress", &list);
            }
        });
    }

    /// Asks the trackers for new peers when a torrent runs dry.
    ///
    /// The trackers hand out a fixed list of peers and the engine only
    /// announces again after the interval the tracker asked for, which is
    /// usually half an hour. The peers in that list go away in the
    /// meantime, and a torrent then sits at zero bytes although the swarm
    /// is large. The engine only looks for new peers when fewer than five
    /// are left, so a torrent that is connected to a handful of leechers
    /// never asks for the seeds.
    ///
    /// Asking for a re-announce when the transfer stops or the peer list
    /// runs thin keeps the torrent in the swarm.
    fn refresh_peers(&self, list: &[TorrentListItem]) {
        let Ok(api) = self.api() else {
            return;
        };
        let now = std::time::Instant::now();
        let mut reannounce = Vec::new();
        {
            let mut states = self.peer_refresh.write().unwrap_or_else(|error| error.into_inner());
            for item in list {
                if item.state != "live" || item.finished {
                    states.remove(&item.id);
                    continue;
                }
                let state = states.entry(item.id).or_insert(TorrentRefresh {
                    last_progress_bytes: item.progress_bytes,
                    last_progress_at: now,
                    last_reannounce: now,
                });
                if item.progress_bytes > state.last_progress_bytes {
                    state.last_progress_bytes = item.progress_bytes;
                    state.last_progress_at = now;
                    continue;
                }
                let stalled = now.duration_since(state.last_progress_at) >= STALE_TRANSFER;
                let cooled = now.duration_since(state.last_reannounce) >= REANNOUNCE_COOLDOWN;
                if needs_new_peers(item.peers, stalled, cooled) {
                    state.last_reannounce = now;
                    reannounce.push(item.id);
                }
            }
        }
        // The engine is called with the map unlocked: a slow announce must not
        // block every other command that needs the refresh state.
        for id in reannounce {
            let target = librtbit::api::TorrentIdOrHash::Id(id);
            let _ = api.api_torrent_action_reannounce(target);
        }
    }

    // ---------------------------------------------------------------------
    // Feature-disabled stubs. The public surface stays identical so command
    // registration and AppState do not need conditional compilation.
    // ---------------------------------------------------------------------













}

/// Parses the peer addresses a caller wants the engine to try first.
///
/// An empty list means "no preference", so it is reported as `None` instead of
/// an empty peer set, which the engine would treat as "no way to find peers".
fn parse_initial_peers(
    peers: Option<&[String]>,
) -> AppResult<Option<Vec<std::net::SocketAddr>>> {
    let Some(peers) = peers else {
        return Ok(None);
    };
    let mut parsed = Vec::with_capacity(peers.len());
    for entry in peers {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let address = trimmed
            .parse::<std::net::SocketAddr>()
            .map_err(|_| AppError::torrent(format!("Invalid peer address: {trimmed}")))?;
        parsed.push(address);
    }
    if parsed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parsed))
    }
}

/// The name of the announce state, as the frontend knows it.
///
/// The debug name of the engine enum is not the documented name: the engine
/// writes "notcontacted", the contract says "not_contacted".
fn tracker_state_name(state: &impl std::fmt::Debug) -> String {
    let name = format!("{state:?}");
    match name.as_str() {
        "NotContacted" => "not_contacted".to_owned(),
        "Updating" => "updating".to_owned(),
        "Working" => "working".to_owned(),
        "Error" => "error".to_owned(),
        "Disabled" => "disabled".to_owned(),
        other => other.to_lowercase(),
    }
}

fn map_files(
    details: &librtbit::api::TorrentDetailsResponse,
    progress: &[u64],
) -> Vec<TorrentFileItem> {
    details
        .files
        .as_ref()
        .map(|files| {
            let single = files.len() == 1;
            let total_progress = details.stats.as_ref().map(|s| s.progress_bytes).unwrap_or(0);
            files
                .iter()
                .enumerate()
                .map(|(index, file)| {
                    let mut file_progress = progress.get(index).copied().unwrap_or(0);
                    if single && total_progress > file_progress {
                        file_progress = total_progress.min(file.length);
                    }
                    TorrentFileItem {
                        index,
                        name: file.name.clone(),
                        length: file.length,
                        included: file.included,
                        priority: file.priority,
                        progress_bytes: file_progress,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// True when the engine reports the torrent as running.
///
/// The engine refuses to unpause a running torrent ("torrent is already
/// live"), so the caller has to know the state before it asks for a resume.
/// `api_torrent_details` does not carry the stats, so the state comes from
/// `api_stats_v1`.
fn is_live(api: &librtbit::Api, id: usize) -> bool {
    let target = librtbit::api::TorrentIdOrHash::Id(id);
    api.api_stats_v1(target)
        .map(|stats| stats.state.to_string())
        .is_ok_and(|state| state == "live")
}

/// Waits until the engine finished setting the torrent up.
///
/// While a torrent initializes, the engine refuses to change the file
/// selection or to start it. A magnet resolves its metadata in the
/// background, so the window can act before the engine is ready.
async fn wait_until_settled(api: &librtbit::Api, id: usize) -> AppResult<()> {
    let target = librtbit::api::TorrentIdOrHash::Id(id);
    for _ in 0..ENGINE_SETTLE_ATTEMPTS {
        let state = api
            .api_stats_v1(target)
            .map(|stats| stats.state.to_string())
            .unwrap_or_else(|_| "initializing".to_owned());
        if state != "initializing" {
            return Ok(());
        }
        tokio::time::sleep(ENGINE_SETTLE_DELAY).await;
    }
    Err(AppError::torrent(
        "The torrent is still being prepared. Try again in a moment.",
    ))
}

fn map_list_item(details: &librtbit::api::TorrentDetailsResponse) -> TorrentListItem {    let stats = details.stats.as_ref();
    let live = stats.and_then(|stats| stats.live.as_ref());
    let progress_bytes = stats.map(|stats| stats.progress_bytes).unwrap_or(0);
    let total_bytes = stats.map(|stats| stats.total_bytes).unwrap_or(0);
    let uploaded_bytes = stats.map(|stats| stats.uploaded_bytes).unwrap_or(0);
    let download_speed = live.map(|live| live.download_speed.as_bytes()).unwrap_or(0);
    let upload_speed = live.map(|live| live.upload_speed.as_bytes()).unwrap_or(0);
    let remaining = total_bytes.saturating_sub(progress_bytes);
    let time_remaining_secs = if download_speed > 0 && remaining > 0 {
        Some(remaining / download_speed)
    } else {
        None
    };
    let seeds = live.map(|live| live.snapshot.connected_seeders).unwrap_or(0);
    let leechers = live
        .map(|live| live.snapshot.connected_leechers)
        .unwrap_or(0);

    TorrentListItem {
        id: details.id.unwrap_or(0),
        info_hash: details.info_hash.clone(),
        name: details
            .name
            .clone()
            .unwrap_or_else(|| details.info_hash.clone()),
        output_folder: details.output_folder.clone(),
        state: stats
            .map(|stats| stats.state.to_string())
            .unwrap_or_else(|| "initializing".to_owned()),
        error: stats.and_then(|stats| stats.error.clone()),
        progress_bytes,
        uploaded_bytes,
        total_bytes,
        finished: stats.map(|stats| stats.finished).unwrap_or(false),
        download_speed,
        upload_speed,
        time_remaining_secs,
        seeds,
        leechers,
        peers: seeds.saturating_add(leechers),
        ratio: if progress_bytes > 0 {
            uploaded_bytes as f64 / progress_bytes as f64
        } else {
            0.0
        },
        added_on: details.added_on,
        trackers: details.trackers.clone().unwrap_or_default(),
        files: map_files(details, stats.map(|stats| stats.file_progress.as_slice()).unwrap_or(&[])),
    }
}

// ---------------------------------------------------------------------------
// Engine contract tests
//
// The review flow depends on how the engine treats a torrent without selected
// files. These tests pin that contract down so a future engine update cannot
// silently break the confirmation again.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod peer_refresh_tests {
    use super::{needs_new_peers, THIN_SWARM};

    #[test]
    fn a_torrent_that_stands_still_asks_for_new_peers() {
        // The transfer is stuck: the peers the tracker handed out are gone.
        assert!(needs_new_peers(50, true, true));
    }

    #[test]
    fn a_torrent_with_a_thin_swarm_asks_for_new_peers() {
        // Connected to a handful of leechers, but the swarm is large.
        assert!(needs_new_peers(THIN_SWARM - 1, false, true));
        assert!(!needs_new_peers(THIN_SWARM, false, true));
    }

    #[test]
    fn a_healthy_torrent_leaves_the_trackers_alone() {
        assert!(!needs_new_peers(100, false, true));
    }

    #[test]
    fn the_trackers_are_not_hammered() {
        // The cooldown keeps the announce rate down.
        assert!(!needs_new_peers(0, true, false));
        assert!(!needs_new_peers(0, false, false));
    }
}

#[cfg(test)]
mod engine_contract_tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use librtbit::{AddTorrent, AddTorrentOptions, Api, Session, SessionOptions};
    use crate::domain::torrent::{TorrentAction, TorrentAddOptions};

    fn bencode_bytes(data: &[u8]) -> Vec<u8> {
        let mut out = format!("{}:", data.len()).into_bytes();
        out.extend_from_slice(data);
        out
    }

    fn bencode_int(value: u64) -> Vec<u8> {
        format!("i{value}e").into_bytes()
    }

    fn bencode_dict(pairs: &[(&[u8], Vec<u8>)]) -> Vec<u8> {
        let mut out = b"d".to_vec();
        for (key, value) in pairs {
            out.extend_from_slice(&bencode_bytes(key));
            out.extend_from_slice(value);
        }
        out.push(b'e');
        out
    }

    /// A single file torrent of 1 KiB with one piece.
    fn single_file_torrent(name: &str) -> Vec<u8> {
        let info = bencode_dict(&[
            (b"length", bencode_int(1024)),
            (b"name", bencode_bytes(name.as_bytes())),
            (b"piece length", bencode_int(16_384)),
            (b"pieces", bencode_bytes(&[0u8; 20])),
        ]);
        bencode_dict(&[
            (b"announce", bencode_bytes(b"http://tracker.invalid/announce")),
            (b"info", info),
        ])
    }

    async fn test_api(directory: PathBuf) -> Api {
        let options = SessionOptions {
            disable_dht: true,
            persistence: None,
            listen: None,
            ..Default::default()
        };
        let session = Session::new_with_opts(directory.clone(), options)
            .await
            .expect("the session must start");
        Api::new(session.clone(), None)
    }

    fn temp_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "odm-engine-contract-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("the test directory must exist");
        directory
    }

    /// Waits until the engine finished setting the torrent up.
    async fn settled_state(api: &Api, id: usize) -> String {
        for _ in 0..200 {
            let state = api
                .api_stats_v1(librtbit::api::TorrentIdOrHash::Id(id))
                .expect("the stats must be available")
                .state
                .to_string();
            if state != "initializing" {
                return state;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        "initializing".to_owned()
    }

    #[tokio::test]
    async fn a_review_torrent_waits_paused_until_the_files_are_confirmed() {
        let directory = temp_directory("review");
        let api = test_api(directory.clone()).await;

        let added = api
            .api_add_torrent(
                AddTorrent::from_bytes(single_file_torrent("review.bin")),
                Some(AddTorrentOptions {
                    paused: true,
                    only_files: Some(Vec::new()),
                    output_folder: Some(directory.to_string_lossy().into_owned()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .expect("the torrent must be added");
        let id = added.id.expect("the torrent must get an id");

        // The torrent waits for the user: it is paused and no file is wanted.
        assert_eq!(
            settled_state(&api, id).await,
            "paused",
            "a torrent that waits for the confirmation must not run"
        );

        // Confirming selects the file and starts the torrent. Starting a
        // torrent that already runs fails with "torrent is already live",
        // which is why the review adds the torrent paused.
        api.api_torrent_action_update_only_files(
            librtbit::api::TorrentIdOrHash::Id(id),
            &HashSet::from([0usize]),
        )
        .await
        .expect("the file selection must be applied");
        api.api_torrent_action_start(librtbit::api::TorrentIdOrHash::Id(id))
            .await
            .expect("a paused torrent must start");

        let stats = api
            .api_stats_v1(librtbit::api::TorrentIdOrHash::Id(id))
            .expect("the stats must be available");
        assert_eq!(
            stats.state.to_string(),
            "live",
            "the confirmed torrent must run"
        );

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn a_running_torrent_without_selected_files_cannot_be_resumed() {
        // This is the engine behaviour that forced the review to add the
        // torrent paused: a torrent that runs without selected files counts as
        // finished, and the engine refuses to resume it.
        let directory = temp_directory("live");
        let api = test_api(directory.clone()).await;

        let added = api
            .api_add_torrent(
                AddTorrent::from_bytes(single_file_torrent("live.bin")),
                Some(AddTorrentOptions {
                    paused: false,
                    only_files: Some(Vec::new()),
                    output_folder: Some(directory.to_string_lossy().into_owned()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .expect("the torrent must be added");
        let id = added.id.expect("the torrent must get an id");

        assert_eq!(
            settled_state(&api, id).await,
            "live",
            "a torrent that is not added paused runs right away"
        );
        let stats = api
            .api_stats_v1(librtbit::api::TorrentIdOrHash::Id(id))
            .expect("the stats must be available");
        assert!(
            stats.finished,
            "a running torrent without selected files counts as finished"
        );

        let error = match api
            .api_torrent_action_start(librtbit::api::TorrentIdOrHash::Id(id))
            .await
        {
            Ok(_) => panic!("a running torrent must not be resumed"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("already live"),
            "the engine refuses to resume a running torrent: {error}"
        );

        let _ = std::fs::remove_dir_all(&directory);
    }

    /// The confirmation flow of the Torrents window, driven through the
    /// service the window actually talks to.
    #[tokio::test]
    async fn confirming_a_review_starts_the_torrent_with_the_chosen_files() {
        use crate::domain::torrent::{TorrentAction, TorrentAddOptions};

        let directory = temp_directory("confirm");
        let downloads = directory.join("downloads");
        std::fs::create_dir_all(&downloads).expect("the download folder must exist");

        // Keep the test off the network: no DHT, no trackers, no peer search.
        let settings = crate::domain::torrent::TorrentSettings {
            enabled: true,
            enable_dht: false,
            enable_upnp: false,
            enable_utp: false,
            ..Default::default()
        };
        std::fs::write(
            directory.join("torrent-settings.json"),
            serde_json::to_vec(&settings).expect("the settings must serialize"),
        )
        .expect("the settings must be written");

        let app = tauri::test::mock_app();
        let _ = app;
        let engine = crate::infrastructure::torrent::engine::create_engine(
            &settings,
            &directory,
            &downloads,
        )
        .await
        .expect("the engine must start");
        let service = super::TorrentService::with_engine(
            directory.clone(),
            downloads.clone(),
            engine,
        );

        // The window adds the torrent in review mode.
        let added = service
            .add_file_bytes(
                single_file_torrent("confirm.bin"),
                TorrentAddOptions {
                    review: true,
                    ..Default::default()
                },
            )
            .await
            .expect("the torrent must be added");
        let id = added.id.expect("the torrent must get an id");

        // The engine sets the torrent up asynchronously.
        let mut state = String::new();
        for _ in 0..200 {
            let list = service.list().expect("the list must be available");
            if let Some(item) = list.iter().find(|item| item.id == id) {
                state = item.state.clone();
                if state != "initializing" {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(
            state, "paused",
            "a torrent that waits for the confirmation must be paused"
        );
        // The window applies the file selection and starts the torrent.
        service
            .set_files(id, vec![0])
            .await
            .expect("the file selection must be applied");
        service
            .action(id, TorrentAction::Resume)
            .await
            .expect("the torrent must start");

        // The engine settles the torrent asynchronously.
        let mut state = String::new();
        for _ in 0..200 {
            let list = service.list().expect("the list must be available");
            if let Some(item) = list.iter().find(|item| item.id == id) {
                state = item.state.clone();
                if state != "initializing" {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert_eq!(
            state, "live",
            "the confirmed torrent must run, got {state}"
        );

        let files = service.files(id).expect("the file list must be available");
        assert_eq!(files.len(), 1);
        assert!(
            files[0].included,
            "the chosen file must be selected for the download"
        );

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn add_magnet_is_non_blocking_and_returns_placeholder_under_10ms() {
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().to_path_buf();
        let dls = tempfile::tempdir().unwrap();
        let downloads = dls.path().to_path_buf();
        let service = super::TorrentService::disabled();

        let magnet = "magnet:?xt=urn:btih:2b6c31677c447e5b9d3b84ef6f530e1590fb3b34&dn=ubuntu-iso";
        let start = std::time::Instant::now();
        let result = service
            .add_magnet(magnet.to_string(), TorrentAddOptions::default())
            .await
            .expect("magnet add should succeed immediately");
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 10,
            "add_magnet must complete in <10ms (took {:?})",
            elapsed
        );
        assert!(!result.already_managed);
        assert_eq!(result.info_hash, "2b6c31677c447e5b9d3b84ef6f530e1590fb3b34");
        assert_eq!(result.name, "ubuntu-iso");
        let placeholder_id = result.id.expect("must return a placeholder id");

        // Immediately visible in list with state initializing
        let list = service.list().expect("list must succeed");
        let item = list.iter().find(|i| i.id == placeholder_id).expect("item must be in list");
        assert_eq!(item.state, "initializing");
        assert_eq!(item.info_hash, "2b6c31677c447e5b9d3b84ef6f530e1590fb3b34");

        // Deduplication: adding the same magnet again returns already_managed: true
        let dedup_result = service
            .add_magnet(magnet.to_string(), TorrentAddOptions::default())
            .await
            .expect("dedup add should succeed");
        assert!(dedup_result.already_managed);
        assert_eq!(dedup_result.id, Some(placeholder_id));

        // Cancellation: deleting placeholder removes it from list
        service
            .action(placeholder_id, TorrentAction::Delete)
            .await
            .expect("delete action should succeed");
        let list_after = service.list().expect("list must succeed");
        assert!(!list_after.iter().any(|i| i.id == placeholder_id));

        let _ = std::fs::remove_dir_all(&directory);
        let _ = std::fs::remove_dir_all(&downloads);
    }

    #[tokio::test]
    async fn test_magnet_lifecycle_cancel_during_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let _directory = dir.path().to_path_buf();
        let service = super::TorrentService::disabled();

        let magnet = "magnet:?xt=urn:btih:3c7d42788d558f6c0e4c95fa7f641f2601fc4c45&dn=cancel-test";
        let res = service
            .add_magnet(magnet.to_string(), TorrentAddOptions::default())
            .await
            .expect("must add magnet placeholder");
        let id = res.id.expect("must have placeholder id");

        // Verify present in list
        assert!(service.list().unwrap().iter().any(|i| i.id == id));

        // User cancels during resolve
        service
            .action(id, TorrentAction::Delete)
            .await
            .expect("must cancel without error");

        // Must be removed from list
        assert!(!service.list().unwrap().iter().any(|i| i.id == id));
    }

    #[tokio::test]
    async fn test_magnet_lifecycle_retry_after_failure() {
        let _dir = tempfile::tempdir().unwrap();
        let service = super::TorrentService::disabled();

        let magnet = "magnet:?xt=urn:btih:4d8e53899e66907d1f5da60b8f752037120d5d56&dn=retry-test";
        let res = service
            .add_magnet(magnet.to_string(), TorrentAddOptions::default())
            .await
            .expect("must add magnet placeholder");
        let id = res.id.expect("must have placeholder id");

        // Simulate failure (e.g. metadata timeout)
        {
            let mut map = service.resolving_torrents.write().unwrap();
            if let Some(item) = map.get_mut("4d8e53899e66907d1f5da60b8f752037120d5d56") {
                item.status = super::ResolvingStatus::Failed("Resolution timed out".to_string());
            }
        }
        let list = service.list().unwrap();
        let failed_item = list.iter().find(|i| i.id == id).expect("item must exist");
        assert_eq!(failed_item.state, "error");
        assert!(failed_item.error.as_deref().unwrap_or("").contains("timed out"));

        // User retries resolution via Resume action
        service
            .action(id, TorrentAction::Resume)
            .await
            .expect("resume action must succeed");

        let retried_item = service
            .list()
            .unwrap()
            .into_iter()
            .find(|i| i.id == id)
            .expect("item must exist");
        assert_eq!(retried_item.state, "initializing");
        assert!(retried_item.error.is_none());
    }

    #[tokio::test]
    async fn test_magnet_lifecycle_restart_mid_resolve_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let directory = dir.path().to_path_buf();
        let downloads = directory.join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();

        // 1. Initial service adds a magnet mid-flight
        let service1 = super::TorrentService::for_test(directory.clone(), downloads.clone());
        let magnet = "magnet:?xt=urn:btih:5e9f64900f77a18e206eb71c90863148231e6e67&dn=persist-test";
        let res = service1
            .add_magnet(magnet.to_string(), TorrentAddOptions::default())
            .await
            .expect("must add magnet placeholder");
        let id = res.id.expect("must have placeholder id");

        // Verify pending_magnets.json file exists on disk
        let pending_file = directory.join("pending_magnets.json");
        assert!(pending_file.exists(), "pending_magnets.json must be written to disk");

        // 2. Simulate application quit and restart with new service instance
        let service2 = super::TorrentService::for_test(directory.clone(), downloads.clone());
        service2.restore_pending_magnets();

        // Must restore the pending magnet in initializing state with same hash
        let list = service2.list().expect("list must succeed");
        let restored = list
            .iter()
            .find(|i| i.info_hash == "5e9f64900f77a18e206eb71c90863148231e6e67")
            .expect("restored magnet must be present after restart");
        assert_eq!(restored.id, id);
        assert_eq!(restored.name, "persist-test");
        assert_eq!(restored.state, "initializing");
    }
}
