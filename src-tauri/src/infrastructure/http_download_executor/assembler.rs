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

pub(crate) fn segment_path(directory: &Path, id: &str, index: u8) -> PathBuf {
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

pub(crate) async fn has_segment_parts(directory: &Path, id: &str, count: u8) -> AppResult<bool> {
    for index in 0..count {
        if fs::try_exists(segment_path(directory, id, index))
            .await
            .map_err(|error| {
                AppError::download(format!("Could not check a partial segment: {error}"))
            })?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) async fn remove_segment_parts(directory: &Path, id: &str, count: u8) -> AppResult<()> {
    for index in 0..count {
        match fs::remove_file(segment_path(directory, id, index)).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::download(format!(
                    "Could not remove a partial segment: {error}"
                )));
            }
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
    for (index, _) in segments.iter().enumerate() {
        let segment_index =
            u8::try_from(index).map_err(|_| AppError::download("The segment index is invalid."))?;
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
    id: &str,
    temporary: PathBuf,
    destination: PathBuf,
    downloaded: u64,
    total: Option<u64>,
) -> AppResult<DownloadResult> {
    fs::hard_link(&temporary, &destination)
        .await
        .map_err(|error| {
            AppError::download(format!("Could not move the download into place: {error}"))
        })?;
    if let Err(error) = fs::remove_file(&temporary).await {
        eprintln!("Could not remove temporary file for download {id}: {error}");
    }
    Ok(DownloadResult {
        downloaded,
        total: total.or(Some(downloaded)),
        path: destination,
    })
}
