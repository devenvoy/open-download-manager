use std::{
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

use crate::application::error::{AppError, AppResult};

/// Archive container detected from a media tool download URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArchiveFormat {
    Zip,
    TarXz,
    TarGz,
}

impl ArchiveFormat {
    pub(crate) fn from_url(url: &str) -> Option<Self> {
        let path = url
            .split(['?', '#'])
            .next()
            .unwrap_or(url)
            .to_ascii_lowercase();
        if path.ends_with(".zip") || path.ends_with("/zip") {
            Some(Self::Zip)
        } else if path.ends_with(".tar.xz") {
            Some(Self::TarXz)
        } else if path.ends_with(".tar.gz") {
            Some(Self::TarGz)
        } else {
            None
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarXz => "tar.xz",
            Self::TarGz => "tar.gz",
        }
    }
}

/// Extracts a single executable from an archive without invoking external
/// `unzip` or `tar` commands, which may not exist on stock Windows systems.
pub(crate) fn extract_executable_from_archive(
    archive: &Path,
    format: ArchiveFormat,
    exe_name: &str,
    destination: &Path,
) -> AppResult<()> {
    let result = match format {
        ArchiveFormat::Zip => extract_from_zip(archive, exe_name, destination),
        ArchiveFormat::TarXz => {
            let file = File::open(archive).map_err(|error| {
                AppError::download(format!(
                    "Could not open {} archive: {error}",
                    format.label()
                ))
            })?;
            extract_from_tar(xz2::read::XzDecoder::new(file), exe_name, destination)
        }
        ArchiveFormat::TarGz => {
            let file = File::open(archive).map_err(|error| {
                AppError::download(format!(
                    "Could not open {} archive: {error}",
                    format.label()
                ))
            })?;
            extract_from_tar(flate2::read::GzDecoder::new(file), exe_name, destination)
        }
    };

    if result.is_err() {
        let _ = std::fs::remove_file(destination);
    }

    result
}

fn extract_from_zip(archive: &Path, exe_name: &str, destination: &Path) -> AppResult<()> {
    let file = File::open(archive)
        .map_err(|error| AppError::download(format!("Could not open zip archive: {error}")))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| AppError::download(format!("Could not read zip archive: {error}")))?;

    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| AppError::download(format!("Could not read zip entry: {error}")))?;

        let entry_name = entry
            .name()
            .map_err(|error| AppError::download(format!("Could not read zip entry name: {error}")))?
            .to_string();

        if Path::new(&entry_name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(exe_name)
        {
            continue;
        }
        if !entry.is_file() {
            continue;
        }
        if entry.enclosed_name().is_none() {
            return Err(AppError::download(
                "Media tool archive contains an unsafe file path.",
            ));
        }

        return write_reader_to_file(&mut entry, destination);
    }

    Err(AppError::download(format!(
        "Could not find executable '{exe_name}' inside extracted archive"
    )))
}

fn extract_from_tar<R: Read>(reader: R, exe_name: &str, destination: &Path) -> AppResult<()> {
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|error| AppError::download(format!("Could not read tar archive: {error}")))?;

    for entry in entries {
        let mut entry = entry
            .map_err(|error| AppError::download(format!("Could not read tar entry: {error}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }

        let path = entry
            .path()
            .map_err(|error| AppError::download(format!("Could not read tar entry path: {error}")))?
            .to_path_buf();

        if path.file_name().and_then(|name| name.to_str()) != Some(exe_name) {
            continue;
        }
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(AppError::download(
                "Media tool archive contains an unsafe file path.",
            ));
        }

        return write_reader_to_file(&mut entry, destination);
    }

    Err(AppError::download(format!(
        "Could not find executable '{exe_name}' inside extracted archive"
    )))
}

fn write_reader_to_file<R: Read>(mut reader: R, destination: &Path) -> AppResult<()> {
    let mut output = File::create(destination).map_err(|error| {
        AppError::download(format!("Could not create media tool binary: {error}"))
    })?;
    io::copy(&mut reader, &mut output).map_err(|error| {
        AppError::download(format!("Could not extract media tool binary: {error}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ArchiveFormat;

    #[test]
    fn detects_archive_formats_from_urls() {
        assert_eq!(
            ArchiveFormat::from_url("https://example.test/build.zip?token=abc"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            ArchiveFormat::from_url("https://evermeet.cx/ffmpeg/getrelease/zip"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            ArchiveFormat::from_url("https://example.test/ffmpeg.tar.xz"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            ArchiveFormat::from_url("https://example.test/ffmpeg.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::from_url("https://example.test/yt-dlp.exe"),
            None
        );
    }
}
