use std::path::{Path, PathBuf};

use crate::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::super::adapter::{MediaToolAdapter, ToolPlatform};

/// Linux media tool adapter for x64 and ARM64 builds.
#[derive(Clone, Copy, Debug)]
pub struct LinuxAdapter {
    platform: ToolPlatform,
}

impl LinuxAdapter {
    pub fn x64() -> Self {
        Self {
            platform: ToolPlatform::LinuxX64,
        }
    }

    pub fn arm64() -> Self {
        Self {
            platform: ToolPlatform::LinuxArm64,
        }
    }
}

impl MediaToolAdapter for LinuxAdapter {
    fn id(&self) -> &'static str {
        match self.platform {
            ToolPlatform::LinuxX64 => "linux-x64",
            ToolPlatform::LinuxArm64 => "linux-arm64",
            _ => "linux-unsupported",
        }
    }

    fn executable_name(&self, tool: MediaToolType) -> &'static str {
        match tool {
            MediaToolType::Ffmpeg => "ffmpeg",
            MediaToolType::YtDlp => "yt-dlp",
        }
    }

    fn download_spec(&self, tool: MediaToolType) -> AppResult<ToolDownloadSpec> {
        let url = match (tool, self.platform) {
            (MediaToolType::YtDlp, ToolPlatform::LinuxX64) => {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
            }
            (MediaToolType::YtDlp, ToolPlatform::LinuxArm64) => {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
            }
            (MediaToolType::Ffmpeg, ToolPlatform::LinuxX64) => {
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
            }
            (MediaToolType::Ffmpeg, ToolPlatform::LinuxArm64) => {
                return Err(AppError::download(
                    "Automated ffmpeg download is not available for Linux ARM64. Install ffmpeg with your package manager or set AD_FFMPEG_PATH.",
                ));
            }
            _ => {
                return Err(AppError::download(
                    "Linux media tools are only available for x64 and ARM64.",
                ));
            }
        };

        Ok(ToolDownloadSpec {
            url: url.to_owned(),
            binary_name: self.executable_name(tool).to_owned(),
        })
    }

    fn bundled_tool_candidates(&self, exe_name: &str, app_dir: &Path) -> Vec<PathBuf> {
        vec![
            app_dir.join("tools").join(exe_name),
            app_dir.join(exe_name),
            PathBuf::from("/usr/lib/open-download-manager/tools").join(exe_name),
            PathBuf::from("/usr/libexec/open-download-manager/tools").join(exe_name),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_native_binary_for_each_architecture() {
        let x64 = LinuxAdapter::x64()
            .download_spec(MediaToolType::YtDlp)
            .expect("Linux x64 yt-dlp spec");
        assert!(x64.url.ends_with("/yt-dlp_linux"));
        assert_eq!(x64.binary_name, "yt-dlp");

        let arm64 = LinuxAdapter::arm64()
            .download_spec(MediaToolType::YtDlp)
            .expect("Linux ARM64 yt-dlp spec");
        assert!(arm64.url.ends_with("/yt-dlp_linux_aarch64"));
        assert_eq!(arm64.binary_name, "yt-dlp");
    }

    #[test]
    fn rejects_arm64_ffmpeg_instead_of_falling_back_to_x64() {
        let error = LinuxAdapter::arm64()
            .download_spec(MediaToolType::Ffmpeg)
            .expect_err("Linux ARM64 ffmpeg must not silently use an x64 binary");

        assert!(
            error.message.contains("Linux ARM64"),
            "Error must explain the unsupported platform: {}",
            error.message
        );
    }

    #[test]
    fn probes_app_and_system_libexec_directories() {
        let app_dir = PathBuf::from("/opt/open-download-manager");
        let candidates = LinuxAdapter::x64().bundled_tool_candidates("ffmpeg", &app_dir);

        assert_eq!(
            candidates,
            vec![
                app_dir.join("tools").join("ffmpeg"),
                app_dir.join("ffmpeg"),
                PathBuf::from("/usr/lib/open-download-manager/tools/ffmpeg"),
                PathBuf::from("/usr/libexec/open-download-manager/tools/ffmpeg"),
            ]
        );
    }
}
