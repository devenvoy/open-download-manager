// Created: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: Platform-specific media tool adapter trait for locating and executing binary dependencies.

use std::path::{Path, PathBuf};

use crate::{
    application::error::AppResult,
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::binary::make_executable;

/// OS and CPU architecture used to select a tool binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPlatform {
    WindowsX64,
    WindowsArm64,
    MacosX64,
    MacosArm64,
    LinuxX64,
    LinuxArm64,
}

impl ToolPlatform {
    pub fn current() -> Option<Self> {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        {
            return Some(Self::WindowsX64);
        }
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        {
            return Some(Self::WindowsArm64);
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            return Some(Self::MacosX64);
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            return Some(Self::MacosArm64);
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return Some(Self::LinuxX64);
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            return Some(Self::LinuxArm64);
        }
        #[allow(unreachable_code)]
        None
    }
}

/// Platform-specific behavior required to resolve and install media tools.
///
/// Every adapter is compiled on every host. The running host is selected with
/// [`ToolPlatform::current`], while tests can exercise any adapter directly.
/// This keeps ARM64 behavior testable from macOS or Linux CI machines.
pub trait MediaToolAdapter: Send + Sync {
    /// Stable identifier used in diagnostics.
    fn id(&self) -> &'static str;

    /// Executable file name for this platform, including `.exe` on Windows.
    fn executable_name(&self, tool: MediaToolType) -> &'static str;

    /// Architecture-specific download source for this platform.
    ///
    /// Implementations must return an explicit error rather than silently
    /// selecting an x64 binary on an ARM64 host.
    fn download_spec(&self, tool: MediaToolType) -> AppResult<ToolDownloadSpec>;

    /// Bundled sidecar paths to probe, in priority order, for an app installed
    /// in `app_dir` (the directory containing the main executable).
    fn bundled_tool_candidates(&self, exe_name: &str, app_dir: &Path) -> Vec<PathBuf>;

    /// Additional directories to probe when the process PATH is minimal.
    fn supplemental_path_dirs(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// Applies platform-specific permissions after a binary is installed.
    fn finalize_binary(&self, path: &Path) -> AppResult<()> {
        make_executable(path)
    }
}
