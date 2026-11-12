// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
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
#[cfg(feature = "torrent")]
use std::{
    collections::HashSet,
    path::Path,
    time::Duration,
};

use crate::{
    application::error::{AppError, AppResult},
    domain::torrent::{
        TorrentAction, TorrentAddOptions, TorrentAddResult, TorrentEngineStatus, TorrentFileItem,
        TorrentListItem, TorrentReviewRequest, TorrentSettings, TorrentTrackerStatus,
    },
    infrastructure::torrent::settings_store::TorrentSettingsStore,
};

#[cfg(feature = "torrent")]
const MAX_MAGNET_LENGTH: usize = 16 * 1024;
#[cfg(feature = "torrent")]
const MAX_TORRENT_FILE_BYTES: u64 = 32 * 1024 * 1024;
#[cfg(feature = "torrent")]
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How many times an add is retried while the engine is still starting.
#[cfg(feature = "torrent")]
const ENGINE_READY_ATTEMPTS: usize = 40;

/// Pause between two attempts while the engine is still starting.
#[cfg(feature = "torrent")]
const ENGINE_READY_DELAY: Duration = Duration::from_millis(250);

/// How long the engine may take to finish setting a torrent up before the
/// window gives up on changing its file selection.
#[cfg(feature = "torrent")]
const ENGINE_SETTLE_ATTEMPTS: usize = 240;

/// Pause between two attempts while the engine sets a torrent up.
#[cfg(feature = "torrent")]
const ENGINE_SETTLE_DELAY: Duration = Duration::from_millis(250);

/// A torrent that receives nothing for this long asks the trackers for new
/// peers.
#[cfg(feature = "torrent")]
const STALE_TRANSFER: Duration = Duration::from_secs(45);

/// Fewer connected peers than this means the torrent needs a fresh peer
/// list.
#[cfg(feature = "torrent")]
const THIN_SWARM: u32 = 10;

/// The shortest time between two re-announces for one torrent. Trackers do
/// not like being hammered.
#[cfg(feature = "torrent")]
const REANNOUNCE_COOLDOWN: Duration = Duration::from_secs(60);

/// Decides whether a torrent should ask the trackers for new peers.
///
/// A tracker hands out a fixed list of peers and then asks to be left alone
/// for the next half an hour. The engine only announces again when fewer
/// than five peers are left, so a torrent that is connected to a handful of
/// leechers never asks for the seeds. Asking again keeps the torrent in the
/// swarm.
#[cfg(feature = "torrent")]
fn needs_new_peers(peers: u32, stalled: bool, cooled: bool) -> bool {
    if !cooled {
        return false;
    }
    stalled || peers < THIN_SWARM
}

/// What the service remembers about a running torrent to decide whether it
/// needs new peers.
#[cfg(feature = "torrent")]
struct TorrentRefresh {
    last_progress_bytes: u64,
    last_progress_at: std::time::Instant,
    last_reannounce: std::time::Instant,
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
    #[cfg(feature = "torrent")]
    peer_refresh: RwLock<std::collections::HashMap<usize, TorrentRefresh>>,
    #[cfg(feature = "torrent")]
    engine: RwLock<Option<crate::infrastructure::torrent::engine::TorrentEngine>>,
    #[cfg(feature = "torrent")]
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
            #[cfg(feature = "torrent")]
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            #[cfg(feature = "torrent")]
            engine: RwLock::new(None),
            #[cfg(feature = "torrent")]
            app: None,
        })
    }

    /// Builds a service around an already running engine.
    ///
    /// The production path is [`TorrentService::start`], which needs a Tauri
    /// application handle. The tests only need the engine, so they use this
    /// constructor instead of building a whole application.
    #[cfg(all(test, feature = "torrent"))]
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
            engine: RwLock::new(Some(engine)),
            app: None,
        })
    }

    /// Builds the service, loads torrent settings, and starts the engine in the
    /// background so application startup is never blocked by network work.
    #[cfg(feature = "torrent")]
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
            #[cfg(feature = "torrent")]
            peer_refresh: RwLock::new(std::collections::HashMap::new()),
            engine: RwLock::new(None),
            app: Some(app),
        });

        let starter = service.clone();
        tauri::async_runtime::spawn(async move {
            let _ = starter.start_engine().await;
        });
        service.spawn_poller();
        service
    }

    #[cfg(not(feature = "torrent"))]
    pub fn start(
        _app: tauri::AppHandle,
        _data_directory: PathBuf,
        _default_download_directory: PathBuf,
    ) -> Arc<Self> {
        Self::disabled()
    }

    pub fn status(&self) -> TorrentEngineStatus {
        let settings = self.settings();
        let state = *self.state.read().unwrap();
        let listen_port = {
            #[cfg(feature = "torrent")]
            {
                self.engine
                    .read()
                    .unwrap()
                    .as_ref()
                    .map(|engine| engine.api.api_listen_port())
            }
            #[cfg(not(feature = "torrent"))]
            {
                None
            }
        };
        TorrentEngineStatus {
            available: cfg!(feature = "torrent"),
            enabled: settings.enabled,
            state: state.as_str().to_owned(),
            message: self.message.read().unwrap().clone(),
            listen_port,
            dht_enabled: settings.enable_dht,
            restart_required: self.restart_required.load(Ordering::SeqCst),
        }
    }

    pub fn settings(&self) -> TorrentSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn set_watching(&self, watching: bool) {
        self.watching.store(usize::from(watching), Ordering::Relaxed);
    }

    /// Remembers a torrent added by the browser so the window can show its
    /// file picker even if it finishes loading after the event was emitted.
    pub fn set_pending_review(&self, review: Option<TorrentReviewRequest>) {
        *self.pending_review.write().unwrap() = review;
    }

    /// Takes the pending review, if there still is one.
    ///
    /// The engine hands the same id to the next torrent once the list is
    /// empty, so a pending review can point at a torrent that no longer
    /// exists. The info hash tells the two apart, and a review that belongs
    /// to a deleted torrent is dropped instead of being shown for a
    /// different download.
    pub fn take_pending_review(&self) -> Option<TorrentReviewRequest> {
        let review = self.pending_review.write().unwrap().take()?;
        #[cfg(feature = "torrent")]
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

    pub fn shutdown(&self) {
        #[cfg(feature = "torrent")]
        {
            let engine = self.engine.write().unwrap().take();
            if let Some(engine) = engine {
                tauri::async_runtime::block_on(engine.session.stop());
            }
            *self.state.write().unwrap() = EngineState::Stopped;
        }
    }

    // ---------------------------------------------------------------------
    // Feature-enabled implementation
    // ---------------------------------------------------------------------

    #[cfg(feature = "torrent")]
    fn api(&self) -> AppResult<librtbit::Api> {
        self.engine
            .read()
            .unwrap()
            .as_ref()
            .map(|engine| engine.api.clone())
            .ok_or_else(|| self.unavailable_error())
    }

    #[cfg(feature = "torrent")]
    fn unavailable_error(&self) -> AppError {
        let state = *self.state.read().unwrap();
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

    #[cfg(feature = "torrent")]
    fn set_state(&self, state: EngineState, message: Option<String>) {
        *self.state.write().unwrap() = state;
        *self.message.write().unwrap() = message;
        if let Some(app) = &self.app {
            use tauri::Emitter;
            let _ = app.emit("torrent://engine", self.status());
        }
    }

    #[cfg(feature = "torrent")]
    async fn start_engine(self: &Arc<Self>) -> AppResult<()> {
        let settings = self.settings();
        if !settings.enabled {
            self.set_state(EngineState::Disabled, None);
            return Ok(());
        }
        if self.engine.read().unwrap().is_some() {
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
                *self.engine.write().unwrap() = Some(engine);
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

    #[cfg(feature = "torrent")]
    async fn stop_engine(&self) {
        let engine = self.engine.write().unwrap().take();
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

    #[cfg(feature = "torrent")]
    pub async fn restart_engine(self: &Arc<Self>) -> AppResult<()> {
        self.stop_engine().await;
        self.restart_required.store(false, Ordering::SeqCst);
        self.start_engine().await
    }

    #[cfg(feature = "torrent")]
    pub fn list(&self) -> AppResult<Vec<TorrentListItem>> {
        let api = self.api()?;
        let response = api.api_torrent_list_ext(librtbit::api::ApiTorrentListOpts {
            with_stats: true,
            ..Default::default()
        });
        Ok(response.torrents.iter().map(map_list_item).collect())
    }

    #[cfg(feature = "torrent")]
    pub async fn add_magnet(
        &self,
        magnet: String,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        let magnet = magnet.trim().to_owned();
        if !magnet.starts_with("magnet:?") {
            return Err(AppError::torrent("Enter a valid magnet link."));
        }
        if magnet.len() > MAX_MAGNET_LENGTH {
            return Err(AppError::torrent("The magnet link is too long."));
        }
        if !magnet.to_ascii_lowercase().contains("xt=urn:btih:") {
            return Err(AppError::torrent(
                "The magnet link does not contain a BitTorrent info hash.",
            ));
        }
        let api = self.api()?;
        let add = librtbit::AddTorrent::from_url(magnet.as_str());
        self.add(api, add, options).await
    }

    /// Adds a .torrent payload, waiting for the engine to finish starting.
    #[cfg(feature = "torrent")]
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
    #[cfg(feature = "torrent")]
    pub async fn add_magnet_when_ready(
        &self,
        magnet: String,
        options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        for attempt in 0..ENGINE_READY_ATTEMPTS {
            match self.add_magnet(magnet.clone(), options.clone()).await {
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

    #[cfg(feature = "torrent")]
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
        let metadata = std::fs::metadata(&path)
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
    #[cfg(feature = "torrent")]
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

    #[cfg(feature = "torrent")]
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
            std::fs::create_dir_all(path).map_err(|error| {
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

    #[cfg(feature = "torrent")]
    pub async fn action(&self, id: usize, action: TorrentAction) -> AppResult<()> {
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

    #[cfg(feature = "torrent")]
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
    #[cfg(feature = "torrent")]
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
                state: format!("{:?}", tracker.state).to_lowercase(),
                seeders: tracker.seeders,
                leechers: tracker.leechers,
                peers_returned: tracker.peers_returned,
                last_error: tracker.last_error.clone(),
            })
            .collect())
    }

    #[cfg(feature = "torrent")]
    pub fn files(&self, id: usize) -> AppResult<Vec<TorrentFileItem>> {
        let api = self.api()?;
        let target = librtbit::api::TorrentIdOrHash::Id(id);
        let details = api
            .api_torrent_details(target)
            .map_err(|error| AppError::torrent(format!("{error:#}")))?;
        let progress = api
            .api_stats_v1(target)
            .map(|stats| stats.file_progress)
            .unwrap_or_default();
        Ok(map_files(&details, &progress))
    }

    #[cfg(feature = "torrent")]
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

        *self.settings.write().unwrap() = next.clone();
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

    #[cfg(feature = "torrent")]
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
    #[cfg(feature = "torrent")]
    fn refresh_peers(&self, list: &[TorrentListItem]) {
        let Ok(api) = self.api() else {
            return;
        };
        let now = std::time::Instant::now();
        let mut states = self.peer_refresh.write().unwrap();
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
                let target = librtbit::api::TorrentIdOrHash::Id(item.id);
                let _ = api.api_torrent_action_reannounce(target);
            }
        }
    }

    // ---------------------------------------------------------------------
    // Feature-disabled stubs. The public surface stays identical so command
    // registration and AppState do not need conditional compilation.
    // ---------------------------------------------------------------------

    #[cfg(not(feature = "torrent"))]
    fn unavailable_error(&self) -> AppError {
        AppError::torrent_unavailable(
            "This build does not include BitTorrent support. Rebuild with --features torrent.",
        )
    }

    #[cfg(not(feature = "torrent"))]
    pub fn list(&self) -> AppResult<Vec<TorrentListItem>> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn add_magnet(
        &self,
        _magnet: String,
        _options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn add_file(
        &self,
        _path: String,
        _options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn add_file_bytes(
        &self,
        _bytes: Vec<u8>,
        _options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn add_magnet_when_ready(
        &self,
        _magnet: String,
        _options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn add_file_bytes_when_ready(
        &self,
        _bytes: Vec<u8>,
        _options: TorrentAddOptions,
    ) -> AppResult<TorrentAddResult> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn action(&self, _id: usize, _action: TorrentAction) -> AppResult<()> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn set_files(&self, _id: usize, _included: Vec<usize>) -> AppResult<()> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub fn tracker_status(&self, _id: usize) -> AppResult<Vec<TorrentTrackerStatus>> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub fn files(&self, _id: usize) -> AppResult<Vec<TorrentFileItem>> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub fn update_settings(self: &Arc<Self>, next: TorrentSettings) -> AppResult<TorrentSettings> {
        next.validate().map_err(AppError::invalid_settings)?;
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn restart_engine(self: &Arc<Self>) -> AppResult<()> {
        Err(self.unavailable_error())
    }
}

/// Parses the peer addresses a caller wants the engine to try first.
///
/// An empty list means "no preference", so it is reported as `None` instead of
/// an empty peer set, which the engine would treat as "no way to find peers".
#[cfg(feature = "torrent")]
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

#[cfg(feature = "torrent")]
fn map_files(
    details: &librtbit::api::TorrentDetailsResponse,
    progress: &[u64],
) -> Vec<TorrentFileItem> {
    details
        .files
        .as_ref()
        .map(|files| {
            files
                .iter()
                .enumerate()
                .map(|(index, file)| TorrentFileItem {
                    index,
                    name: file.name.clone(),
                    length: file.length,
                    included: file.included,
                    priority: file.priority,
                    progress_bytes: progress.get(index).copied().unwrap_or(0),
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
#[cfg(feature = "torrent")]
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
#[cfg(feature = "torrent")]
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

#[cfg(feature = "torrent")]
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

#[cfg(all(test, feature = "torrent"))]
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

#[cfg(all(test, feature = "torrent"))]
mod engine_contract_tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use librtbit::{AddTorrent, AddTorrentOptions, Api, Session, SessionOptions};

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
}
