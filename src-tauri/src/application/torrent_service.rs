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
        TorrentListItem, TorrentSettings,
    },
    infrastructure::torrent::settings_store::TorrentSettingsStore,
};

#[cfg(feature = "torrent")]
const MAX_MAGNET_LENGTH: usize = 16 * 1024;
#[cfg(feature = "torrent")]
const MAX_TORRENT_FILE_BYTES: u64 = 32 * 1024 * 1024;
#[cfg(feature = "torrent")]
const POLL_INTERVAL: Duration = Duration::from_secs(1);

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
            #[cfg(feature = "torrent")]
            engine: RwLock::new(None),
            #[cfg(feature = "torrent")]
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
            paused: options.start_paused,
            only_files: options.only_files,
            output_folder,
            overwrite: false,
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
            TorrentAction::Resume => api.api_torrent_action_start(target).await,
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
        api.api_torrent_action_update_only_files(target, &included)
            .await
            .map(|_| ())
            .map_err(|error| AppError::torrent(format!("{error:#}")))
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
                if service.watching.load(Ordering::Relaxed) == 0 {
                    continue;
                }
                if let Ok(list) = service.list() {
                    let _ = app.emit("torrent://progress", &list);
                }
            }
        });
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
    pub async fn action(&self, _id: usize, _action: TorrentAction) -> AppResult<()> {
        Err(self.unavailable_error())
    }

    #[cfg(not(feature = "torrent"))]
    pub async fn set_files(&self, _id: usize, _included: Vec<usize>) -> AppResult<()> {
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

#[cfg(feature = "torrent")]
fn map_list_item(details: &librtbit::api::TorrentDetailsResponse) -> TorrentListItem {
    let stats = details.stats.as_ref();
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
