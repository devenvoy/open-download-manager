use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl, DownloadProgressHandler},
        download_progress::DownloadProgress,
        download_rate_limiter::DownloadRateLimiter,
        error::{AppError, AppResult},
    },
    domain::download::TransferValidator,
};

const CHUNK_BUFFER_SIZE: usize = 64 * 1024;
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(250);

pub async fn stream_ftp_payload(
    mut data_stream: TcpStream,
    destination_file: &mut File,
    initial_downloaded: u64,
    total_size: Option<u64>,
    validator: Option<TransferValidator>,
    progress_handler: &DownloadProgressHandler,
    control: &mut DownloadControl,
    rate_limiter: Arc<DownloadRateLimiter>,
) -> AppResult<u64> {
    let mut downloaded = initial_downloaded;
    let mut buffer = [0u8; CHUNK_BUFFER_SIZE];
    let mut last_tick = Instant::now();
    let mut bytes_since_tick: u64 = 0;

    loop {
        if let Some(reason) = control.current_stop_reason() {
            destination_file.flush().await.map_err(|e| {
                AppError::download(format!("Could not save partial FTP download: {e}"))
            })?;
            destination_file.sync_all().await.map_err(|e| {
                AppError::download(format!("Could not sync partial FTP download: {e}"))
            })?;
            return Err(stopped_error(reason));
        }

        let n = match data_stream.read(&mut buffer).await {
            Ok(0) => break, // EOF reached
            Ok(n) => n,
            Err(err) => {
                return Err(AppError::download(format!(
                    "FTP data connection read error: {err}"
                )));
            }
        };

        rate_limiter.throttle(n as u64).await;

        destination_file.write_all(&buffer[..n]).await.map_err(|e| {
            AppError::download(format!("Could not write to FTP download file: {e}"))
        })?;

        let chunk_size = n as u64;
        downloaded = downloaded
            .checked_add(chunk_size)
            .ok_or_else(|| AppError::download("Download size limit exceeded"))?;
        bytes_since_tick = bytes_since_tick
            .checked_add(chunk_size)
            .ok_or_else(|| AppError::download("Download progress counter overflow"))?;

        if last_tick.elapsed() >= PROGRESS_EVENT_INTERVAL {
            let elapsed = last_tick.elapsed().as_secs_f64().max(0.001);
            let speed = (bytes_since_tick as f64 / elapsed) as u64;
            progress_handler(DownloadProgress {
                downloaded,
                total: total_size,
                bytes_per_second: speed,
                connections_progress: Vec::new(),
                notice: None,
                transfer_kind: Some("ftp".to_string()),
                effective_concurrency: Some(1),
                etag: None,
                validator: validator.clone(),
            });
            last_tick = Instant::now();
            bytes_since_tick = 0;
        }
    }

    if let Some(reason) = control.current_stop_reason() {
        destination_file.flush().await.map_err(|e| {
            AppError::download(format!("Could not save partial FTP download: {e}"))
        })?;
        destination_file.sync_all().await.map_err(|e| {
            AppError::download(format!("Could not sync partial FTP download: {e}"))
        })?;
        return Err(stopped_error(reason));
    }

    // Emit final progress on completion
    progress_handler(DownloadProgress {
        downloaded,
        total: total_size.or(Some(downloaded)),
        bytes_per_second: 0,
        connections_progress: Vec::new(),
        notice: None,
        transfer_kind: Some("ftp".to_string()),
        effective_concurrency: Some(1),
        etag: None,
        validator,
    });

    destination_file.flush().await.map_err(|e| {
        AppError::download(format!("Could not flush FTP download file: {e}"))
    })?;
    destination_file.sync_all().await.map_err(|e| {
        AppError::download(format!("Could not sync FTP download file: {e}"))
    })?;

    Ok(downloaded)
}
