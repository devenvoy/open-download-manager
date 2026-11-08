// Created: 2026-11-05
// Reason: feat: add native HTTP download engine
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Application service orchestrating download requests, transfers, scheduler pumps, and completion actions.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use tokio::sync::watch;

use crate::{
    application::{
        download_concurrency_limiter::DownloadConcurrencyLimiter,
        download_executor::{
            stopped_error, DownloadControl, DownloadExecutor, DownloadProgressHandler,
            DownloadStopReason,
        },
        download_history_service::DownloadHistoryService,
        download_rate_limiter::DownloadRateLimiter,
        download_transfer_policy::{
            HTTP_DOWNLOAD_KIND, NEW_DOWNLOAD_IS_ESTIMATED, NEW_DOWNLOAD_IS_RESUMABLE,
            SCHEDULER_POLL_INTERVAL_MILLISECONDS,
        },
        error::{AppError, AppResult},
        shell_service::ShellService,
    },
    domain::{
        category::DownloadCategory,
        download::{
            filename_from_url, sanitize_filename, ConflictResolutionOutcome, DestinationAllocator,
            DownloadAction, DownloadRecord, DownloadRequest, DownloadStatus, FileConflictPolicy,
            ReviewDownloadRequest,
        },
        media::AddPlaylistDownloadsRequest,
        settings::Settings,
    },
    infrastructure::{
        action::{ActionExecutor, SystemActionExecutor},
        system_shell::SystemShellService,
    },
};

#[derive(Clone)]
pub struct DownloadTransferService {
    history: DownloadHistoryService,
    executor: Arc<dyn DownloadExecutor>,
    concurrency: Arc<DownloadConcurrencyLimiter>,
    rate_limiter: Arc<DownloadRateLimiter>,
    controls: Arc<Mutex<HashMap<String, watch::Sender<Option<DownloadStopReason>>>>>,
    shell: Arc<dyn ShellService>,
    queue_pump_lock: Arc<Mutex<()>>,
    allocator: DestinationAllocator,
    action_executor: Arc<dyn ActionExecutor>,
    settings: Arc<std::sync::RwLock<Option<Settings>>>,
}

impl DownloadTransferService {
    pub fn new(
        history: DownloadHistoryService,
        executor: impl DownloadExecutor + 'static,
        max_concurrent: u8,
    ) -> Self {
        Self::with_shell(
            history,
            executor,
            max_concurrent,
            SystemShellService::new(),
        )
    }

    pub fn with_shell(
        history: DownloadHistoryService,
        executor: impl DownloadExecutor + 'static,
        max_concurrent: u8,
        shell: impl ShellService + 'static,
    ) -> Self {
        Self::with_shell_and_allocator(
            history,
            executor,
            max_concurrent,
            shell,
            DestinationAllocator::new(),
        )
    }

    pub fn with_shell_and_allocator(
        history: DownloadHistoryService,
        executor: impl DownloadExecutor + 'static,
        max_concurrent: u8,
        shell: impl ShellService + 'static,
        allocator: DestinationAllocator,
    ) -> Self {
        Self::with_all(
            history,
            executor,
            max_concurrent,
            shell,
            allocator,
            SystemActionExecutor::new(),
        )
    }

    pub fn with_all(
        history: DownloadHistoryService,
        executor: impl DownloadExecutor + 'static,
        max_concurrent: u8,
        shell: impl ShellService + 'static,
        allocator: DestinationAllocator,
        action_executor: impl ActionExecutor + 'static,
    ) -> Self {
        Self {
            history,
            executor: Arc::new(executor),
            concurrency: DownloadConcurrencyLimiter::new(max_concurrent),
            rate_limiter: Arc::new(DownloadRateLimiter::new(0)),
            controls: Arc::new(Mutex::new(HashMap::new())),
            shell: Arc::new(shell),
            queue_pump_lock: Arc::new(Mutex::new(())),
            allocator,
            action_executor: Arc::new(action_executor),
            settings: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn with_action_executor(
        mut self,
        action_executor: impl ActionExecutor + 'static,
    ) -> Self {
        self.action_executor = Arc::new(action_executor);
        self
    }

    pub fn set_settings(&self, settings: Settings) {
        if let Ok(mut lock) = self.settings.write() {
            *lock = Some(settings);
        }
    }

    pub fn settings(&self) -> Option<Settings> {
        self.settings.read().ok().and_then(|lock| lock.clone())
    }

    pub fn allocator(&self) -> DestinationAllocator {
        self.allocator.clone()
    }

    pub fn check_destination_conflict(
        &self,
        directory: &str,
        filename: &str,
    ) -> AppResult<ConflictResolutionOutcome> {
        let dir = Path::new(directory);
        let sanitized = sanitize_filename(filename);
        if sanitized.is_empty() {
            return Err(AppError::invalid_download("Choose a valid file name."));
        }
        let target = dir.join(&sanitized);
        if target.is_dir() {
            return Err(AppError::conflict("The destination path is an existing directory."));
        }
        let exists = target.exists() || self.allocator.is_claimed(&target);
        if exists {
            let suggested = self.allocator.suggest_conflict_free_destination(dir, &sanitized);
            Ok(ConflictResolutionOutcome::Conflict {
                existing_path: target,
                suggested_path: suggested,
                candidates: vec![
                    FileConflictPolicy::Overwrite,
                    FileConflictPolicy::Rename,
                    FileConflictPolicy::Skip,
                    FileConflictPolicy::Cancel,
                ],
            })
        } else {
            Ok(ConflictResolutionOutcome::Resolved {
                path: target,
                policy: FileConflictPolicy::Overwrite,
            })
        }
    }

    pub fn add(&self, request: DownloadRequest, settings: &Settings) -> AppResult<Vec<String>> {
        self.set_settings(settings.clone());
        let request_has_explicit_directory = request.directory.is_some();
        let detected_category = request.category.or_else(|| {
            if let Some(ref filename) = request.filename {
                return Some(DownloadCategory::from_path(std::path::Path::new(filename)));
            }
            if let Some(first_url) = request.urls.first() {
                if let Ok(parsed) = url::Url::parse(first_url) {
                    if let Some(segment) = parsed.path_segments().and_then(|mut s| s.next_back()) {
                        if !segment.is_empty() {
                            return Some(DownloadCategory::from_path(std::path::Path::new(segment)));
                        }
                    }
                }
            }
            None
        });

        let default_directory = detected_category
            .and_then(|category| settings.category_directories.get(&category))
            .cloned()
            .unwrap_or_else(|| settings.default_directory.clone());
        let request = request
            .validate(default_directory)
            .map_err(AppError::invalid_download)?;
        let base_directory = request.directory.to_str().ok_or_else(|| {
            AppError::invalid_download(
                "The download folder cannot be represented as a supported path.",
            )
        })?;

        if request.reuse_active {
            let active_states = [
                DownloadStatus::Connecting,
                DownloadStatus::Downloading,
                DownloadStatus::Assembling,
                DownloadStatus::Queued,
                DownloadStatus::Paused,
                DownloadStatus::Scheduled,
                DownloadStatus::Failed,
            ];
            let first_req_url = request.urls.first().map(|u| u.url().to_string()).unwrap_or_default();
            let matched_download = self.history.with_downloads(|existing_records| {
                existing_records
                    .iter()
                    .find(|d| {
                        if !active_states.contains(&d.status) {
                            return false;
                        }
                        if let Some(ref req_sabr) = request.sabr {
                            d.sabr.as_ref().is_some_and(|s| {
                                s.video_id == req_sabr.video_id
                                    && s.video_itag == req_sabr.video_itag
                                    && s.audio_itag == req_sabr.audio_itag
                            })
                        } else {
                            (d.sabr.is_none() && d.url == first_req_url && d.audio_url == request.audio_url)
                                || (d.referer.is_some() && d.referer == request.referer && request.filename.as_ref().is_some_and(|f| d.filename == *f))
                        }
                    })
                    .map(|d| (d.id.clone(), d.status.clone()))
            })?;

            if let Some((existing_id, status)) = matched_download {
                if status == DownloadStatus::Failed || status == DownloadStatus::Paused {
                    self.history.update_download(&existing_id, |d| {
                        if !first_req_url.is_empty() {
                            d.url = first_req_url.clone();
                        }
                        if request.audio_url.is_some() {
                            d.audio_url = request.audio_url.clone();
                        }
                        if request.sabr.is_some() {
                            d.sabr = request.sabr.clone();
                        }
                        d.error = None;
                        d.notice = Some("Download link refreshed from browser; resuming transfer.".to_owned());
                    })?;
                    if status == DownloadStatus::Failed {
                        let _ = self.start_download(&existing_id, true);
                    }
                }
                return Ok(vec![existing_id]);
            }
        }

        let is_scheduled = request.scheduled_at.is_some();
        let records = request
            .urls
            .iter()
            .enumerate()
            .map(|(index, url)| {
                let filename = request
                    .filename
                    .clone()
                    .filter(|f| !crate::domain::download::is_generic_filename(f))
                    .unwrap_or_else(|| filename_from_url(url.url()));
                let category = request.category.or_else(|| {
                    Some(DownloadCategory::from_path(std::path::Path::new(&filename)))
                });
                let item_directory = if request_has_explicit_directory {
                    base_directory.to_owned()
                } else {
                    category
                        .and_then(|cat| settings.category_directories.get(&cat))
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| base_directory.to_owned())
                };
                let item_temp_directory = request
                    .temp_directory
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .or_else(|| {
                        settings
                            .temporary_directory
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned())
                    });
                DownloadRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    url: url.url().as_str().to_owned(),
                    filename,
                    status: if request.review_only {
                        DownloadStatus::Paused
                    } else if is_scheduled {
                        DownloadStatus::Scheduled
                    } else {
                        DownloadStatus::Queued
                    },
                    downloaded: 0,
                    total: None,
                    speed: 0,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    needs_review: request.review_only,
                    scheduled_at: request.scheduled_at.clone(),
                    description: request.description.clone(),
                    category,
                    directory: Some(item_directory),
                    path: None,
                    temp_directory: item_temp_directory,
                    last_try: None,
                    error: None,
                    notice: (request.urls.len() > 1)
                        .then(|| format!("Address {} of {}.", index + 1, request.urls.len())),
                    estimated: NEW_DOWNLOAD_IS_ESTIMATED,
                    kind: request.kind.clone().or_else(|| {
                        if request.sabr.is_some() {
                            Some("sabr".to_owned())
                        } else if request.audio_url.is_some() {
                            Some("paired".to_owned())
                        } else if crate::domain::media::youtube_url(url.url().as_str()).is_some() {
                            Some("youtube".to_owned())
                        } else {
                            Some(HTTP_DOWNLOAD_KIND.to_owned())
                        }
                    }),
                    resumable: if request.sabr.is_some() {
                        Some(false)
                    } else {
                        Some(NEW_DOWNLOAD_IS_RESUMABLE)
                    },
                    duration: None,
                    connections: if request.audio_url.is_some() {
                        2
                    } else {
                        settings.connections
                    },
                    connections_progress: Vec::new(),
                    audio_url: request.audio_url.clone(),
                    sabr: request.sabr.clone(),
                    referer: request.referer.clone(),
                    youtube_format: request.youtube_format.clone(),
                    transfer_kind: None,
                    etag: None,
                    validator: None,
                    conflict_policy: request.conflict_policy,
                    credentials_override: request.credentials.as_ref().map(|c| {
                        (c.username.clone(), c.password.clone().unwrap_or_default())
                    }),
                    completion_actions: request.completion_actions.clone(),
                    action_ledger: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        let ids = records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        self.history.add_records(records)?;

        if request.review_only {
            for (index, url) in request.urls.iter().enumerate() {
                if let Some(id) = ids.get(index) {
                    self.probe_unreviewed_download(id.clone(), url.url().as_str().to_owned(), request.referer.clone());
                }
            }
        }

        if !request.queue_only && !request.review_only && !is_scheduled {
            self.pump_queue();
        }
        Ok(ids)
    }

    pub fn add_playlist_downloads(
        &self,
        request: AddPlaylistDownloadsRequest,
        settings: &Settings,
    ) -> AppResult<usize> {
        let category = request.common_category.unwrap_or(DownloadCategory::Video);
        let target_dir = request
            .common_directory
            .filter(|d| !d.trim().is_empty())
            .unwrap_or_else(|| {
                settings
                    .category_directories
                    .get(&category)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| settings.default_directory.to_string_lossy().to_string())
            });
        let temp_dir = settings
            .temporary_directory
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let selected_items: Vec<_> = request.items.into_iter().filter(|i| i.selected).collect();
        if selected_items.is_empty() {
            return Ok(0);
        }

        let total_count = selected_items.len();
        let records: Vec<DownloadRecord> = selected_items
            .into_iter()
            .enumerate()
            .map(|(idx, item)| {
                let clean_title = sanitize_filename::sanitize(&item.title);
                let clean_title = if clean_title.trim().is_empty() {
                    format!("video_{}", item.id)
                } else {
                    clean_title
                };
                let filename = if clean_title.ends_with(".mp4") || clean_title.ends_with(".mkv") {
                    clean_title
                } else {
                    format!("{clean_title}.mp4")
                };

                DownloadRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    url: item.url,
                    filename,
                    status: DownloadStatus::Paused,
                    downloaded: 0,
                    total: None,
                    speed: 0,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    needs_review: false,
                    scheduled_at: None,
                    description: Some(format!("From playlist: {}", request.playlist_title)),
                    category: Some(category),
                    directory: Some(target_dir.clone()),
                    path: None,
                    temp_directory: temp_dir.clone(),
                    last_try: None,
                    error: None,
                    notice: Some(format!("Playlist item {} of {}.", idx + 1, total_count)),
                    estimated: NEW_DOWNLOAD_IS_ESTIMATED,
                    kind: Some("youtube".to_owned()),
                    resumable: Some(NEW_DOWNLOAD_IS_RESUMABLE),
                    duration: None,
                    connections: settings.connections,
                    connections_progress: Vec::new(),
                    audio_url: None,
                    sabr: None,
                    referer: None,
                    youtube_format: None,
                    transfer_kind: None,
                    etag: None,
                    validator: None,
                    conflict_policy: None,
                    credentials_override: None,
                    completion_actions: None,
                    action_ledger: Vec::new(),
                }
            })
            .collect();

        let count = records.len();
        self.history.add_records(records)?;
        Ok(count)
    }

    pub fn review_download(
        &self,
        request: ReviewDownloadRequest,
    ) -> AppResult<Vec<String>> {
        let id = request.id.clone();
        let record = self.history.review_download(&request)?;
        if record.status == DownloadStatus::Queued {
            self.pump_queue();
        }
        Ok(vec![id])
    }

    pub fn refresh_download(
        &self,
        id: &str,
        new_url: Option<String>,
        new_audio_url: Option<String>,
        new_sabr: Option<crate::domain::media::sabr::NormalizedSabrSession>,
    ) -> AppResult<String> {
        let record = self.history.download(id)?;
        self.history.update_download(id, |d| {
            if let Some(u) = new_url {
                d.url = u;
            }
            if let Some(a) = new_audio_url {
                d.audio_url = Some(a);
            }
            if let Some(s) = new_sabr {
                d.sabr = Some(s);
            }
            d.error = None;
            d.notice = Some("Download link refreshed from browser; resuming transfer.".to_owned());
        })?;
        if record.status == DownloadStatus::Failed || record.status == DownloadStatus::Paused {
            let retry = record.status == DownloadStatus::Failed;
            self.start_download(id, retry)?;
        }
        Ok(id.to_owned())
    }

    pub fn action(&self, id: &str, action: DownloadAction) -> AppResult<()> {
        match action {
            DownloadAction::Resume => self.start_download(id, false),
            DownloadAction::Retry => self.start_download(id, true),
            DownloadAction::Pause => self.stop_download(id, DownloadStopReason::Pause),
            DownloadAction::Cancel => self.stop_download(id, DownloadStopReason::Cancel),
            DownloadAction::Remove => self.remove_download(id),
            DownloadAction::Open => self.open_download(id),
            DownloadAction::Reveal => self.reveal_download(id),
            DownloadAction::OpenWith => self.open_download_with(id),
        }
    }

    pub fn remove_completed(&self) -> AppResult<Vec<String>> {
        self.history.remove_completed_downloads()
    }

    fn remove_download(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        let controls = self
            .controls
            .lock()
            .map_err(|_| AppError::storage("Download controls are unavailable."))?;
        if let Some(control) = controls.get(id) {
            control.send_replace(Some(DownloadStopReason::Cancel));
            let control = control.clone();
            let executor = self.executor.clone();
            let cleanup_record = record.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    control.closed().await;
                    if let Err(error) = executor.discard_partial(&cleanup_record) {
                        eprintln!(
                            "Could not remove the cancelled download partial: {}",
                            error.message
                        );
                    }
                });
            } else {
                let _ = executor.discard_partial(&cleanup_record);
            }
        } else {
            self.executor.discard_partial(&record)?;
        }
        drop(controls);

        self.history.remove_download(id)?;
        self.pump_queue();
        Ok(())
    }

    fn open_download(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        if record.status != DownloadStatus::Completed || record.path.is_none() {
            return Err(AppError::download("This download has not finished."));
        }
        let path = Path::new(record.path.as_deref().unwrap());
        if !path.exists() {
            return Err(AppError::download("The downloaded file no longer exists."));
        }
        self.shell.open_path(path)
    }

    fn open_download_with(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        if record.status != DownloadStatus::Completed || record.path.is_none() {
            return Err(AppError::download("This download has not finished."));
        }
        let path = Path::new(record.path.as_deref().unwrap());
        if !path.exists() {
            return Err(AppError::download("The downloaded file no longer exists."));
        }
        self.shell.open_with(path)
    }

    fn reveal_download(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        if let Some(path_str) = &record.path {
            let path = Path::new(path_str);
            if path.exists() {
                return self.shell.reveal_path(path);
            }
        }
        if let Some(directory_str) = &record.directory {
            let directory = Path::new(directory_str);
            if directory.exists() {
                return self.shell.open_path(directory);
            }
        }
        Err(AppError::download("The file or folder cannot be found."))
    }

    pub fn pump_queue(&self) {
        let _guard = match self.queue_pump_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let max = self.concurrency.max_concurrent();
        let active = match self.history.active_download_count() {
            Ok(count) => count,
            Err(_) => return,
        };
        if active < max {
            let available = max - active;
            if let Ok(next_ids) = self.history.next_queued_download_ids(available) {
                for id in next_ids {
                    if let Err(error) = self.start_download(&id, false) {
                        eprintln!("Failed to auto-start queued download {id}: {}", error.message);
                    }
                }
            }
        }
    }

    pub fn set_max_concurrent(&self, value: u8) {
        self.concurrency.set_max_concurrent(value);
        self.pump_queue();
    }

    pub fn set_speed_limit(&self, value_kbps: u32) {
        self.rate_limiter.set_limit_kbps(value_kbps);
    }

    pub fn set_proxy(&self, proxy: Option<&crate::domain::settings::ProxySettings>) -> AppResult<()> {
        self.executor.set_proxy(proxy)
    }

    pub fn start_scheduler(&self) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(
                SCHEDULER_POLL_INTERVAL_MILLISECONDS,
            ));
            loop {
                interval.tick().await;
                if let Err(error) = service.start_due_downloads() {
                    eprintln!("Could not start scheduled downloads: {}", error.message);
                }
                service.pump_queue();
            }
        });
    }

    fn start_due_downloads(&self) -> AppResult<()> {
        let mut controls = self
            .controls
            .lock()
            .map_err(|_| AppError::storage("Download controls are unavailable."))?;
        let records = self
            .history
            .begin_due_scheduled_downloads(chrono::Utc::now(), chrono::Utc::now().to_rfc3339())?;
        let started = records
            .into_iter()
            .map(|record| {
                let (sender, receiver) = watch::channel(None);
                controls.insert(record.id.clone(), sender);
                (record, DownloadControl::new(receiver))
            })
            .collect::<Vec<_>>();
        drop(controls);
        for (record, control) in started {
            self.spawn_download(record, control);
        }
        Ok(())
    }

    fn start_download(&self, id: &str, retry: bool) -> AppResult<()> {
        let mut controls = self
            .controls
            .lock()
            .map_err(|_| AppError::storage("Download controls are unavailable."))?;
        if controls.contains_key(id) {
            return Err(AppError::download(
                "The download is still stopping. Try again shortly.",
            ));
        }
        let record = self
            .history
            .begin_download(id, retry, chrono::Utc::now().to_rfc3339())?;
        let (sender, receiver) = watch::channel(None);
        controls.insert(id.to_owned(), sender);
        drop(controls);
        self.spawn_download(record, DownloadControl::new(receiver));
        Ok(())
    }

    fn stop_download(&self, id: &str, reason: DownloadStopReason) -> AppResult<()> {
        let controls = self
            .controls
            .lock()
            .map_err(|_| AppError::storage("Download controls are unavailable."))?;
        let record = self.history.download(id)?;
        self.history.stop_download(id, reason)?;
        if let Some(control) = controls.get(id) {
            control.send_replace(Some(reason));
            if reason == DownloadStopReason::Cancel {
                let control = control.clone();
                let executor = self.executor.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        control.closed().await;
                        if let Err(error) = executor.discard_partial(&record) {
                            eprintln!(
                                "Could not remove the cancelled download partial: {}",
                                error.message
                            );
                        }
                    });
                } else {
                    let _ = executor.discard_partial(&record);
                }
            }
        } else if reason == DownloadStopReason::Cancel {
            self.executor.discard_partial(&record)?;
        }
        self.pump_queue();
        Ok(())
    }

    fn spawn_download(&self, record: DownloadRecord, control: DownloadControl) {
        let id = record.id.clone();
        let cleanup_record = record.clone();
        let service = self.clone();
        let download_task = async move {
            if let Err(error) = service.run_download(record, control).await {
                match error.code {
                    "download_paused" => {}
                    "download_cancelled" => {
                        let _ = service.history.update_active_download(&id, |d| {
                            d.status = DownloadStatus::Cancelled;
                            d.downloaded = 0;
                            d.speed = 0;
                            d.error = None;
                        });
                        if let Err(cleanup_error) =
                            service.executor.discard_partial(&cleanup_record)
                        {
                            eprintln!(
                                "Could not remove the cancelled download partial for {id}: {}",
                                cleanup_error.message
                            );
                        }
                    }
                    "download_skipped" => {
                        let _ = service.history.update_active_download(&id, |d| {
                            d.status = DownloadStatus::Skipped;
                            d.speed = 0;
                            d.notice = Some(
                                "Download skipped because destination file already exists."
                                    .to_owned(),
                            );
                        });
                    }
                    "destination_conflict" => {
                        let _ = service.history.update_active_download(&id, |d| {
                            d.status = DownloadStatus::Paused;
                            d.needs_review = true;
                            d.speed = 0;
                            d.error = Some(error.message.clone());
                            d.notice = Some(
                                "A file with that name already exists. Please choose an action: overwrite, keep both, skip, or cancel."
                                    .to_owned(),
                            );
                        });
                    }
                    _ => {
                        eprintln!("Download {id} failed: {}", error.message);
                        service.fail_download(&id, error.message);
                    }
                }
            }
            match service.controls.lock() {
                Ok(mut controls) => {
                    controls.remove(&id);
                }
                Err(_) => eprintln!("Could not release download controls for {id}."),
            }
            service.pump_queue();
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(download_task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(download_task);
                }
            });
        }
    }

    async fn run_download(
        &self,
        record: DownloadRecord,
        mut control: DownloadControl,
    ) -> AppResult<()> {
        let id = record.id.clone();
        let permit = tokio::select! {
            reason = control.stopped() => return Err(stopped_error(reason)),
            permit = self.concurrency.acquire() => permit?,
        };

        let history = self.history.clone();
        let download_id = id.clone();
        let progress_handler: DownloadProgressHandler = Arc::new(move |progress| {
            if let Err(error) = history.update_active_download(&download_id, |download| {
                download.status = DownloadStatus::Downloading;
                download.downloaded = progress.downloaded;
                download.total = progress.total;
                download.speed = progress.bytes_per_second;
                download.connections_progress = progress.connections_progress;
                if progress.notice.is_some() {
                    download.notice = progress.notice;
                }
                if let Some(ref tk) = progress.transfer_kind {
                    download.transfer_kind = Some(tk.clone());
                }
                if let Some(ec) = progress.effective_concurrency {
                    download.connections = ec;
                }
                if let Some(ref validator) = progress.validator {
                    download.validator = Some(validator.clone());
                    if let crate::domain::download::TransferValidator::ETag(ref etag) = validator {
                        download.etag = Some(etag.clone());
                    }
                } else if let Some(ref etag) = progress.etag {
                    download.etag = Some(etag.clone());
                }
            }) {
                eprintln!(
                    "Could not update download progress for {download_id}: {}",
                    error.message
                );
            }
        });
        let initial_total_unknown = record.total.is_none();
        let result = self
            .executor
            .execute(record, progress_handler, control, self.rate_limiter.clone())
            .await?;
        let _permit = permit;
        self.history.update_active_download(&id, |download| {
            download.status = DownloadStatus::Completed;
            download.downloaded = result.downloaded;
            download.total = result.total;
            download.speed = 0;
            if let Some(new_name) = result.path.file_name().and_then(|n| n.to_str()) {
                if download.filename != new_name {
                    download.filename = new_name.to_owned();
                }
            }
            download.path = Some(result.path.to_string_lossy().into_owned());
            if initial_total_unknown && result.total.is_none() {
                download.notice = Some("The file size was not supplied by the server. The file may be incomplete.".to_owned());
            }
        })?;
        self.run_completion_actions(&id, &result.path).await;
        Ok(())
    }

    pub async fn retry_completion_actions(&self, id: &str) -> AppResult<()> {
        let record = self.history.download(id)?;
        if record.status != DownloadStatus::Completed {
            return Err(AppError::download(
                "Actions can only be run on completed downloads.",
            ));
        }
        let path_str = record
            .path
            .ok_or_else(|| AppError::download("Download file path is missing."))?;
        let path = std::path::PathBuf::from(path_str);
        if !path.is_file() {
            return Err(AppError::download(
                "Downloaded file does not exist at final destination.",
            ));
        }
        self.history.update_download(id, |d| {
            d.action_ledger.clear();
            d.notice = None;
        })?;
        self.run_completion_actions(id, &path).await;
        Ok(())
    }

    pub fn update_download_completion_actions(
        &self,
        id: &str,
        actions: Option<Vec<crate::domain::completion::CompletionAction>>,
    ) -> AppResult<()> {
        if let Some(ref actions) = actions {
            for action in actions {
                if let Err(msg) = action.validate() {
                    return Err(AppError::invalid_download(msg));
                }
            }
        }
        self.history.update_download(id, |d| {
            d.completion_actions = actions;
        })
    }

    pub async fn run_completion_actions(&self, id: &str, final_path: &Path) {
        if !final_path.is_file() {
            eprintln!(
                "Skipping completion actions for {id}: destination file does not exist or is not a file."
            );
            return;
        }

        let record = match self.history.download(id) {
            Ok(record) => record,
            Err(error) => {
                eprintln!(
                    "Could not retrieve download record for {id} to run completion actions: {}",
                    error.message
                );
                return;
            }
        };

        if record.status != DownloadStatus::Completed {
            return;
        }

        let current_settings = self.settings();
        let effective_actions = if let Some(ref actions) = record.completion_actions {
            actions.clone()
        } else if let Some(ref settings) = current_settings {
            if !settings.completion_actions.is_empty() {
                settings.completion_actions.clone()
            } else {
                let mut legacy = Vec::new();
                if settings.notify_on_complete {
                    legacy.push(crate::domain::completion::CompletionAction::Notification);
                }
                if settings.reveal_on_complete {
                    legacy.push(crate::domain::completion::CompletionAction::OpenFolder);
                }
                legacy
            }
        } else {
            Vec::new()
        };

        if effective_actions.is_empty() {
            return;
        }

        let timeout_duration = std::time::Duration::from_secs(60);

        for action in effective_actions {
            let already_run = self
                .history
                .with_downloads(|downloads| {
                    downloads.iter().find(|d| d.id == id).is_some_and(|d| {
                        d.action_ledger
                            .iter()
                            .any(|entry| entry.action_key == action.action_key())
                    })
                })
                .unwrap_or(false);

            if already_run {
                continue;
            }

            let entry = match &action {
                crate::domain::completion::CompletionAction::Notification => {
                    let _ = self.history.notify_complete(id);
                    crate::domain::completion::ActionLedgerEntry::success(&action, None)
                }
                crate::domain::completion::CompletionAction::PlaySound { sound_file } => {
                    match self.action_executor.play_sound(sound_file.as_deref()).await {
                        Ok(()) => {
                            crate::domain::completion::ActionLedgerEntry::success(&action, None)
                        }
                        Err(err) => {
                            crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                        }
                    }
                }
                crate::domain::completion::CompletionAction::OpenFile => {
                    match self.action_executor.open_file(final_path) {
                        Ok(()) => {
                            crate::domain::completion::ActionLedgerEntry::success(&action, None)
                        }
                        Err(err) => {
                            crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                        }
                    }
                }
                crate::domain::completion::CompletionAction::OpenFolder => {
                    match self.action_executor.open_folder(final_path) {
                        Ok(()) => {
                            crate::domain::completion::ActionLedgerEntry::success(&action, None)
                        }
                        Err(err) => {
                            crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                        }
                    }
                }
                crate::domain::completion::CompletionAction::AntivirusScan { executable, .. } => {
                    let safe_args = action.resolve_arguments(final_path);
                    match self
                        .action_executor
                        .execute_process(executable, &safe_args, timeout_duration)
                        .await
                    {
                        Ok(output) => {
                            let msg = if output.is_empty() {
                                None
                            } else {
                                Some(output)
                            };
                            crate::domain::completion::ActionLedgerEntry::success(&action, msg)
                        }
                        Err(err) => {
                            crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                        }
                    }
                }
                crate::domain::completion::CompletionAction::ExecuteCommand {
                    executable, ..
                } => {
                    let safe_args = action.resolve_arguments(final_path);
                    match self
                        .action_executor
                        .execute_process(executable, &safe_args, timeout_duration)
                        .await
                    {
                        Ok(output) => {
                            let msg = if output.is_empty() {
                                None
                            } else {
                                Some(output)
                            };
                            crate::domain::completion::ActionLedgerEntry::success(&action, msg)
                        }
                        Err(err) => {
                            crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                        }
                    }
                }
                crate::domain::completion::CompletionAction::PowerAction {
                    kind,
                    confirmed,
                } => {
                    if !*confirmed {
                        crate::domain::completion::ActionLedgerEntry::skipped(
                            &action,
                            "Power action requires explicit confirmation.",
                        )
                    } else {
                        match self.action_executor.power_action(*kind).await {
                            Ok(()) => crate::domain::completion::ActionLedgerEntry::success(
                                &action, None,
                            ),
                            Err(err) => {
                                crate::domain::completion::ActionLedgerEntry::failed(&action, err)
                            }
                        }
                    }
                }
            };

            let is_failure = entry.status == crate::domain::completion::ActionStatus::Failed;
            let failure_message = entry.message.clone();
            let action_type = entry.action_type.clone();

            let _ = self.history.update_download(id, |d| {
                d.action_ledger.push(entry);
                if is_failure {
                    if let Some(msg) = failure_message {
                        d.notice = Some(format!("Action '{action_type}' failed: {msg}"));
                    }
                }
            });
        }
    }

    fn probe_unreviewed_download(&self, id: String, url_str: String, referer: Option<String>) {
        let history = self.history.clone();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build();
        let Ok(client) = client else { return; };

        let task = async move {
            let Ok(url) = url::Url::parse(&url_str) else { return; };
            let mut req = client.get(url).header(reqwest::header::RANGE, "bytes=0-0");
            if let Some(ref r) = referer {
                req = req.header(reqwest::header::REFERER, r);
            }
            if let Ok(resp) = req.send().await {
                let status = resp.status();
                let headers = resp.headers().clone();
                let cd_name = headers
                    .get(reqwest::header::CONTENT_DISPOSITION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(crate::domain::download::parse_content_disposition_filename);

                let (total, resumable) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
                    let total = headers
                        .get(reqwest::header::CONTENT_RANGE)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|cr| cr.rsplit('/').next())
                        .and_then(|s| s.parse::<u64>().ok());
                    (total, true)
                } else if status == reqwest::StatusCode::OK {
                    let total = headers
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok());
                    let has_ranges = headers
                        .get(reqwest::header::ACCEPT_RANGES)
                        .is_some_and(|v| v.as_bytes() == b"bytes");
                    (total, has_ranges)
                } else {
                    (None, false)
                };

                let validator = headers
                    .get(reqwest::header::ETAG)
                    .and_then(|v| v.to_str().ok())
                    .filter(|v| !v.starts_with("W/") && v.starts_with('"') && v.ends_with('"'))
                    .map(|s| crate::domain::download::TransferValidator::ETag(s.to_owned()))
                    .or_else(|| {
                        headers
                            .get(reqwest::header::LAST_MODIFIED)
                            .and_then(|v| v.to_str().ok())
                            .filter(|v| !v.trim().is_empty())
                            .map(|s| {
                                crate::domain::download::TransferValidator::LastModified(
                                    s.to_owned(),
                                )
                            })
                    });

                let _ = history.update_download(&id, |d| {
                    if d.needs_review {
                        if let Some(tot) = total {
                            d.total = Some(tot);
                        }
                        d.resumable = Some(resumable);
                        if let Some(ref val) = validator {
                            d.validator = Some(val.clone());
                            if let crate::domain::download::TransferValidator::ETag(ref e) = val {
                                d.etag = Some(e.clone());
                            }
                        }
                        if let Some(better_name) = cd_name {
                            let current_stem = std::path::Path::new(&d.filename)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("");
                            if crate::domain::download::is_generic_filename(current_stem) {
                                d.filename = better_name;
                            }
                        }
                    }
                });
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(task);
        } else {
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(task);
                }
            });
        }
    }

    fn fail_download(&self, id: &str, message: String) {
        if let Err(error) = self.history.update_active_download(id, |download| {
            download.status = DownloadStatus::Failed;
            download.speed = 0;
            download.error = Some(message);
        }) {
            eprintln!("Could not save failed download {id}: {}", error.message);
        }
    }
}
