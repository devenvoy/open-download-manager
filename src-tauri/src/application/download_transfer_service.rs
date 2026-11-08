// Created: 2026-11-05
// Reason: feat: add native HTTP download engine
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
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
        quota_service::{QuotaEnforcementService, QuotaStatus},
        settings_service::SettingsRepository,
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
    quota_service: Arc<QuotaEnforcementService>,
    settings_repository: Arc<std::sync::RwLock<Option<Arc<dyn SettingsRepository>>>>,
    queue_service: Arc<std::sync::RwLock<Option<Arc<crate::application::queue_service::QueueService>>>>,
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
            quota_service: Arc::new(QuotaEnforcementService::new(None)),
            settings_repository: Arc::new(std::sync::RwLock::new(None)),
            queue_service: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn with_queue_service(
        self,
        queue_service: Arc<crate::application::queue_service::QueueService>,
    ) -> Self {
        self.set_queue_service(queue_service);
        self
    }

    pub fn set_queue_service(
        &self,
        queue_service: Arc<crate::application::queue_service::QueueService>,
    ) {
        *self.queue_service.write().unwrap() = Some(queue_service);
    }

    pub fn queue_service(&self) -> Option<Arc<crate::application::queue_service::QueueService>> {
        self.queue_service.read().unwrap().clone()
    }

    pub fn with_action_executor(
        mut self,
        action_executor: impl ActionExecutor + 'static,
    ) -> Self {
        self.action_executor = Arc::new(action_executor);
        self
    }

    pub fn with_quota_service(mut self, quota_service: Arc<QuotaEnforcementService>) -> Self {
        self.quota_service = quota_service;
        self
    }

    pub fn with_settings_repository(self, repo: Arc<dyn SettingsRepository>) -> Self {
        *self.settings_repository.write().unwrap() = Some(repo);
        self
    }

    pub fn set_settings_repository(&self, repo: Arc<dyn SettingsRepository>) {
        if let Ok(mut lock) = self.settings_repository.write() {
            *lock = Some(repo);
        }
    }

    pub fn quota_service(&self) -> Arc<QuotaEnforcementService> {
        self.quota_service.clone()
    }

    pub fn set_settings(&self, settings: Settings) {
        if let Ok(mut lock) = self.settings.write() {
            *lock = Some(settings.clone());
        }
        self.executor.set_connection_overrides(&settings.connection_overrides, settings.connections);
        self.quota_service.update_limit(settings.window_limit.clone());
        if !self.quota_service.is_quota_exceeded().0 {
            self.resume_quota_paused_downloads();
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
                    queue_id: request
                        .queue_id
                        .clone()
                        .or_else(|| Some(crate::domain::queue::DEFAULT_QUEUE_ID.to_string())),
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
                        settings.connections_for_host(url.url().host_str().unwrap_or(""))
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

        if let Some(qs) = self.queue_service() {
            let target_queue = request
                .queue_id
                .as_deref()
                .unwrap_or(crate::domain::queue::DEFAULT_QUEUE_ID);
            for id in &ids {
                let _ = qs.assign_download(id.clone(), target_queue);
            }
        }

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

                let item_host = url::Url::parse(&item.url)
                    .ok()
                    .and_then(|u| u.host_str().map(|h| h.to_string()))
                    .unwrap_or_default();

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
                    queue_id: Some(crate::domain::queue::DEFAULT_QUEUE_ID.to_string()),
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
                    connections: settings.connections_for_host(&item_host),
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
            DownloadAction::Resume => {
                let (exceeded, explanation) = self.quota_service.is_quota_exceeded();
                if exceeded {
                    let msg = explanation.unwrap_or_else(|| "Download limit reached.".to_string());
                    return Err(AppError::download(format!("Cannot resume download: {msg}")));
                }
                self.start_download(id, false)
            }
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
        let removed = self.history.remove_completed_downloads()?;
        if let Some(qs) = self.queue_service() {
            for id in &removed {
                let _ = qs.remove_download(id);
            }
        }
        Ok(removed)
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
        if let Some(qs) = self.queue_service() {
            let _ = qs.remove_download(id);
        }
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
        if self.quota_service.is_quota_exceeded().0 {
            return;
        }
        let _guard = match self.queue_pump_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let max = self.concurrency.max_concurrent();
        let active = match self.history.active_download_count() {
            Ok(count) => count,
            Err(_) => return,
        };
        if active >= max {
            return;
        }
        let mut available = max - active;

        if let Some(qs) = self.queue_service() {
            let running = qs.running_queues();
            for queue in running {
                if available == 0 {
                    break;
                }
                let mut queue_active = 0;
                if let Ok(controls) = self.controls.lock() {
                    for item_id in &queue.item_ids {
                        if controls.contains_key(item_id) {
                            queue_active += 1;
                        }
                    }
                }

                let queue_limit = queue.concurrency_limit as usize;
                if queue_active < queue_limit {
                    let queue_available = (queue_limit - queue_active).min(available);
                    let mut started_for_queue = 0;
                    for item_id in &queue.item_ids {
                        if started_for_queue >= queue_available {
                            break;
                        }
                        if let Ok(record) = self.history.download(item_id) {
                            if record.status == DownloadStatus::Queued {
                                if let Err(error) = self.start_download(item_id, false) {
                                    eprintln!(
                                        "Failed to auto-start queued download {item_id}: {}",
                                        error.message
                                    );
                                } else {
                                    started_for_queue += 1;
                                    available = available.saturating_sub(1);
                                }
                            }
                        }
                    }
                }
            }
        } else if let Ok(next_ids) = self.history.next_queued_download_ids(available) {
            for id in next_ids {
                if let Err(error) = self.start_download(&id, false) {
                    eprintln!("Failed to auto-start queued download {id}: {}", error.message);
                }
            }
        }
    }

    pub fn pause_queue_downloads(&self, queue_id: &str) {
        if let Some(qs) = self.queue_service() {
            if let Some(q) = qs.get_queue(queue_id) {
                for item_id in q.item_ids {
                    let _ = self.action(&item_id, DownloadAction::Pause);
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
                if service.quota_service.check_window_roll() {
                    service.resume_quota_paused_downloads();
                    service.persist_quota_state();
                }
                if let Err(error) = service.start_due_downloads() {
                    eprintln!("Could not start scheduled downloads: {}", error.message);
                }
                if let Some(qs) = service.queue_service() {
                    let (_started, stopped) = qs.check_schedules();
                    for stop_id in stopped {
                        service.pause_queue_downloads(&stop_id);
                    }
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
        let (exceeded, explanation) = self.quota_service.is_quota_exceeded();
        if exceeded {
            let msg = explanation.unwrap_or_else(|| "Download limit reached.".to_string());
            return Err(AppError::download(format!("Cannot start download: {msg}")));
        }
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
        self.quota_service.remove_download_tracker(id);
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
                    "link_expired" => {
                        let _ = service.history.update_active_download(&id, |d| {
                            d.status = DownloadStatus::Failed;
                            d.speed = 0;
                            d.error = Some(crate::domain::download::sanitize_diagnostic_text(&error.message));
                            d.notice = Some(
                                "The download link has expired. Refresh the link from the browser to resume."
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
            service.quota_service.remove_download_tracker(&id);
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
        let quota_service = self.quota_service.clone();
        let self_for_quota = self.clone();
        let progress_handler: DownloadProgressHandler = Arc::new(move |progress| {
            let quota_status = quota_service.record_progress(&download_id, progress.downloaded);
            if let QuotaStatus::Exceeded { explanation, .. } = quota_status {
                self_for_quota.pause_active_for_quota(&explanation);
                self_for_quota.persist_quota_state();
            }
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
        self.quota_service.remove_download_tracker(&id);
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
        let sanitized = crate::domain::download::sanitize_diagnostic_text(&message);
        if let Err(error) = self.history.update_active_download(id, |download| {
            download.status = DownloadStatus::Failed;
            download.speed = 0;
            download.error = Some(sanitized);
        }) {
            eprintln!("Could not save failed download {id}: {}", error.message);
        }
    }

    pub fn pause_active_for_quota(&self, explanation: &str) {
        let active_ids = match self.history.with_downloads(|downloads| {
            downloads
                .iter()
                .filter(|d| {
                    matches!(
                        d.status,
                        DownloadStatus::Downloading
                            | DownloadStatus::Connecting
                            | DownloadStatus::Assembling
                    )
                })
                .map(|d| d.id.clone())
                .collect::<Vec<_>>()
        }) {
            Ok(ids) => ids,
            Err(_) => return,
        };

        for id in active_ids {
            self.quota_service.mark_quota_paused(&id);
            let _ = self.stop_download(&id, DownloadStopReason::Pause);
            let _ = self.history.update_active_download(&id, |d| {
                d.notice = Some(explanation.to_string());
            });
        }
    }

    pub fn resume_quota_paused_downloads(&self) {
        let paused_ids = self.quota_service.drain_quota_paused();
        for id in paused_ids {
            let _ = self.history.update_active_download(&id, |d| {
                d.notice = None;
                d.status = DownloadStatus::Queued;
            });
        }
        self.pump_queue();
    }

    pub fn persist_quota_state(&self) {
        if let (Some(mut settings), Some(repo)) = (
            self.settings.read().ok().and_then(|g| g.clone()),
            self.settings_repository.read().ok().and_then(|g| g.clone()),
        ) {
            settings.window_limit = self.quota_service.current_limit();
            let _ = repo.save(&settings);
        }
    }
}
