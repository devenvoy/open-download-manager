use std::path::{Path, PathBuf};

use crate::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::super::adapter::{MediaToolAdapter, ToolPlatform};

/// Windows media tool adapter for x64 and ARM64 builds.
#[derive(Clone, Copy, Debug)]
pub struct WindowsAdapter {
    platform: ToolPlatform,
}

impl WindowsAdapter {
    pub fn x64() -> Self {
        Self {
            platform: ToolPlatform::WindowsX64,
        }
    }

    pub fn arm64() -> Self {
        Self {
            platform: ToolPlatform::WindowsArm64,
        }
    }
}

impl MediaToolAdapter for WindowsAdapter {
    fn id(&self) -> &'static str {
        match self.platform {
            ToolPlatform::WindowsX64 => "windows-x64",
            ToolPlatform::WindowsArm64 => "windows-arm64",
            _ => "windows-unsupported",
        }
    }

    fn executable_name(&self, tool: MediaToolType) -> &'static str {
        match tool {
            MediaToolType::Ffmpeg => "ffmpeg.exe",
            MediaToolType::YtDlp => "yt-dlp.exe",
        }
    }

    fn download_spec(&self, tool: MediaToolType) -> AppResult<ToolDownloadSpec> {
        let url = match (tool, self.platform) {
            (MediaToolType::YtDlp, ToolPlatform::WindowsX64) => {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
            }
            (MediaToolType::YtDlp, ToolPlatform::WindowsArm64) => {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_arm64.exe"
            }
            (MediaToolType::Ffmpeg, ToolPlatform::WindowsX64) => {
                "https://github.com/GyanD/codexffmpeg/releases/download/7.1/ffmpeg-7.1-essentials_build.zip"
            }
            (MediaToolType::Ffmpeg, ToolPlatform::WindowsArm64) => {
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-winarm64-gpl.zip"
            }
            _ => {
                return Err(AppError::download(
                    "Windows media tools are only available for x64 and ARM64.",
                ));
            }
        };

        Ok(ToolDownloadSpec {
            url: url.to_owned(),
            binary_name: self.executable_name(tool).to_owned(),
        })
    }

    fn bundled_tool_candidates(&self, exe_name: &str, app_dir: &Path) -> Vec<PathBuf> {
        vec![app_dir.join("tools").join(exe_name), app_dir.join(exe_name)]
    }

    fn finalize_binary(&self, _path: &Path) -> AppResult<()> {
        // Windows has no executable permission bit.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_native_binary_for_each_architecture() {
        let arm64 = WindowsAdapter::arm64();
        let x64 = WindowsAdapter::x64();

        let ytdlp_arm64 = arm64
            .download_spec(MediaToolType::YtDlp)
            .expect("ARM64 yt-dlp spec");
        assert!(ytdlp_arm64.url.ends_with("/yt-dlp_arm64.exe"));
        assert_eq!(ytdlp_arm64.binary_name, "yt-dlp.exe");
        assert_eq!(arm64.executable_name(MediaToolType::YtDlp), "yt-dlp.exe");

        let ffmpeg_arm64 = arm64
            .download_spec(MediaToolType::Ffmpeg)
            .expect("ARM64 ffmpeg spec");
        assert!(ffmpeg_arm64.url.contains("winarm64"));
        assert!(ffmpeg_arm64.url.ends_with(".zip"));
        assert_eq!(ffmpeg_arm64.binary_name, "ffmpeg.exe");

        let ytdlp_x64 = x64
            .download_spec(MediaToolType::YtDlp)
            .expect("x64 yt-dlp spec");
        assert!(ytdlp_x64.url.ends_with("/yt-dlp.exe"));
        assert!(!ytdlp_x64.url.contains("arm64"));

        let ffmpeg_x64 = x64
            .download_spec(MediaToolType::Ffmpeg)
            .expect("x64 ffmpeg spec");
        assert!(!ffmpeg_x64.url.contains("winarm64"));
        assert_eq!(ffmpeg_x64.binary_name, "ffmpeg.exe");
    }

    #[test]
    fn probes_tools_directory_before_app_directory() {
        let app_dir = PathBuf::from(r"C:\Program Files\Open Download Manager");
        let candidates = WindowsAdapter::arm64().bundled_tool_candidates("ffmpeg.exe", &app_dir);

        assert_eq!(
            candidates,
            vec![
                app_dir.join("tools").join("ffmpeg.exe"),
                app_dir.join("ffmpeg.exe"),
            ]
        );
    }
}
