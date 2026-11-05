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
            filename_from_url, DownloadAction, DownloadRecord, DownloadRequest, DownloadStatus,
            ReviewDownloadRequest,
        },
        settings::Settings,
    },
    infrastructure::system_shell::SystemShellService,
};

#[derive(Clone)]
pub struct DownloadTransferService {
    history: DownloadHistoryService,
    executor: Arc<dyn DownloadExecutor>,
    concurrency: Arc<DownloadConcurrencyLimiter>,
    rate_limiter: Arc<DownloadRateLimiter>,
    controls: Arc<Mutex<HashMap<String, watch::Sender<Option<DownloadStopReason>>>>>,
    shell: Arc<dyn ShellService>,
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
        Self {
            history,
            executor: Arc::new(executor),
            concurrency: DownloadConcurrencyLimiter::new(max_concurrent),
            rate_limiter: Arc::new(DownloadRateLimiter::new(0)),
            controls: Arc::new(Mutex::new(HashMap::new())),
            shell: Arc::new(shell),
        }
    }

    pub fn add(&self, request: DownloadRequest, settings: &Settings) -> AppResult<Vec<String>> {
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
        let directory = request.directory.to_str().ok_or_else(|| {
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
            ];
            let existing_records = self.history.downloads()?;
            if let Some(existing) = existing_records.iter().find(|d| {
                if !active_states.contains(&d.status) {
                    return false;
                }
                if let Some(ref req_sabr) = request.sabr {
                    d.sabr.as_ref().map_or(false, |s| {
                        s.video_id == req_sabr.video_id
                            && s.video_itag == req_sabr.video_itag
                            && s.audio_itag == req_sabr.audio_itag
                    })
                } else {
                    let first_req_url = request.urls.first().map(|u| u.url().as_str()).unwrap_or("");
                    d.sabr.is_none()
                        && d.url == first_req_url
                        && d.audio_url == request.audio_url
                }
            }) {
                return Ok(vec![existing.id.clone()]);
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
                    .unwrap_or_else(|| filename_from_url(url.url()));
                let category = request.category.or_else(|| {
                    Some(DownloadCategory::from_path(std::path::Path::new(&filename)))
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
                    directory: Some(directory.to_owned()),
                    path: None,
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
                        } else {
                            Some(HTTP_DOWNLOAD_KIND.to_owned())
                        }
                    }),
                    resumable: if request.sabr.is_some() || request.audio_url.is_some() {
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
                }
            })
            .collect::<Vec<_>>();
        let ids = records
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        self.history.add_records(records)?;

        if !request.queue_only && !request.review_only && !is_scheduled {
            for id in &ids {
                self.start_download(id, false)?;
            }
        }
        Ok(ids)
    }

    pub fn review_download(
        &self,
        request: ReviewDownloadRequest,
    ) -> AppResult<Vec<String>> {
        let id = request.id.clone();
        let record = self.history.review_download(&request)?;
        if record.status == DownloadStatus::Queued {
            self.start_download(&id, false)?;
        }
        Ok(vec![id])
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

    pub fn set_max_concurrent(&self, value: u8) {
        self.concurrency.set_max_concurrent(value);
    }

    pub fn set_speed_limit(&self, value_kbps: u32) {
        self.rate_limiter.set_limit_kbps(value_kbps);
    }

    pub fn start_scheduler(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(
                SCHEDULER_POLL_INTERVAL_MILLISECONDS,
            ));
            loop {
                interval.tick().await;
                if let Err(error) = service.start_due_downloads() {
                    eprintln!("Could not start scheduled downloads: {}", error.message);
                }
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
                        if let Err(cleanup_error) =
                            service.executor.discard_partial(&cleanup_record)
                        {
                            eprintln!(
                                "Could not remove the cancelled download partial for {id}: {}",
                                cleanup_error.message
                            );
                        }
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
            }) {
                eprintln!(
                    "Could not update download progress for {download_id}: {}",
                    error.message
                );
            }
        });
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
            download.path = Some(result.path.to_string_lossy().into_owned());
        })?;
        let _ = self.history.notify_complete(&id);
        Ok(())
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
