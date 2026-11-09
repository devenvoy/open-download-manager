// Created: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: Binary tool downloader retrieving sandboxed FFmpeg and yt-dlp builds from official mirrors.

use std::{future::Future, path::Path, pin::Pin};

use tokio::io::AsyncWriteExt;

use crate::application::error::{AppError, AppResult};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait abstracting binary downloads to allow deterministic mock testing
/// without real HTTP network calls.
pub trait ToolDownloaderClient: Send + Sync {
    fn download_binary<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>>;
}

/// Production HTTP downloader client using reqwest.
pub struct HttpToolDownloaderClient {
    client: reqwest::Client,
}

impl HttpToolDownloaderClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl Default for HttpToolDownloaderClient {
    fn default() -> Self {
        Self::new(reqwest::Client::new())
    }
}

impl ToolDownloaderClient for HttpToolDownloaderClient {
    fn download_binary<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>> {
        Box::pin(async move {
            let temp_dest = destination.with_extension("download.tmp");

            let mut response = self.client.get(url).send().await.map_err(|error| {
                AppError::download(format!("Failed to connect to media tool source: {error}"))
            })?;

            if !response.status().is_success() {
                return Err(AppError::download(format!(
                    "Media tool download mirror returned status {}: {url}",
                    response.status()
                )));
            }

            let expected_length = response.content_length();
            let mut output = tokio::fs::File::create(&temp_dest).await.map_err(|error| {
                AppError::download(format!("Failed to create media tool binary: {error}"))
            })?;
            let mut received = 0_u64;

            loop {
                let chunk = match response.chunk().await {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) => break,
                    Err(error) => {
                        drop(output);
                        let _ = tokio::fs::remove_file(&temp_dest).await;
                        return Err(AppError::download(format!(
                            "Failed to stream media tool binary: {error}"
                        )));
                    }
                };

                received += chunk.len() as u64;
                if let Err(error) = output.write_all(&chunk).await {
                    drop(output);
                    let _ = tokio::fs::remove_file(&temp_dest).await;
                    return Err(AppError::download(format!(
                        "Failed to write media tool binary: {error}"
                    )));
                }
            }

            if let Err(error) = output.flush().await {
                drop(output);
                let _ = tokio::fs::remove_file(&temp_dest).await;
                return Err(AppError::download(format!(
                    "Failed to flush media tool binary: {error}"
                )));
            }
            drop(output);

            if let Some(expected_length) = expected_length {
                if received != expected_length {
                    let _ = tokio::fs::remove_file(&temp_dest).await;
                    return Err(AppError::download(format!(
                        "Incomplete media tool download: expected {expected_length} bytes, received {received}"
                    )));
                }
            }

            if let Err(error) = tokio::fs::rename(&temp_dest, destination).await {
                let _ = tokio::fs::remove_file(&temp_dest).await;
                return Err(AppError::download(format!(
                    "Failed to atomically rename media tool binary: {error}"
                )));
            }

            Ok(())
        })
    }
}
