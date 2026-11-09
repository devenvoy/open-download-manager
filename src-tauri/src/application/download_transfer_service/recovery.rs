// Created: 2026-11-10
// Reason: refactor: modularize monolithic download_transfer_service into focused coordinators
// Description: Unreviewed download probing and refresh recovery for expired download URLs.

use crate::{
    application::error::{AppResult},
    domain::{
        download::{DownloadStatus, TransferValidator},
        media::sabr::NormalizedSabrSession,
    },
};

use super::DownloadTransferService;

impl DownloadTransferService {
    pub(crate) fn probe_unreviewed_download(&self, id: String, url_str: String, referer: Option<String>) {
        let history = self.history.clone();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(crate::infrastructure::http_download_executor::client::DEFAULT_USER_AGENT)
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
                    .map(|s| TransferValidator::ETag(s.to_owned()))
                    .or_else(|| {
                        headers
                            .get(reqwest::header::LAST_MODIFIED)
                            .and_then(|v| v.to_str().ok())
                            .filter(|v| !v.trim().is_empty())
                            .map(|s| {
                                TransferValidator::LastModified(
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
                            if let TransferValidator::ETag(ref e) = val {
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

    pub fn refresh_download(
        &self,
        id: &str,
        new_url: Option<String>,
        new_audio_url: Option<String>,
        new_sabr: Option<NormalizedSabrSession>,
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
}
