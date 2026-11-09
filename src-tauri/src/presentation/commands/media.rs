// Created: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers for video format inspection, playlist parsing, and media tools management.

use std::sync::Arc;
use tauri::State;

use crate::{
    application::error::{AppError, AppResult},
    domain::{
        media::{playlist::PlaylistEntry, PlaylistMetadata, YouTubeFormatChoice},
        media_tools::{MediaToolStatus, MediaToolType},
        protocol::mms::{AsxPlaylist, AsxPlaylistParser, MmsProtocolDetector},
    },
    infrastructure::media::{ytdlp::YtDlpTools, MediaToolsManager},
    presentation::app_state::AppState,
};

#[tauri::command]
pub async fn get_playlist_info(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<PlaylistMetadata> {
    if MmsProtocolDetector::is_playlist_wrapper(&url) {
        let content = if url.starts_with("http://") || url.starts_with("https://") {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let resp = client.get(&url).send().await.map_err(|e| {
                AppError::download(format!("Could not fetch ASX playlist: {e}"))
            })?;
            let bytes = resp.bytes().await.map_err(|e| {
                AppError::download(format!("Could not read ASX playlist response: {e}"))
            })?;
            AsxPlaylistParser::parse_bytes(&bytes).map_err(AppError::download)?
        } else if std::path::Path::new(&url).exists() {
            let bytes = std::fs::read(&url).map_err(|e| {
                AppError::download(format!("Could not read ASX playlist file: {e}"))
            })?;
            AsxPlaylistParser::parse_bytes(&bytes).map_err(AppError::download)?
        } else {
            AsxPlaylistParser::parse_str(&url).map_err(AppError::download)?
        };

        let entries = content
            .entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| PlaylistEntry {
                id: format!("asx_{idx}"),
                title: entry.title.unwrap_or_else(|| format!("Media Track {}", idx + 1)),
                duration: None,
                url: entry.href,
                thumbnail: None,
                channel: None,
                index: idx,
            })
            .collect::<Vec<_>>();

        return Ok(PlaylistMetadata {
            id: "asx_playlist".to_string(),
            title: content.title.unwrap_or_else(|| "Windows Media Playlist".to_string()),
            description: Some("Extracted from ASX/WVX stream redirector.".to_string()),
            uploader: None,
            webpage_url: url,
            entry_count: entries.len(),
            entries,
        });
    }

    let ytdlp = state
        .ytdlp
        .clone()
        .or_else(|| YtDlpTools::new().map(Arc::new))
        .ok_or_else(|| {
            AppError::download(
                "yt-dlp is not available. Ensure yt-dlp is installed and available in PATH or the app tools directory.",
            )
        })?;
    ytdlp.fetch_playlist_info(&url).await
}

#[tauri::command]
pub async fn resolve_asx_playlist(
    url_or_content: String,
) -> AppResult<AsxPlaylist> {
    if url_or_content.starts_with("http://") || url_or_content.starts_with("https://") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let resp = client.get(&url_or_content).send().await.map_err(|e| {
            AppError::download(format!("Could not fetch ASX playlist: {e}"))
        })?;
        let bytes = resp.bytes().await.map_err(|e| {
            AppError::download(format!("Could not read ASX playlist response: {e}"))
        })?;
        AsxPlaylistParser::parse_bytes(&bytes).map_err(AppError::download)
    } else if std::path::Path::new(&url_or_content).exists() {
        let bytes = std::fs::read(&url_or_content).map_err(|e| {
            AppError::download(format!("Could not read ASX playlist file: {e}"))
        })?;
        AsxPlaylistParser::parse_bytes(&bytes).map_err(AppError::download)
    } else {
        AsxPlaylistParser::parse_str(&url_or_content).map_err(AppError::download)
    }
}

#[tauri::command]
pub async fn get_video_formats(
    state: State<'_, AppState>,
    url: String,
) -> AppResult<Vec<YouTubeFormatChoice>> {
    let ytdlp = state
        .ytdlp
        .clone()
        .or_else(|| YtDlpTools::new().map(Arc::new))
        .ok_or_else(|| {
            AppError::download(
                "yt-dlp is not available. Ensure yt-dlp is installed and available in PATH or the app tools directory.",
            )
        })?;
    ytdlp.fetch_formats(&url).await
}

#[tauri::command]
pub fn get_media_tools_status() -> AppResult<Vec<MediaToolStatus>> {
    let manager = MediaToolsManager::default_service();
    Ok(manager.status_all())
}

#[tauri::command]
pub async fn install_media_tool(
    tool: MediaToolType,
) -> AppResult<MediaToolStatus> {
    let manager = MediaToolsManager::default_service();
    manager.download_tool(tool).await
}
