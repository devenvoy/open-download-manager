// Created: 2026-11-07
// Reason: feat(media): modularize download executors, enable native multi-track streaming, and fix progress UX
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Description: Chunk assembler combining segmented download files and executing atomic finalization.

use std::path::{Path, PathBuf};

use tokio::fs;

use crate::application::{
    download_result::DownloadResult,
    error::{AppError, AppResult},
};

use super::policy::TEMPORARY_DOWNLOAD_PREFIX;

pub(crate) fn segment_path(directory: &Path, id: &str, index: usize) -> PathBuf {
    directory.join(format!(
        "{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-{index}.part"
    ))
}

pub fn remove_partial_file(path: &Path) -> AppResult<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::download(format!(
            "Could not remove partial file {}: {error}",
            path.display()
        ))),
    }
}

pub(crate) async fn has_segment_parts(directory: &Path, id: &str) -> AppResult<bool> {
    let mut entries = match fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(_) => return Ok(false),
    };
    let prefix = format!("{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-");
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) && name_str.ends_with(".part") {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn remove_segment_parts(directory: &Path, id: &str) -> AppResult<()> {
    let mut entries = match fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    let prefix = format!("{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-");
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) && name_str.ends_with(".part") {
            let _ = fs::remove_file(entry.path()).await;
        }
    }
    Ok(())
}

pub(crate) fn remove_all_segment_parts_sync(directory: &Path, id: &str) -> AppResult<bool> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return Ok(false),
    };
    let marker = format!("{id}.segment-");
    let mut removed_any = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains(&marker) && name_str.ends_with(".part") {
            let _ = std::fs::remove_file(entry.path());
            removed_any = true;
        }
    }
    Ok(removed_any)
}

pub async fn finalize(
    _id: &str,
    temporary: PathBuf,
    destination: PathBuf,
    downloaded: u64,
    total: Option<u64>,
) -> AppResult<DownloadResult> {
    if fs::metadata(&destination)
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

    // Completeness check: if total length is known, ensure full range coverage before rename
    if let Some(expected_total) = total {
        if downloaded < expected_total {
            return Err(AppError::download(format!(
                "Cannot finalize incomplete download: received {downloaded} of {expected_total} bytes."
            )));
        }
        let ranges_path = super::range_map::range_map_path(&temporary);
        if fs::try_exists(&ranges_path).await.unwrap_or(false) {
            if let Ok(Some(map)) = super::range_map::RangeMap::load(&ranges_path, expected_total, 0) {
                if !map.is_complete() {
                    return Err(AppError::download(
                        "Cannot finalize incomplete download: sparse range map has gaps.",
                    ));
                }
            }
        }
    }

    match super::sparse_file::rename_with_av_retry(&temporary, &destination).await {
        Ok(()) => {
            let ranges_path = super::range_map::range_map_path(&temporary);
            let _ = fs::remove_file(ranges_path).await;
        }
        Err(err)
            if err.kind() == std::io::ErrorKind::CrossesDevices
                || err.raw_os_error() == Some(18)
                || err.raw_os_error() == Some(17) =>
        {
            let expected_len = fs::metadata(&temporary)
                .await
                .map(|m| m.len())
                .unwrap_or(downloaded);
            crate::infrastructure::fs_utils::verified_cross_volume_copy(
                &temporary,
                &destination,
                expected_len,
                _id,
            )
            .await?;
            let ranges_path = super::range_map::range_map_path(&temporary);
            let _ = fs::remove_file(ranges_path).await;
        }
        Err(error) => {
            return Err(AppError::download(format!(
                "Could not move the download into place: {error}"
            )));
        }
    }

    Ok(DownloadResult {
        downloaded,
        total: total.or(Some(downloaded)),
        path: destination,
    })
}
