use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::{
    application::error::AppResult,
    domain::updater::{
        friendly_update_error, is_newer_version, is_no_published_release,
        InstallDecision, UpdateCheckMode, UpdateCheckOutcome, UpdateReleaseInfo,
    },
    infrastructure::updater::UpdateClient,
};

#[derive(Clone)]
pub struct UpdateService {
    client: Arc<dyn UpdateClient>,
    current_version: String,
    is_dev_mode: bool,
    checking: Arc<AtomicBool>,
    available: Arc<Mutex<Option<UpdateReleaseInfo>>>,
}

impl UpdateService {
    pub fn new(
        client: Arc<dyn UpdateClient>,
        current_version: impl Into<String>,
        is_dev_mode: bool,
    ) -> Self {
        Self {
            client,
            current_version: current_version.into(),
            is_dev_mode,
            checking: Arc::new(AtomicBool::new(false)),
            available: Arc::new(Mutex::new(None)),
        }
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub fn is_dev_mode(&self) -> bool {
        self.is_dev_mode
    }

    pub async fn check(&self, mode: UpdateCheckMode) -> AppResult<UpdateCheckOutcome> {
        if self.is_dev_mode {
            return Ok(match mode {
                UpdateCheckMode::Manual => UpdateCheckOutcome::DevModeUnavailable,
                UpdateCheckMode::Background => UpdateCheckOutcome::SilentNoop,
            });
        }

        if let Some(cached) = self.available.lock().unwrap().clone() {
            return Ok(match mode {
                UpdateCheckMode::Manual => UpdateCheckOutcome::PromptToDownload(cached),
                UpdateCheckMode::Background => UpdateCheckOutcome::NotifyAvailable(cached),
            });
        }

        if self
            .checking
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(UpdateCheckOutcome::SilentNoop);
        }

        let result = self.client.check_latest_release().await;
        self.checking.store(false, Ordering::SeqCst);

        match result {
            Ok(Some(info)) => {
                if is_newer_version(&self.current_version, &info.version) {
                    *self.available.lock().unwrap() = Some(info.clone());
                    Ok(match mode {
                        UpdateCheckMode::Manual => UpdateCheckOutcome::PromptToDownload(info),
                        UpdateCheckMode::Background => UpdateCheckOutcome::NotifyAvailable(info),
                    })
                } else {
                    Ok(match mode {
                        UpdateCheckMode::Manual => UpdateCheckOutcome::UpToDate {
                            version: self.current_version.clone(),
                        },
                        UpdateCheckMode::Background => UpdateCheckOutcome::SilentNoop,
                    })
                }
            }
            Ok(None) => Ok(match mode {
                UpdateCheckMode::Manual => UpdateCheckOutcome::UpToDate {
                    version: self.current_version.clone(),
                },
                UpdateCheckMode::Background => UpdateCheckOutcome::SilentNoop,
            }),
            Err(err) => {
                if is_no_published_release(&err.message) {
                    Ok(match mode {
                        UpdateCheckMode::Manual => UpdateCheckOutcome::UpToDate {
                            version: self.current_version.clone(),
                        },
                        UpdateCheckMode::Background => UpdateCheckOutcome::SilentNoop,
                    })
                } else {
                    Ok(match mode {
                        UpdateCheckMode::Manual => UpdateCheckOutcome::Error {
                            friendly_detail: friendly_update_error(&err.message).to_string(),
                            message: err.message,
                        },
                        UpdateCheckMode::Background => UpdateCheckOutcome::SilentNoop,
                    })
                }
            }
        }
    }

    pub fn evaluate_install_decision(&self, active_transfers: usize) -> InstallDecision {
        if active_transfers > 0 {
            InstallDecision::WarnActiveDownloads(active_transfers)
        } else {
            InstallDecision::ReadyToInstall
        }
    }

    pub async fn download_update_payload(
        &self,
        download_url: &str,
        destination: &std::path::Path,
    ) -> AppResult<()> {
        self.client
            .download_update_installer(download_url, destination)
            .await
    }
}
