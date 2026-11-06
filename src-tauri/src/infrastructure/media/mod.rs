pub mod composite_executor;
pub mod ffmpeg;
pub mod hls_executor;
pub mod paired_executor;
pub mod sabr_executor;
pub mod tools;
pub mod ytdlp;

pub use composite_executor::CompositeDownloadExecutor;
pub use ffmpeg::FfmpegTools;
pub use hls_executor::HlsDownloadExecutor;
pub use paired_executor::PairedDownloadExecutor;
pub use sabr_executor::SabrDownloadExecutor;
pub use tools::{
    download_spec_for, find_tool_path, BoxFuture, HttpToolDownloaderClient, LinuxAdapter,
    MacosAdapter, MediaToolAdapter, MediaToolsManager, ToolDownloaderClient, ToolPathResolver,
    ToolPlatform, UnsupportedAdapter, WindowsAdapter,
};
pub use ytdlp::YtDlpTools;
