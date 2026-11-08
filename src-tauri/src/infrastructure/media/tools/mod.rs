// Created: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Last updated: 2026-11-07
// Reason: fix(media): handle ffmpeg errors, yt-dlp execution, stream truncation and UI error dialogs
// Description: External media tool management module for FFmpeg and yt-dlp discovery and sandboxing.

mod adapter;
mod adapters;
mod archive;
mod binary;
mod downloader;
mod manager;
mod resolver;

use std::path::PathBuf;

use crate::domain::media_tools::MediaToolType;

pub use adapter::{MediaToolAdapter, ToolPlatform};
pub use adapters::{
    download_spec_for, LinuxAdapter, MacosAdapter, UnsupportedAdapter, WindowsAdapter,
};
pub use downloader::{BoxFuture, HttpToolDownloaderClient, ToolDownloaderClient};
pub use manager::MediaToolsManager;
pub use resolver::ToolPathResolver;

/// Global convenience helper used by executors.
pub fn find_tool_path(tool_name: &str) -> Option<PathBuf> {
    let tool_type = MediaToolType::parse(tool_name)?;
    ToolPathResolver::new()
        .resolve_tool(tool_type)
        .map(|(path, _)| path)
}
