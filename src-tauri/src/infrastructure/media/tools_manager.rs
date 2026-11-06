use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use crate::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolStatus, MediaToolType, ToolDownloadSpec, ToolResolutionSource},
};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait abstracting binary downloads to allow deterministic mock testing without real HTTP network calls.
pub trait ToolDownloaderClient: Send + Sync {
    fn download_binary<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>>;
}

/// Production HTTP downloader client using reqwest
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

            let response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| AppError::download(format!("Failed to connect to media tool source: {e}")))?;

            if !response.status().is_success() {
                return Err(AppError::download(format!(
                    "Media tool download mirror returned status {}: {url}",
                    response.status()
                )));
            }

            let bytes = match response.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    let _ = tokio::fs::remove_file(&temp_dest).await;
                    return Err(AppError::download(format!("Failed to stream media tool binary: {e}")));
                }
            };

            if let Err(e) = tokio::fs::write(&temp_dest, &bytes).await {
                let _ = tokio::fs::remove_file(&temp_dest).await;
                return Err(AppError::download(format!("Failed to write media tool binary: {e}")));
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                if let Err(e) = std::fs::set_permissions(&temp_dest, perms) {
                    let _ = tokio::fs::remove_file(&temp_dest).await;
                    return Err(AppError::download(format!("Failed to set executable permissions: {e}")));
                }
            }

            if let Err(e) = tokio::fs::rename(&temp_dest, destination).await {
                let _ = tokio::fs::remove_file(&temp_dest).await;
                return Err(AppError::download(format!("Failed to atomically rename media tool binary: {e}")));
            }

            Ok(())
        })
    }
}

/// Path resolver that locates media tools across the four resolution tiers.
#[derive(Clone)]
pub struct ToolPathResolver {
    managed_dir_override: Option<PathBuf>,
}

impl ToolPathResolver {
    pub fn new() -> Self {
        Self {
            managed_dir_override: None,
        }
    }

    pub fn with_managed_dir(managed_dir: PathBuf) -> Self {
        Self {
            managed_dir_override: Some(managed_dir),
        }
    }

    /// Resolves the managed user data tools directory (`~/.open-download-manager/tools/`)
    pub fn managed_tools_dir(&self) -> Option<PathBuf> {
        if let Some(ref custom) = self.managed_dir_override {
            return Some(custom.clone());
        }

        let home = std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
            .map(PathBuf::from)?;

        Some(home.join(".open-download-manager").join("tools"))
    }

    /// Resolves a tool's executable location, identifying its resolution tier.
    pub fn resolve_tool(&self, tool: MediaToolType) -> Option<(PathBuf, ToolResolutionSource)> {
        // Tier 1: Environment variable override
        let env_key = tool.env_var_override();
        if let Ok(env_val) = std::env::var(env_key) {
            let path = PathBuf::from(env_val);
            if path.is_file() {
                return Some((path, ToolResolutionSource::EnvironmentOverride));
            }
        }

        let exe_name = tool.executable_name();

        // Tier 2: Bundled application sidecars
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                // <exe_dir>/tools/<tool>
                let sub_tools = parent.join("tools").join(exe_name);
                if sub_tools.is_file() {
                    return Some((sub_tools, ToolResolutionSource::BundledSidecar));
                }

                // <exe_dir>/<tool> (Tauri externalBin in Contents/MacOS/)
                let direct = parent.join(exe_name);
                if direct.is_file() {
                    return Some((direct, ToolResolutionSource::BundledSidecar));
                }

                // macOS .app bundle resources: Contents/Resources/tools/<tool>
                #[cfg(target_os = "macos")]
                if let Some(bundle_res) = parent.parent().map(|p| p.join("Resources").join("tools").join(exe_name)) {
                    if bundle_res.is_file() {
                        return Some((bundle_res, ToolResolutionSource::BundledSidecar));
                    }
                }

                // Linux libexec
                #[cfg(target_os = "linux")]
                for prefix in &["/usr/lib/open-download-manager/tools", "/usr/libexec/open-download-manager/tools"] {
                    let candidate = std::path::Path::new(prefix).join(exe_name);
                    if candidate.is_file() {
                        return Some((candidate, ToolResolutionSource::BundledSidecar));
                    }
                }
            }
        }

        // Tier 3: In-App Managed user directory
        if let Some(tools_dir) = self.managed_tools_dir() {
            let candidate = tools_dir.join(exe_name);
            if candidate.is_file() {
                return Some((candidate, ToolResolutionSource::ManagedUserData));
            }
        }

        // Tier 4: System PATH
        if let Ok(system_path) = which::which(tool.name()) {
            return Some((system_path, ToolResolutionSource::SystemPath));
        }

        #[cfg(target_os = "macos")]
        {
            // When launched from launchd/Chrome/Finder, PATH is minimal (/usr/bin:/bin:/usr/sbin:/sbin).
            // Query Apple's standard system utility /usr/libexec/path_helper which constructs the
            // complete system PATH from /etc/paths and /etc/paths.d without hardcoding any paths.
            if let Ok(output) = std::process::Command::new("/usr/libexec/path_helper")
                .arg("-s")
                .output()
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    if let Some(start) = text.find("PATH=\"") {
                        let rem = &text[start + 6..];
                        if let Some(end) = rem.find('"') {
                            for dir in std::env::split_paths(&rem[..end]) {
                                let candidate = dir.join(exe_name);
                                if candidate.is_file() {
                                    return Some((candidate, ToolResolutionSource::SystemPath));
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Evaluates current tool status
    pub fn tool_status(&self, tool: MediaToolType) -> MediaToolStatus {
        match self.resolve_tool(tool) {
            Some((path, source)) => MediaToolStatus {
                tool_type: tool,
                is_available: true,
                path: Some(path),
                source: Some(source),
            },
            None => MediaToolStatus {
                tool_type: tool,
                is_available: false,
                path: None,
                source: None,
            },
        }
    }
}

impl Default for ToolPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Service that coordinates media tool discovery, status inspection, and on-demand installation.
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

    /// Obtains the verified download specification for the current OS and architecture
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
                    binary_name: tool.executable_name().to_owned(),
                });
            }
        }

        match tool {
            MediaToolType::YtDlp => {
                #[cfg(target_os = "windows")]
                let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
                #[cfg(target_os = "macos")]
                let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
                #[cfg(target_os = "linux")]
                let url = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                return Err(AppError::download("Unsupported platform for automated yt-dlp download"));

                Ok(ToolDownloadSpec {
                    url: url.to_owned(),
                    binary_name: tool.executable_name().to_owned(),
                })
            }
            MediaToolType::Ffmpeg => {
                #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
                let url = "https://evermeet.cx/ffmpeg/getrelease/zip";
                #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
                let url = "https://evermeet.cx/ffmpeg/getrelease/zip";
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                let url = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";
                #[cfg(target_os = "windows")]
                let url = "https://github.com/GyanD/codexffmpeg/releases/download/7.1/ffmpeg-7.1-essentials_build.zip";
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                return Err(AppError::download("Unsupported platform for automated ffmpeg download"));

                Ok(ToolDownloadSpec {
                    url: url.to_owned(),
                    binary_name: tool.executable_name().to_owned(),
                })
            }
        }
    }

    /// Downloads and installs a tool into the managed tools directory from an explicit URL
    pub async fn install_managed_tool_from_url(
        &self,
        tool: MediaToolType,
        url: &str,
    ) -> AppResult<PathBuf> {
        let tools_dir = self
            .resolver
            .managed_tools_dir()
            .ok_or_else(|| AppError::download("Could not resolve tools directory"))?;

        tokio::fs::create_dir_all(&tools_dir)
            .await
            .map_err(|e| AppError::download(format!("Could not create tools directory: {e}")))?;

        let dest = tools_dir.join(tool.executable_name());
        let is_archive = url.contains(".zip") || url.contains(".tar.xz") || url.contains(".tar.gz");

        if is_archive {
            let archive_tmp = tools_dir.join(format!("{}.archive.tmp", tool.executable_name()));
            self.downloader.download_binary(url, &archive_tmp).await?;
            extract_executable_from_archive(&archive_tmp, tool.executable_name(), &dest).await?;
        } else {
            self.downloader.download_binary(url, &dest).await?;
        }
        Ok(dest)
    }

    /// Downloads and installs a tool into the managed tools directory
    pub async fn install_managed_tool(&self, tool: MediaToolType) -> AppResult<PathBuf> {
        let spec = self.download_spec(tool)?;
        self.install_managed_tool_from_url(tool, &spec.url).await
    }
}

async fn extract_executable_from_archive(
    archive: &Path,
    exe_name: &str,
    destination: &Path,
) -> AppResult<()> {
    let extract_dir = destination.with_extension("extracted_tmp");
    let _ = tokio::fs::remove_dir_all(&extract_dir).await;
    tokio::fs::create_dir_all(&extract_dir)
        .await
        .map_err(|e| AppError::download(format!("Failed to create extract directory: {e}")))?;

    let is_zip = archive.to_string_lossy().ends_with(".zip");
    let status = if is_zip {
        tokio::process::Command::new("unzip")
            .arg("-q")
            .arg("-o")
            .arg(archive)
            .arg("-d")
            .arg(&extract_dir)
            .status()
            .await
    } else {
        tokio::process::Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .await
    };

    let status = match status {
        Ok(s) => s,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&extract_dir).await;
            return Err(AppError::download(format!("Archive extraction failed: {e}")));
        }
    };

    if !status.success() {
        let _ = tokio::fs::remove_dir_all(&extract_dir).await;
        return Err(AppError::download("Failed to extract media tool archive."));
    }

    // Recursively locate exe_name in extract_dir
    let mut found = None;
    let mut stack = vec![extract_dir.clone()];
    while let Some(dir) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(exe_name) {
                found = Some(path);
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }

    let found_path = match found {
        Some(p) => p,
        None => {
            let _ = tokio::fs::remove_dir_all(&extract_dir).await;
            return Err(AppError::download(format!(
                "Could not find executable '{exe_name}' inside extracted archive"
            )));
        }
    };

    if let Err(_) = tokio::fs::rename(&found_path, destination).await {
        let _ = tokio::fs::copy(&found_path, destination).await;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(destination, perms);
    }

    let _ = tokio::fs::remove_dir_all(&extract_dir).await;
    let _ = tokio::fs::remove_file(archive).await;

    Ok(())
}

/// Global convenience helper used by executors
pub fn find_tool_path(tool_name: &str) -> Option<PathBuf> {
    let tool_type = MediaToolType::parse(tool_name)?;
    ToolPathResolver::new().resolve_tool(tool_type).map(|(p, _)| p)
}
