use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tokio::sync::watch;

use awesome_downloader_lib::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler},
        download_history_service::DownloadHistoryService,
        download_rate_limiter::DownloadRateLimiter,
        download_transfer_service::DownloadTransferService,
        settings_service::SettingsService,
    },
    domain::download::{DownloadRecord, DownloadRequest, DownloadStatus},
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        media::{ffmpeg::FfmpegTools, paired_executor::PairedDownloadExecutor},
        settings_store::SettingsStore,
    },
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(1);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-paired-test-{}-{id}",
        std::process::id()
    ))
}

fn serve_media_tracks(video_data: Vec<u8>, audio_data: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let address = listener.local_addr().expect("local address");

    thread::spawn(move || {
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(c) => c,
                Err(_) => break,
            };

            let mut request = [0; 1024];
            let n = match stream.read(&mut request) {
                Ok(n) if n > 0 => n,
                _ => continue,
            };

            let req_str = String::from_utf8_lossy(&request[..n]);
            let path = req_str
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("");
            let method = req_str
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().next())
                .unwrap_or("");

            if path == "/video.mp4" {
                if method == "HEAD" {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        video_data.len()
                    );
                } else {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        video_data.len()
                    );
                    let _ = stream.write_all(&video_data);
                }
            } else if path == "/audio.m4a" {
                if method == "HEAD" {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: audio/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        audio_data.len()
                    );
                } else {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: audio/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        audio_data.len()
                    );
                    let _ = stream.write_all(&audio_data);
                }
            } else {
                let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
            }
        }
    });

    format!("http://{address}")
}

#[test]
fn paired_request_validation_and_record_mapping() {
    let dir = temporary_directory();
    let settings = SettingsService::new(
        SettingsStore::new(dir.clone()),
        dir.clone(),
    )
    .expect("settings");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(dir.clone()))
        .expect("history");

    let mock_executor = awesome_downloader_lib::infrastructure::http_download_executor::HttpDownloadExecutor::new()
        .expect("mock http executor");

    let transfer = DownloadTransferService::new(history.clone(), mock_executor, 4);

    let req = DownloadRequest {
        urls: vec!["https://example.com/video_stream".to_string()],
        filename: Some("paired_video.mp4".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
        category: None,
        description: Some("Adaptive paired video".to_string()),
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        audio_url: Some("https://example.com/audio_stream".to_string()),
        kind: Some("paired".to_string()),
    };

    let ids = transfer.add(req, settings.settings()).expect("add download");
    assert_eq!(ids.len(), 1);

    let downloads = history.downloads().expect("downloads");
    let record = downloads.into_iter().find(|d| d.id == ids[0]).expect("download record");
    assert_eq!(record.kind.as_deref(), Some("paired"));
    assert_eq!(record.resumable, Some(false));
    assert_eq!(record.connections, 2);
    assert_eq!(
        record.audio_url.as_deref(),
        Some("https://example.com/audio_stream")
    );
    assert_eq!(record.status, DownloadStatus::Queued);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn paired_download_streams_both_tracks_and_muxes() {
    let ffmpeg = FfmpegTools::new();
    if ffmpeg.is_none() {
        eprintln!("Skipping full mux test: ffmpeg not in PATH");
        return;
    }

    let dir = temporary_directory();
    tokio::fs::create_dir_all(&dir).await.expect("create dir");

    // Generate real short audio and video tracks via ffmpeg for the test server
    let temp_v = dir.join("test_src_v.mp4");
    let temp_a = dir.join("test_src_a.m4a");

    let mut v_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    v_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "color=c=red:s=160x120:d=0.5",
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-t", "0.5",
        ])
        .arg(&temp_v);
    assert!(v_cmd.status().await.expect("v cmd").success());

    let mut a_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    a_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono",
            "-t", "0.5", "-c:a", "aac",
        ])
        .arg(&temp_a);
    assert!(a_cmd.status().await.expect("a cmd").success());

    let video_bytes = tokio::fs::read(&temp_v).await.expect("read v");
    let audio_bytes = tokio::fs::read(&temp_a).await.expect("read a");

    let base_url = serve_media_tracks(video_bytes.clone(), audio_bytes.clone());

    let download = DownloadRecord {
        id: "test-paired-1".to_string(),
        url: format!("{base_url}/video.mp4"),
        filename: "final_combined.mp4".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("paired".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 2,
        connections_progress: Vec::new(),
        audio_url: Some(format!("{base_url}/audio.m4a")),
    };

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let pe = progress_events.clone();
    let progress_handler: DownloadProgressHandler = Arc::new(move |p| {
        pe.lock().unwrap().push(p);
    });

    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));

    let executor = PairedDownloadExecutor::new(reqwest::Client::new());
    let result = executor
        .transfer(download.clone(), progress_handler, control, rate_limiter)
        .await
        .expect("transfer paired");

    // Output file must exist and have content
    assert!(result.path.exists());
    assert_eq!(result.path.file_name().unwrap(), "final_combined.mp4");
    assert!(result.downloaded > 0);

    // Temporary parts dir must be cleaned up
    let parts_dir = dir.join(format!(".awesome-downloader-paired-{}", download.id));
    assert!(!parts_dir.exists(), "parts dir should be deleted after mux");

    // Progress updates should have recorded 2 connections
    let events = progress_events.lock().unwrap();
    assert!(!events.is_empty());
    let last = events.last().unwrap();
    assert_eq!(last.connections_progress.len(), 2);

    let _ = tokio::fs::remove_dir_all(dir).await;
}

#[tokio::test]
async fn paired_download_converts_to_mp3() {
    let ffmpeg = FfmpegTools::new();
    if ffmpeg.is_none() {
        eprintln!("Skipping mp3 test: ffmpeg not in PATH");
        return;
    }

    let dir = temporary_directory();
    tokio::fs::create_dir_all(&dir).await.expect("create dir");

    let temp_v = dir.join("test_src_v_mp3.mp4");
    let temp_a = dir.join("test_src_a_mp3.m4a");

    let mut v_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    v_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "color=c=green:s=160x120:d=0.5",
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-t", "0.5",
        ])
        .arg(&temp_v);
    assert!(v_cmd.status().await.expect("v cmd").success());

    let mut a_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    a_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono",
            "-t", "0.5", "-c:a", "aac",
        ])
        .arg(&temp_a);
    assert!(a_cmd.status().await.expect("a cmd").success());

    let video_bytes = tokio::fs::read(&temp_v).await.expect("read v");
    let audio_bytes = tokio::fs::read(&temp_a).await.expect("read a");

    let base_url = serve_media_tracks(video_bytes, audio_bytes);

    let download = DownloadRecord {
        id: "test-paired-mp3".to_string(),
        url: format!("{base_url}/video.mp4"),
        filename: "final_soundtrack.mp3".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("paired".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 2,
        connections_progress: Vec::new(),
        audio_url: Some(format!("{base_url}/audio.m4a")),
    };

    let progress_handler: DownloadProgressHandler = Arc::new(|_| {});
    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));

    let executor = PairedDownloadExecutor::new(reqwest::Client::new());
    let result = executor
        .transfer(download, progress_handler, control, rate_limiter)
        .await
        .expect("transfer paired to mp3");

    assert!(result.path.exists());
    assert_eq!(result.path.file_name().unwrap(), "final_soundtrack.mp3");
    assert!(result.downloaded > 0);

    let _ = tokio::fs::remove_dir_all(dir).await;
}

#[test]
fn paired_download_discard_partial_cleans_up_directory() {
    let dir = temporary_directory();
    let download = DownloadRecord {
        id: "discard-paired-1".to_string(),
        url: "https://example.com/v".to_string(),
        filename: "file.mp4".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("paired".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 2,
        connections_progress: Vec::new(),
        audio_url: Some("https://example.com/a".to_string()),
    };

    let parts_dir = dir.join(format!(".awesome-downloader-paired-{}", download.id));
    std::fs::create_dir_all(&parts_dir).expect("create parts");
    assert!(parts_dir.exists());

    let executor = PairedDownloadExecutor::new(reqwest::Client::new());
    executor.discard_partial(&download).expect("discard");
    assert!(!parts_dir.exists());

    let _ = std::fs::remove_dir_all(dir);
}
