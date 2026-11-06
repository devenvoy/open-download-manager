use std::{path::PathBuf, sync::Arc};

use crate::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolStatus, MediaToolType, ToolDownloadSpec},
};

use super::{
    archive::{extract_executable_from_archive, ArchiveFormat},
    downloader::{HttpToolDownloaderClient, ToolDownloaderClient},
    resolver::ToolPathResolver,
};

/// Service that coordinates media tool discovery, status inspection, and
/// on-demand installation.
pub struct MediaToolsManager {
    resolver: ToolPathResolver,
    downloader: Arc<dyn ToolDownloaderClient>,
}

impl MediaToolsManager {
    pub fn new(resolver: ToolPathResolver, downloader: Arc<dyn ToolDownloaderClient>) -> Self {
        Self {
            resolver,
            downloader,
        }
    }

    pub fn default_service() -> Self {
        Self::new(
            ToolPathResolver::new(),
            Arc::new(HttpToolDownloaderClient::default()),
        )
    }

    pub fn resolver(&self) -> &ToolPathResolver {
        &self.resolver
    }

    pub fn status(&self, tool: MediaToolType) -> MediaToolStatus {
        self.resolver.tool_status(tool)
    }

    pub fn status_all(&self) -> Vec<MediaToolStatus> {
        vec![
            self.status(MediaToolType::Ffmpeg),
            self.status(MediaToolType::YtDlp),
        ]
    }

    pub fn find_path(&self, tool: MediaToolType) -> Option<PathBuf> {
        self.resolver.resolve_tool(tool).map(|(path, _)| path)
    }

    pub async fn download_tool(&self, tool: MediaToolType) -> AppResult<MediaToolStatus> {
        self.install_managed_tool(tool).await?;
        Ok(self.status(tool))
    }

    /// Obtains the verified download specification for the current OS and
    /// architecture.
    pub fn download_spec(&self, tool: MediaToolType) -> AppResult<ToolDownloadSpec> {
        let mirror_env = match tool {
            MediaToolType::Ffmpeg => "AD_FFMPEG_MIRROR",
            MediaToolType::YtDlp => "AD_YTDLP_MIRROR",
        };
        if let Ok(mirror_url) = std::env::var(mirror_env) {
            let trimmed = mirror_url.trim();
            if !trimmed.is_empty() {
                return Ok(ToolDownloadSpec {
                    url: trimmed.to_owned(),
                    binary_name: self.resolver.adapter().executable_name(tool).to_owned(),
                });
            }
        }

        self.resolver.adapter().download_spec(tool)
    }

    /// Downloads and installs a tool into the managed tools directory from an
    /// explicit URL.
    pub async fn install_managed_tool_from_url(
        &self,
        tool: MediaToolType,
        url: &str,
    ) -> AppResult<PathBuf> {
        let exe_name = self.resolver.adapter().executable_name(tool).to_owned();

        let tools_dir = self
            .resolver
            .managed_tools_dir()
            .ok_or_else(|| AppError::download("Could not resolve tools directory"))?;

        tokio::fs::create_dir_all(&tools_dir)
            .await
            .map_err(|error| {
                AppError::download(format!("Could not create tools directory: {error}"))
            })?;

        let destination = tools_dir.join(&exe_name);

        if let Some(format) = ArchiveFormat::from_url(url) {
            let archive_tmp = tools_dir.join(format!("{exe_name}.archive.tmp"));
            self.downloader.download_binary(url, &archive_tmp).await?;

            let archive_path = archive_tmp.clone();
            let extraction_destination = destination.clone();
            let archived_exe_name = exe_name.clone();
            let result = tokio::task::spawn_blocking(move || {
                extract_executable_from_archive(
                    &archive_path,
                    format,
                    &archived_exe_name,
                    &extraction_destination,
                )
            })
            .await;

            let _ = tokio::fs::remove_file(&archive_tmp).await;
            result.map_err(|error| {
                AppError::download(format!("Archive extraction task failed: {error}"))
            })??;
        } else {
            self.downloader.download_binary(url, &destination).await?;
        }

        self.resolver.adapter().finalize_binary(&destination)?;

        Ok(destination)
    }

    /// Downloads and installs a tool into the managed tools directory.
    pub async fn install_managed_tool(&self, tool: MediaToolType) -> AppResult<PathBuf> {
        let spec = self.download_spec(tool)?;
        self.install_managed_tool_from_url(tool, &spec.url).await
    }
}
