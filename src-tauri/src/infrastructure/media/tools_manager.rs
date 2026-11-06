use std::path::PathBuf;
use crate::application::error::{AppError, AppResult};

/// Official, verified release URLs for standalone static binaries across platforms and architectures.
pub struct ToolDownloadSource {
    pub url: &'static str,
    pub archive_type: ToolArchiveType,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolArchiveType {
    /// Standalone raw executable binary (e.g. yt-dlp)
    DirectExecutable,
    /// Compressed gzip/tar or zip archive containing the binary
    GzipCompressed,
}

/// Resolves the managed user data directory for Open Download Manager tools:
/// `~/.open-download-manager/tools/`
pub fn managed_tools_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)?;

    Some(home.join(".open-download-manager").join("tools"))
}

/// Finds a binary by name using the production 4-tier resolution hierarchy:
/// 1. Environment Variable override (`AD_<TOOL>_PATH`)
/// 2. Application-bundled companion/sidecar binary
/// 3. In-App Managed user directory (`~/.open-download-manager/tools/`)
/// 4. System PATH (`which`)
pub fn find_tool_path(tool_name: &str) -> Option<PathBuf> {
    let env_var_name = match tool_name {
        "ffmpeg" => "AD_FFMPEG_PATH",
        "yt-dlp" => "AD_YTDLP_PATH",
        _ => "",
    };

    // 1. Environment variable override
    if !env_var_name.is_empty() {
        if let Ok(env_path) = std::env::var(env_var_name) {
            let path = PathBuf::from(env_path);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    #[cfg(windows)]
    let exe_name = format!("{tool_name}.exe");
    #[cfg(not(windows))]
    let exe_name = tool_name.to_owned();

    // 2. Bundled sidecar in application bundle
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            // Adjacent tools directory: <exe_dir>/tools/<tool>
            let candidate = parent.join("tools").join(&exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }

            // Direct adjacent binary: <exe_dir>/<tool> (e.g. Tauri externalBin in Contents/MacOS/)
            let direct = parent.join(&exe_name);
            if direct.is_file() {
                return Some(direct);
            }

            // macOS .app bundle resources: Contents/Resources/tools/<tool>
            #[cfg(target_os = "macos")]
            if let Some(bundle_res) = parent.parent().map(|p| p.join("Resources").join("tools").join(&exe_name)) {
                if bundle_res.is_file() {
                    return Some(bundle_res);
                }
            }

            // Linux standard libexec directory
            #[cfg(target_os = "linux")]
            for prefix in &["/usr/lib/open-download-manager/tools", "/usr/libexec/open-download-manager/tools"] {
                let candidate = std::path::Path::new(prefix).join(&exe_name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    // 3. Managed user data directory (~/.open-download-manager/tools/<tool>)
    if let Some(tools_dir) = managed_tools_dir() {
        let candidate = tools_dir.join(&exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // 4. System PATH fallback
    which::which(tool_name).ok()
}

/// Returns the official download source for a tool based on the host OS and architecture.
pub fn get_download_source(tool_name: &str) -> AppResult<ToolDownloadSource> {
    match tool_name {
        "yt-dlp" => {
            // yt-dlp publishes standalone single-binary executables for all platforms
            #[cfg(target_os = "windows")]
            return Ok(ToolDownloadSource {
                url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(target_os = "macos")]
            return Ok(ToolDownloadSource {
                url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(target_os = "linux")]
            return Ok(ToolDownloadSource {
                url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            Err(AppError::download(format!("Unsupported platform for automated yt-dlp download: {}", std::env::consts::OS)))
        }
        "ffmpeg" => {
            #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
            return Ok(ToolDownloadSource {
                url: "https://evermeet.cx/ffmpeg/getrelease/zip",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
            return Ok(ToolDownloadSource {
                url: "https://evermeet.cx/ffmpeg/getrelease/zip",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            return Ok(ToolDownloadSource {
                url: "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(target_os = "windows")]
            return Ok(ToolDownloadSource {
                url: "https://github.com/GyanD/codexffmpeg/releases/download/7.1/ffmpeg-7.1-essentials_build.zip",
                archive_type: ToolArchiveType::DirectExecutable,
            });

            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            Err(AppError::download(format!("Unsupported platform for automated ffmpeg download: {}", std::env::consts::OS)))
        }
        _ => Err(AppError::download(format!("Unknown media tool: {tool_name}"))),
    }
}

/// Downloads and sets up a standalone media tool into the managed tools directory.
pub async fn download_managed_tool(
    client: &reqwest::Client,
    tool_name: &str,
) -> AppResult<PathBuf> {
    let source = get_download_source(tool_name)?;
    let tools_dir = managed_tools_dir()
        .ok_or_else(|| AppError::download("Could not determine user data directory for tools."))?;

    tokio::fs::create_dir_all(&tools_dir)
        .await
        .map_err(|e| AppError::download(format!("Could not create tools directory: {e}")))?;

    #[cfg(windows)]
    let bin_name = format!("{tool_name}.exe");
    #[cfg(not(windows))]
    let bin_name = tool_name.to_owned();

    let target_path = tools_dir.join(&bin_name);
    let temp_path = tools_dir.join(format!("{bin_name}.download.tmp"));

    let response = client
        .get(source.url)
        .send()
        .await
        .map_err(|e| AppError::download(format!("Failed to connect to media tool release source: {e}")))?;

    if !response.status().is_success() {
        return Err(AppError::download(format!(
            "Download mirror returned HTTP status {}: {}",
            response.status(),
            source.url
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::download(format!("Failed to read media tool payload: {e}")))?;

    tokio::fs::write(&temp_path, &bytes)
        .await
        .map_err(|e| AppError::download(format!("Failed to write media tool to disk: {e}")))?;

    // On Unix, ensure the executable bit is set
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        if let Err(e) = std::fs::set_permissions(&temp_path, perms) {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(AppError::download(format!("Failed to mark binary as executable: {e}")));
        }
    }

    // Atomic rename into final location
    tokio::fs::rename(&temp_path, &target_path)
        .await
        .map_err(|e| AppError::download(format!("Failed to finalize media tool installation: {e}")))?;

    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_tools_dir_resolves_under_home() {
        let dir = managed_tools_dir();
        assert!(dir.is_some());
        let path = dir.unwrap();
        assert!(path.to_string_lossy().contains(".open-download-manager"));
        assert!(path.ends_with("tools"));
    }

    #[test]
    fn get_download_source_returns_valid_url_for_known_tools() {
        assert!(get_download_source("yt-dlp").is_ok());
        assert!(get_download_source("ffmpeg").is_ok());
        assert!(get_download_source("unknown_tool").is_err());
    }

    #[test]
    fn find_tool_path_respects_env_var_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let fake_bin = temp_dir.path().join("fake_ffmpeg");
        std::fs::write(&fake_bin, b"binary").unwrap();

        std::env::set_var("AD_FFMPEG_PATH", fake_bin.to_str().unwrap());
        let found = find_tool_path("ffmpeg");
        assert_eq!(found, Some(fake_bin));
        std::env::remove_var("AD_FFMPEG_PATH");
    }
}
