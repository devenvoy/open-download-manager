// Created: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: Platform-specific adapter implementations for Windows, macOS, and Linux.

mod linux;
mod macos;
mod unsupported;
mod windows;

pub use linux::LinuxAdapter;
pub use macos::MacosAdapter;
pub use unsupported::UnsupportedAdapter;
pub use windows::WindowsAdapter;

use std::sync::Arc;

use crate::{
    application::error::AppResult,
    domain::media_tools::{MediaToolType, ToolDownloadSpec},
};

use super::adapter::{MediaToolAdapter, ToolPlatform};

/// Builds the adapter for an explicit platform. Useful for cross-platform tests
/// and for selecting a platform independently of the host.
pub(crate) fn adapter_for(platform: ToolPlatform) -> Arc<dyn MediaToolAdapter> {
    match platform {
        ToolPlatform::WindowsX64 => Arc::new(WindowsAdapter::x64()),
        ToolPlatform::WindowsArm64 => Arc::new(WindowsAdapter::arm64()),
        ToolPlatform::MacosX64 => Arc::new(MacosAdapter::x64()),
        ToolPlatform::MacosArm64 => Arc::new(MacosAdapter::arm64()),
        ToolPlatform::LinuxX64 => Arc::new(LinuxAdapter::x64()),
        ToolPlatform::LinuxArm64 => Arc::new(LinuxAdapter::arm64()),
    }
}

/// Selects the adapter matching the running host.
pub(crate) fn current_adapter() -> Arc<dyn MediaToolAdapter> {
    ToolPlatform::current()
        .map(adapter_for)
        .unwrap_or_else(|| Arc::new(UnsupportedAdapter))
}

/// Returns the architecture-specific download specification for a media tool.
///
/// Kept independent of the host platform so the full matrix is testable on any
/// CI machine.
pub fn download_spec_for(
    tool: MediaToolType,
    platform: ToolPlatform,
) -> AppResult<ToolDownloadSpec> {
    adapter_for(platform).download_spec(tool)
}
