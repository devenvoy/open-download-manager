// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-07
// Reason: feat(media): complete YouTube media pipeline, desktop quality selector, resilient download resumption, and native messaging registration
// Description: Media download infrastructure orchestrating specialized executors for adaptive streams.

pub mod composite_executor;
pub mod ffmpeg;
pub mod hls_executor;
pub mod paired_executor;
pub mod sabr_executor;
pub mod tools;
pub mod youtube_executor;
pub mod ytdlp;

pub use composite_executor::CompositeDownloadExecutor;
pub use ffmpeg::FfmpegTools;
pub use hls_executor::HlsDownloadExecutor;
pub use paired_executor::PairedDownloadExecutor;
pub use sabr_executor::SabrDownloadExecutor;
pub use youtube_executor::YoutubeDownloadExecutor;
pub use tools::{
    download_spec_for, find_tool_path, BoxFuture, HttpToolDownloaderClient, LinuxAdapter,
    MacosAdapter, MediaToolAdapter, MediaToolsManager, ToolDownloaderClient, ToolPathResolver,
    ToolPlatform, UnsupportedAdapter, WindowsAdapter,
};
pub use ytdlp::YtDlpTools;
