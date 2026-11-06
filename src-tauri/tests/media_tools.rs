use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use open_download_manager_lib::{
    application::error::{AppError, AppResult},
    domain::media_tools::{MediaToolType, ToolResolutionSource},
    infrastructure::media::{
        BoxFuture, MediaToolsManager, ToolDownloaderClient, ToolPathResolver,
    },
};

/// Mock tool downloader client that records calls and writes mock bytes without network access.
struct MockToolDownloaderClient {
    download_count: AtomicUsize,
    download_result: Mutex<Option<Result<Vec<u8>, AppError>>>,
    last_url: Mutex<Option<String>>,
}

impl MockToolDownloaderClient {
    fn new(result: Result<Vec<u8>, AppError>) -> Self {
        Self {
            download_count: AtomicUsize::new(0),
            download_result: Mutex::new(Some(result)),
            last_url: Mutex::new(None),
        }
    }
}

impl ToolDownloaderClient for MockToolDownloaderClient {
    fn download_binary<'a>(
        &'a self,
        url: &'a str,
        destination: &'a Path,
    ) -> BoxFuture<'a, AppResult<()>> {
        self.download_count.fetch_add(1, Ordering::SeqCst);
        *self.last_url.lock().unwrap() = Some(url.to_owned());

        let res = self
            .download_result
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .unwrap_or_else(|| Ok(b"binary-payload".to_vec()));

        Box::pin(async move {
            match res {
                Ok(bytes) => {
                    tokio::fs::write(destination, &bytes)
                        .await
                        .map_err(|e| AppError::download(format!("Mock write failed: {e}")))?;
                    Ok(())
                }
                Err(err) => Err(err),
            }
        })
    }
}

#[test]
fn test_media_tool_type_parsing_and_names() {
    assert_eq!(MediaToolType::parse("ffmpeg"), Some(MediaToolType::Ffmpeg));
    assert_eq!(MediaToolType::parse("FFMPEG"), Some(MediaToolType::Ffmpeg));
    assert_eq!(MediaToolType::parse("yt-dlp"), Some(MediaToolType::YtDlp));
    assert_eq!(MediaToolType::parse("ytdlp"), Some(MediaToolType::YtDlp));
    assert_eq!(MediaToolType::parse("unknown"), None);

    assert_eq!(MediaToolType::Ffmpeg.name(), "ffmpeg");
    assert_eq!(MediaToolType::YtDlp.name(), "yt-dlp");
    assert_eq!(MediaToolType::Ffmpeg.env_var_override(), "AD_FFMPEG_PATH");
    assert_eq!(MediaToolType::YtDlp.env_var_override(), "AD_YTDLP_PATH");
}

#[test]
fn test_env_var_override_takes_precedence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let fake_ffmpeg = temp_dir.path().join("fake_ffmpeg");
    std::fs::write(&fake_ffmpeg, b"dummy-ffmpeg").unwrap();

    std::env::set_var("AD_FFMPEG_PATH", fake_ffmpeg.to_str().unwrap());

    let resolver = ToolPathResolver::new();
    let resolved = resolver.resolve_tool(MediaToolType::Ffmpeg);

    assert!(resolved.is_some());
    let (path, source) = resolved.unwrap();
    assert_eq!(path, fake_ffmpeg);
    assert_eq!(source, ToolResolutionSource::EnvironmentOverride);

    let status = resolver.tool_status(MediaToolType::Ffmpeg);
    assert!(status.is_available);
    assert_eq!(status.source, Some(ToolResolutionSource::EnvironmentOverride));

    std::env::remove_var("AD_FFMPEG_PATH");
}

#[test]
fn test_managed_user_directory_resolution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let managed_tools = temp_dir.path().join("tools");
    std::fs::create_dir_all(&managed_tools).unwrap();

    let exe_name = MediaToolType::YtDlp.executable_name();
    let fake_ytdlp = managed_tools.join(exe_name);
    std::fs::write(&fake_ytdlp, b"dummy-ytdlp").unwrap();

    let resolver = ToolPathResolver::with_managed_dir(managed_tools.clone());
    let resolved = resolver.resolve_tool(MediaToolType::YtDlp);

    assert!(resolved.is_some());
    let (path, source) = resolved.unwrap();
    assert_eq!(path, fake_ytdlp);
    assert_eq!(source, ToolResolutionSource::ManagedUserData);

    let status = resolver.tool_status(MediaToolType::YtDlp);
    assert!(status.is_available);
    assert_eq!(status.source, Some(ToolResolutionSource::ManagedUserData));
}

#[test]
fn test_missing_tool_status_reporting() {
    // Override with empty directory
    let temp_dir = tempfile::tempdir().unwrap();
    let empty_dir = temp_dir.path().join("empty_tools");
    std::fs::create_dir_all(&empty_dir).unwrap();

    // Use unique env key or ensure it is clear
    std::env::remove_var("AD_YTDLP_PATH");

    let resolver = ToolPathResolver::with_managed_dir(empty_dir);
    // If yt-dlp is not on system path:
    if which::which("yt-dlp").is_err() {
        let status = resolver.tool_status(MediaToolType::YtDlp);
        assert!(!status.is_available);
        assert!(status.path.is_none());
        assert!(status.source.is_none());
    }
}

#[test]
fn test_download_spec_urls_are_configured() {
    let resolver = ToolPathResolver::new();
    let mock_client = Arc::new(MockToolDownloaderClient::new(Ok(vec![])));
    let manager = MediaToolsManager::new(resolver, mock_client);

    let ffmpeg_spec = manager.download_spec(MediaToolType::Ffmpeg);
    assert!(ffmpeg_spec.is_ok());
    let spec = ffmpeg_spec.unwrap();
    assert!(spec.url.starts_with("https://"));

    let ytdlp_spec = manager.download_spec(MediaToolType::YtDlp);
    assert!(ytdlp_spec.is_ok());
    let spec = ytdlp_spec.unwrap();
    assert!(spec.url.starts_with("https://"));
}

#[tokio::test]
async fn test_mock_downloader_installs_tool_successfully() {
    let temp_dir = tempfile::tempdir().unwrap();
    let managed_tools = temp_dir.path().join("tools");

    let resolver = ToolPathResolver::with_managed_dir(managed_tools.clone());
    let mock_client = Arc::new(MockToolDownloaderClient::new(Ok(b"compiled-binary".to_vec())));
    let manager = MediaToolsManager::new(resolver, mock_client.clone());

    let installed_path = manager
        .install_managed_tool(MediaToolType::YtDlp)
        .await
        .expect("Install should succeed");

    assert_eq!(mock_client.download_count.load(Ordering::SeqCst), 1);
    assert!(installed_path.exists());
    let contents = std::fs::read(&installed_path).unwrap();
    assert_eq!(contents, b"compiled-binary");

    // After installation, resolver must find it in managed directory
    let status = manager.status(MediaToolType::YtDlp);
    assert!(status.is_available);
    assert_eq!(status.source, Some(ToolResolutionSource::ManagedUserData));
}

#[tokio::test]
async fn test_mock_downloader_handles_network_failure() {
    let temp_dir = tempfile::tempdir().unwrap();
    let managed_tools = temp_dir.path().join("tools");

    let resolver = ToolPathResolver::with_managed_dir(managed_tools.clone());
    let mock_client = Arc::new(MockToolDownloaderClient::new(Err(AppError::download(
        "HTTP 404 Not Found",
    ))));
    let manager = MediaToolsManager::new(resolver, mock_client.clone());

    let result = manager.install_managed_tool(MediaToolType::Ffmpeg).await;
    assert!(result.is_err());
    assert_eq!(mock_client.download_count.load(Ordering::SeqCst), 1);

    let err_msg = result.unwrap_err().message;
    assert!(err_msg.contains("404"));
}
