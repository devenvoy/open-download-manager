use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::fs::{self, OpenOptions};
use url::Url;

use crate::{
    application::{
        download_executor::{
            DownloadControl, DownloadExecution, DownloadExecutor, DownloadProgressHandler,
        },
        download_rate_limiter::DownloadRateLimiter,
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::{
        download::{
            percent_decode, sanitize_filename, DestinationAllocator, DownloadRecord,
            FileConflictPolicy,
        },
        settings::ProxySettings,
    },
};

pub mod connection;
pub mod data_channel;
pub mod error;
pub mod stream;

use connection::{FtpConnection, DEFAULT_FTP_PORT, DEFAULT_FTP_TIMEOUT};
use data_channel::FtpDataChannel;
use error::FtpError;
use stream::stream_ftp_payload;
use crate::infrastructure::credentials::CredentialStore;

pub const TEMPORARY_DOWNLOAD_PREFIX: &str = ".open-download-manager-";

#[derive(Clone)]
pub struct FtpDownloadExecutor {
    allocator: DestinationAllocator,
    passive_default: bool,
    credential_store: Option<Arc<CredentialStore>>,
}

impl FtpDownloadExecutor {
    pub fn new(allocator: DestinationAllocator) -> Self {
        Self {
            allocator,
            passive_default: true,
            credential_store: None,
        }
    }

    pub fn with_mode(allocator: DestinationAllocator, passive_default: bool) -> Self {
        Self {
            allocator,
            passive_default,
            credential_store: None,
        }
    }

    pub fn with_credentials(mut self, credential_store: Arc<CredentialStore>) -> Self {
        self.credential_store = Some(credential_store);
        self
    }

    pub fn allocator(&self) -> DestinationAllocator {
        self.allocator.clone()
    }

    async fn transfer(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        mut control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> AppResult<DownloadResult> {
        let url = Url::parse(&download.url)
            .map_err(|_| AppError::download("The stored FTP download address is invalid."))?;

        if url.scheme() != "ftp" {
            return Err(AppError::download("The download protocol is not FTP."));
        }

        let host = url
            .host_str()
            .ok_or_else(|| AppError::download("FTP URL is missing host address."))?;
        let port = url.port().unwrap_or(DEFAULT_FTP_PORT);

        let remote_path = percent_decode(url.path());
        if remote_path.trim_matches('/').is_empty() {
            return Err(AppError::download("FTP URL must point to a specific file."));
        }

        let directory = PathBuf::from(
            download
                .directory
                .as_deref()
                .ok_or_else(|| AppError::download("The download folder is missing."))?,
        );

        let filename = sanitize_filename(&download.filename);
        if filename.is_empty() {
            return Err(AppError::download("The download file name is invalid."));
        }

        let mut destination = directory.join(&filename);
        fs::create_dir_all(&directory).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;

        if destination.is_dir() {
            return Err(AppError::download(
                "The destination path is an existing directory.",
            ));
        }

        let destination_exists = fs::try_exists(&destination).await.map_err(|error| {
            AppError::download(format!("Could not check the destination file: {error}"))
        })?;
        let is_claimed = self.allocator.is_claimed(&destination);

        if destination_exists || is_claimed {
            match download.conflict_policy {
                Some(FileConflictPolicy::Overwrite) => {}
                Some(FileConflictPolicy::Rename) => {
                    destination = self
                        .allocator
                        .allocate_conflict_free_destination(&directory, &filename);
                }
                Some(FileConflictPolicy::Skip) => {
                    return Err(AppError {
                        code: "download_skipped",
                        message: "Download skipped because destination file already exists."
                            .to_owned(),
                    });
                }
                Some(FileConflictPolicy::Cancel) => {
                    return Err(AppError {
                        code: "download_cancelled",
                        message: "Download cancelled due to destination conflict.".to_owned(),
                    });
                }
                None | Some(FileConflictPolicy::Ask) => {
                    return Err(AppError::conflict("A file with that name already exists."));
                }
            }
        }

        let _claim_guard = self.allocator.claim_guard(destination.clone());

        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        let mut existing_len = match fs::metadata(&temporary).await {
            Ok(metadata) => metadata.len(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
            Err(err) => {
                return Err(AppError::download(format!(
                    "Could not inspect partial FTP download: {err}"
                )));
            }
        };

        // Extract credentials:
        // Priority: 1. download.credentials_override, 2. inline URL credentials, 3. CredentialStore matcher
        let (username, password) = if let Some((ref u, ref p)) = download.credentials_override {
            (Some(u.clone()), Some(p.clone()))
        } else if !url.username().is_empty() {
            (
                Some(percent_decode(url.username())),
                url.password().map(|p| percent_decode(p)),
            )
        } else if let Some(rule) = self.credential_store.as_ref().and_then(|s| s.matcher().ok()).and_then(|m| m.match_url(&url).cloned()) {
            (Some(rule.username.clone()), Some(rule.secret.clone()))
        } else {
            (None, None)
        };

        // 1. Connect control channel
        let mut conn = FtpConnection::connect(host, port, DEFAULT_FTP_TIMEOUT)
            .await
            .map_err(AppError::from)?;

        // 2. Login
        conn.login(username.as_deref(), password.as_deref())
            .await
            .map_err(AppError::from)?;

        // 3. Negotiate capabilities (FEAT & UTF8)
        let features = conn.send_feat().await;
        if features.utf8 {
            let _ = conn.send_opts_utf8().await;
        }

        // 4. Set binary transfer mode
        conn.send_type_binary().await.map_err(AppError::from)?;

        // 5. Probe size & modification time (SIZE/MDTM with MLST fallback)
        let mut remote_size = conn.send_size(&remote_path).await.map_err(AppError::from)?;
        let mut remote_mdtm = conn.send_mdtm(&remote_path).await.map_err(AppError::from)?;

        if remote_size.is_none() || remote_mdtm.is_none() {
            if let Ok((mlst_size, mlst_mdtm)) = conn.send_mlst(&remote_path).await {
                if remote_size.is_none() {
                    remote_size = mlst_size;
                }
                if remote_mdtm.is_none() {
                    remote_mdtm = mlst_mdtm;
                }
            }
        }

        let current_validator = remote_mdtm.map(crate::domain::download::TransferValidator::LastModified);

        // 6. Audit for restart if partial file exists
        if existing_len > 0 {
            let mut restart_needed = false;
            // Check if remote size changed
            if let (Some(expected_total), Some(actual_size)) = (download.total, remote_size) {
                if expected_total != actual_size {
                    restart_needed = true;
                }
            }
            // Check if validator changed
            if let (Some(ref prev_val), Some(ref curr_val)) = (&download.validator, &current_validator) {
                if prev_val != curr_val {
                    restart_needed = true;
                }
            }

            if restart_needed {
                let _ = fs::remove_file(&temporary).await;
                existing_len = 0;
            }
        }

        // 7. Resume negotiation (REST)
        if existing_len > 0 {
            let rest_accepted = conn
                .send_rest(existing_len)
                .await
                .map_err(AppError::from)?;

            if !rest_accepted {
                // Server rejected REST offset -> clean restart fallback
                let _ = fs::remove_file(&temporary).await;
                existing_len = 0;
            }
        }

        // 8. Open data channel (Passive mode with EPSV/PASV, active mode fallback)
        let data_stream = if self.passive_default {
            match FtpDataChannel::open_passive(&mut conn, features.epsv, DEFAULT_FTP_TIMEOUT).await {
                Ok(stream) => {
                    retrieve_file(&mut conn, &remote_path).await?;
                    stream
                }
                Err(err) => {
                    // Fallback to active mode (PORT)
                    let listener = FtpDataChannel::setup_active(&mut conn, None)
                        .await
                        .map_err(|_| AppError::from(err))?;
                    retrieve_file(&mut conn, &remote_path).await?;
                    FtpDataChannel::accept_active(listener, DEFAULT_FTP_TIMEOUT)
                        .await
                        .map_err(AppError::from)?
                }
            }
        } else {
            let listener = FtpDataChannel::setup_active(&mut conn, None)
                .await
                .map_err(AppError::from)?;
            retrieve_file(&mut conn, &remote_path).await?;
            FtpDataChannel::accept_active(listener, DEFAULT_FTP_TIMEOUT)
                .await
                .map_err(AppError::from)?
        };

        // 9. Open file for writing
        let mut file = if existing_len > 0 {
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(&temporary)
                .await
                .map_err(|e| AppError::download(format!("Could not open partial file: {e}")))?
        } else {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temporary)
                .await
                .map_err(|e| AppError::download(format!("Could not create download file: {e}")))?
        };

        // 10. Stream payload
        let stream_result = stream_ftp_payload(
            data_stream,
            &mut file,
            existing_len,
            remote_size,
            current_validator,
            &progress_handler,
            &mut control,
            rate_limiter,
        )
        .await;

        let final_downloaded = match stream_result {
            Ok(bytes) => bytes,
            Err(err) => {
                // If stopped/cancelled, abort on server
                let _ = conn.send_abor().await;
                let _ = conn.send_quit().await;
                return Err(err);
            }
        };

        // 11. Check completion on control channel
        conn.check_transfer_end_reply()
            .await
            .map_err(AppError::from)?;
        let _ = conn.send_quit().await;

        drop(file);

        // 12. Finalize file into destination
        finalize_download(&temporary, &destination).await?;

        Ok(DownloadResult {
            downloaded: final_downloaded,
            total: remote_size.or(Some(final_downloaded)),
            path: destination,
        })
    }
}

async fn retrieve_file(conn: &mut FtpConnection, remote_path: &str) -> Result<(), AppError> {
    match conn.send_retr(remote_path).await {
        Ok(()) => Ok(()),
        Err(FtpError::FileNotFound(msg)) => {
            // Path navigation fallback: If absolute path fails, try CWD to parent directory and RETR basename
            if let Some(slash_idx) = remote_path.rfind('/') {
                let dir_part = &remote_path[..slash_idx];
                let file_part = &remote_path[slash_idx + 1..];
                if !dir_part.is_empty() && !file_part.is_empty() {
                    if let Ok(true) = conn.send_cwd(dir_part).await {
                        if let Ok(()) = conn.send_retr(file_part).await {
                            return Ok(());
                        }
                    }
                }
            }
            Err(AppError::from(FtpError::FileNotFound(msg)))
        }
        Err(err) => Err(AppError::from(err)),
    }
}

async fn finalize_download(temporary: &Path, destination: &Path) -> AppResult<()> {
    if fs::metadata(destination)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        return Err(AppError::download(
            "The destination path is an existing directory.",
        ));
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).await.map_err(|error| {
            AppError::download(format!("Could not create the download folder: {error}"))
        })?;
    }

    match fs::rename(temporary, destination).await {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(18) => {
            fs::copy(temporary, destination).await.map_err(|error| {
                AppError::download(format!("Could not move the download into place: {error}"))
            })?;
            let _ = fs::remove_file(temporary).await;
            Ok(())
        }
        Err(error) => Err(AppError::download(format!(
            "Could not move the download into place: {error}"
        ))),
    }
}

impl DownloadExecutor for FtpDownloadExecutor {
    fn execute(
        &self,
        download: DownloadRecord,
        progress_handler: DownloadProgressHandler,
        control: DownloadControl,
        rate_limiter: Arc<DownloadRateLimiter>,
    ) -> DownloadExecution {
        let executor = self.clone();
        Box::pin(async move {
            executor
                .transfer(download, progress_handler, control, rate_limiter)
                .await
        })
    }

    fn discard_partial(&self, download: &DownloadRecord) -> AppResult<()> {
        let directory = match download.directory.as_deref() {
            Some(d) => PathBuf::from(d),
            None => return Ok(()),
        };
        let temporary = directory.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{}.part", download.id));
        match std::fs::remove_file(&temporary) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AppError::download(format!(
                "Could not remove partial FTP file: {err}"
            ))),
        }
    }

    fn set_proxy(&self, _proxy: Option<&ProxySettings>) -> AppResult<()> {
        Ok(())
    }
}
