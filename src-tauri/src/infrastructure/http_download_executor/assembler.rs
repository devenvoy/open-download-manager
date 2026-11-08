use std::path::{Path, PathBuf};

use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::{
    application::{
        download_executor::{stopped_error, DownloadControl},
        download_result::DownloadResult,
        error::{AppError, AppResult},
    },
    domain::download::DownloadSegmentProgress,
};

use super::policy::{CONCAT_BUFFER_SIZE, TEMPORARY_DOWNLOAD_PREFIX};

pub(crate) fn segment_path(directory: &Path, id: &str, index: usize) -> PathBuf {
    directory.join(format!(
        "{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-{index}.part"
    ))
}

pub(crate) fn remove_partial_file(path: &Path) -> AppResult<()> {
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

pub(crate) async fn remove_segment_parts(directory: &Path, id: &str) -> AppResult<()> {
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

pub(crate) fn remove_all_segment_parts_sync(directory: &Path, id: &str) -> AppResult<()> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    let prefix = format!("{TEMPORARY_DOWNLOAD_PREFIX}{id}.segment-");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&prefix) && name_str.ends_with(".part") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

pub(crate) async fn concat_segments(
    id: &str,
    segments: &[DownloadSegmentProgress],
    directory: &Path,
    temporary: &Path,
    control: &mut DownloadControl,
) -> AppResult<()> {
    let mut sorted_chunks: Vec<(usize, &DownloadSegmentProgress)> =
        segments.iter().enumerate().collect();
    sorted_chunks.sort_by_key(|(_, s)| s.start);

    let mut expected_start = 0_u64;
    for (_, s) in &sorted_chunks {
        if s.start != expected_start {
            return Err(AppError::download(format!(
                "Inconsistent chunk boundary during assembly: expected start {expected_start}, found {}",
                s.start
            )));
        }
        expected_start = s.end + 1;
    }

    let mut output = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(temporary)
        .await
        .map_err(|error| {
            AppError::download(format!("Could not assemble the download file: {error}"))
        })?;

    let mut buffer = vec![0; CONCAT_BUFFER_SIZE];
    for (segment_index, _) in sorted_chunks {
        let path = segment_path(directory, id, segment_index);
        let mut input = fs::File::open(path).await.map_err(|error| {
            AppError::download(format!("Could not read a completed segment: {error}"))
        })?;
        loop {
            let count = input.read(&mut buffer).await.map_err(|error| {
                AppError::download(format!("Could not read a completed segment: {error}"))
            })?;
            if count == 0 {
                break;
            }
            if let Some(reason) = control.current_stop_reason() {
                output.sync_all().await.map_err(|error| {
                    AppError::download(format!("Could not save the partial assembly: {error}"))
                })?;
                drop(output);
                fs::remove_file(temporary).await.map_err(|error| {
                    AppError::download(format!("Could not discard the partial assembly: {error}"))
                })?;
                return Err(stopped_error(reason));
            }
            output.write_all(&buffer[..count]).await.map_err(|error| {
                AppError::download(format!("Could not assemble the download file: {error}"))
            })?;
        }
    }
    if let Some(reason) = control.current_stop_reason() {
        output.sync_all().await.map_err(|error| {
            AppError::download(format!("Could not save the partial assembly: {error}"))
        })?;
        drop(output);
        fs::remove_file(temporary).await.map_err(|error| {
            AppError::download(format!("Could not discard the partial assembly: {error}"))
        })?;
        return Err(stopped_error(reason));
    }
    output.flush().await.map_err(|error| {
        AppError::download(format!("Could not finalize the assembled file: {error}"))
    })?;
    output.sync_all().await.map_err(|error| {
        AppError::download(format!("Could not finalize the assembled file: {error}"))
    })?;
    Ok(())
}

pub(crate) async fn finalize(
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

    match fs::rename(&temporary, &destination).await {
        Ok(()) => {}
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
