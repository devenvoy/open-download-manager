use std::path::{Path, PathBuf};

use crate::{
    application::error::AppResult,
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::super::adapter::{MediaToolAdapter, ToolPlatform};

/// macOS media tool adapter for Intel and Apple Silicon builds.
#[derive(Clone, Copy, Debug)]
pub struct MacosAdapter {
    platform: ToolPlatform,
}

impl MacosAdapter {
    pub fn x64() -> Self {
        Self {
            platform: ToolPlatform::MacosX64,
        }
    }

    pub fn arm64() -> Self {
        Self {
            platform: ToolPlatform::MacosArm64,
        }
    }
}

impl MediaToolAdapter for MacosAdapter {
    fn id(&self) -> &'static str {
        match self.platform {
            ToolPlatform::MacosX64 => "macos-x64",
            ToolPlatform::MacosArm64 => "macos-arm64",
            _ => "macos-unsupported",
        }
    }

    fn executable_name(&self, tool: MediaToolType) -> &'static str {
        match tool {
            MediaToolType::Ffmpeg => "ffmpeg",
            MediaToolType::YtDlp => "yt-dlp",
        }
    }

    fn download_spec(&self, tool: MediaToolType) -> AppResult<ToolDownloadSpec> {
        // yt-dlp_macos is a universal binary. evermeet.cx currently publishes
        // an x64 FFmpeg that runs under Rosetta 2 on Apple Silicon; no native
        // static macOS ARM64 FFmpeg build is available upstream.
        let url = match tool {
            MediaToolType::YtDlp => {
                "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
            }
            MediaToolType::Ffmpeg => "https://evermeet.cx/ffmpeg/getrelease/zip",
        };

        Ok(ToolDownloadSpec {
            url: url.to_owned(),
            binary_name: self.executable_name(tool).to_owned(),
        })
    }

    fn bundled_tool_candidates(&self, exe_name: &str, app_dir: &Path) -> Vec<PathBuf> {
        let mut candidates = vec![app_dir.join("tools").join(exe_name), app_dir.join(exe_name)];

        if let Some(contents_dir) = app_dir.parent() {
            candidates.push(contents_dir.join("Resources").join("tools").join(exe_name));
        }

        candidates
    }

    fn supplemental_path_dirs(&self) -> Vec<PathBuf> {
        // When launched from launchd, Finder, or a browser, PATH is minimal
        // (/usr/bin:/bin:/usr/sbin:/sbin). Ask Apple's path_helper for the
        // complete system PATH built from /etc/paths and /etc/paths.d.
        let output = match std::process::Command::new("/usr/libexec/path_helper")
            .arg("-s")
            .output()
        {
            Ok(output) if output.status.success() => output,
            _ => return Vec::new(),
        };

        let text = String::from_utf8_lossy(&output.stdout);
        let Some(start) = text.find("PATH=\"") else {
            return Vec::new();
        };
        let remainder = &text[start + 6..];
        let Some(end) = remainder.find('"') else {
            return Vec::new();
        };

        // macOS PATH entries are colon-separated. Parse them explicitly so the
        // adapter behaves identically when exercised from a non-macOS host.
        remainder[..end]
            .split(':')
            .filter(|entry| !entry.is_empty())
            .map(PathBuf::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_native_macos_binaries_for_both_architectures() {
        for adapter in [MacosAdapter::x64(), MacosAdapter::arm64()] {
            let ytdlp = adapter
                .download_spec(MediaToolType::YtDlp)
                .expect("macOS yt-dlp spec");
            assert!(ytdlp.url.ends_with("/yt-dlp_macos"));
            assert_eq!(ytdlp.binary_name, "yt-dlp");

            let ffmpeg = adapter
                .download_spec(MediaToolType::Ffmpeg)
                .expect("macOS ffmpeg spec");
            assert!(ffmpeg.url.ends_with("/getrelease/zip"));
            assert_eq!(ffmpeg.binary_name, "ffmpeg");
        }
    }

    #[test]
    fn probes_bundle_resources_after_executable_directory() {
        let app_dir = PathBuf::from("/Applications/Open Download Manager.app/Contents/MacOS");
        let candidates = MacosAdapter::arm64().bundled_tool_candidates("ffmpeg", &app_dir);

        assert_eq!(
            candidates,
            vec![
                app_dir.join("tools").join("ffmpeg"),
                app_dir.join("ffmpeg"),
                PathBuf::from(
                    "/Applications/Open Download Manager.app/Contents/Resources/tools/ffmpeg"
                ),
            ]
        );
    }
}
