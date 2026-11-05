use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use awesome_downloader_lib::{
    application::download_history_service::DownloadHistoryService,
    application::download_transfer_service::DownloadTransferService,
    domain::{
        download::{DownloadAction, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::download_history_store::DownloadHistoryStore,
    infrastructure::http_download_executor::HttpDownloadExecutor,
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("awesome-downloader-transfer-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn settings(directory: PathBuf, connections: u8) -> Settings {
    let mut settings = Settings::new(directory);
    settings.connections = connections;
    settings
}

fn serve_once(status: &str, body: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    let status = status.to_owned();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("download request should connect");
        let mut request = [0; 1_024];
        let _ = stream.read(&mut request);
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("fixture response headers should be written");
        stream
            .write_all(body)
            .expect("fixture response body should be written");
    });
    format!("http://{address}/file.bin")
}

fn serve_multiple(body: Vec<u8>, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().expect("download request should connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1_024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream
                    .read(&mut chunk)
                    .expect("download request should be readable");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("fixture response headers should be written");
            stream
                .write_all(&body)
                .expect("fixture response body should be written");
        }
    });
    format!("http://{address}")
}

fn serve_segmented(body: Vec<u8>, requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        for _ in 0..requests {
            let (mut stream, _) = listener.accept().expect("download request should connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1_024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream
                    .read(&mut chunk)
                    .expect("download request should be readable");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
            }
            let request = String::from_utf8_lossy(&request);
            let range = request
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("range")
                        .then(|| value.trim().strip_prefix("bytes="))
                        .flatten()
                })
                .expect("range request should be present");
            let (start, end) = range.split_once('-').expect("range should have an end");
            let start: usize = start.parse().expect("range start should be numeric");
            let end: usize = if end.is_empty() {
                body.len() - 1
            } else {
                end.parse().expect("range end should be numeric")
            };
            let response_body = &body[start..=end];
            if write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nETag: \"fixture-v1\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                response_body.len(),
                body.len()
            )
            .is_err()
            {
                continue;
            }
            for part in response_body.chunks(64 * 1024) {
                if stream.write_all(part).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(2));
            }
        }
    });
    format!("http://{address}/file.bin")
}

fn serve_ranged(body: Vec<u8>, expected_requests: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("download request should connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1_024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream
                    .read(&mut chunk)
                    .expect("download request should be readable");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
            }
            let request = String::from_utf8_lossy(&request);
            let offset = request
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("range")
                        .then(|| {
                            value
                                .trim()
                                .strip_prefix("bytes=")?
                                .trim_end_matches('-')
                                .parse()
                                .ok()
                        })
                        .flatten()
                })
                .unwrap_or(0);
            let response_body = &body[offset..];
            let (status, range_header) = if offset > 0 {
                (
                    "206 Partial Content",
                    format!(
                        "Content-Range: bytes {}-{}/{}\r\n",
                        offset,
                        body.len() - 1,
                        body.len()
                    ),
                )
            } else {
                ("200 OK", String::new())
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n{range_header}Connection: close\r\n\r\n",
                response_body.len()
            )
            .expect("fixture response headers should be written");
            if offset == 0 {
                for part in response_body.chunks(64 * 1024) {
                    if stream.write_all(part).is_err() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            } else {
                let _ = stream.write_all(response_body);
            }
        }
    });
    format!("http://{address}/file.bin")
}

fn serve_then_succeed(first_status: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        for (status, body) in [(first_status, &b""[..]), ("200 OK", &b"retried bytes"[..])] {
            let (mut stream, _) = listener.accept().expect("download request should connect");
            let mut request = [0; 1_024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("fixture response headers should be written");
            stream
                .write_all(body)
                .expect("fixture response body should be written");
        }
    });
    format!("http://{address}/file.bin")
}

fn download_request(url: String, filename: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_owned()),
        directory: None,
        category: None,
        description: Some("fixture download".to_owned()),
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
    }
}

async fn wait_for_terminal_status(service: &DownloadHistoryService, id: &str) -> DownloadStatus {
    for _ in 0..200 {
        let record = service
            .downloads()
            .expect("history should remain readable")
            .into_iter()
            .find(|record| record.id == id)
            .expect("download should be present");
        if matches!(
            record.status,
            DownloadStatus::Completed
                | DownloadStatus::Failed
                | DownloadStatus::Paused
                | DownloadStatus::Cancelled
        ) {
            return record.status;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("download did not reach a terminal status");
}

async fn wait_for_downloaded_bytes(history: &DownloadHistoryService, id: &str) -> u64 {
    for _ in 0..300 {
        let record = history
            .downloads()
            .expect("history should remain readable")
            .into_iter()
            .find(|record| record.id == id)
            .expect("download should be present");
        if record.downloaded > 0 {
            return record.downloaded;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("download did not report progress");
}

#[tokio::test]
async fn streams_http_response_to_disk_and_persists_redacted_history() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let legacy_history = br#"{
        "version": 1,
        "downloads": [{
            "id": "legacy-download",
            "url": "https://example.test/legacy.bin",
            "filename": "legacy.bin",
            "status": "complete",
            "downloaded": 4,
            "createdAt": "2025-01-02T03:04:05.000Z"
        }]
    }"#;
    fs::write(directory.join("downloads.json"), legacy_history)
        .expect("legacy history should be written");
    let url = serve_once("200 OK", b"downloaded bytes");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 3);

    let ids = service
        .add(
            download_request(format!("{url}?token=private"), "result.bin"),
            &settings,
        )
        .expect("download should be accepted");
    assert_eq!(ids.len(), 1);
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("result.bin")).unwrap(),
        b"downloaded bytes"
    );

    let persisted = fs::read_to_string(directory.join("downloads-tauri.json"))
        .expect("Tauri history should be persisted");
    assert!(!persisted.contains("token=private"));
    assert!(!persisted.contains("legacy-download"));
    let loaded = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("persisted history should reload");
    assert_eq!(loaded.downloads().unwrap().len(), 2);
    assert_eq!(
        fs::read(directory.join("downloads.json")).unwrap(),
        legacy_history
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn failed_http_status_is_reported_without_leaving_a_partial_file() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let url = serve_once("404 Not Found", b"missing");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 3);

    let ids = service
        .add(download_request(url, "missing.bin"), &settings)
        .expect("request should be accepted before network response");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Failed
    );
    assert!(!directory.join("missing.bin").exists());
    assert!(!directory
        .join(format!(".awesome-downloader-{}.part", ids[0]))
        .exists());
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn queued_download_can_be_started_once_with_the_resume_action() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let url = serve_once("200 OK", b"queued bytes");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let mut request = download_request(url, "queued.bin");
    request.queue_only = true;

    let ids = service
        .add(request, &settings)
        .expect("queue-only download should be accepted");
    assert_eq!(
        history.downloads().unwrap()[0].status,
        DownloadStatus::Queued
    );
    service
        .action(&ids[0], DownloadAction::Resume)
        .expect("queued download should start");
    let duplicate = service
        .action(&ids[0], DownloadAction::Resume)
        .expect_err("a started download must not be spawned a second time");
    assert_eq!(duplicate.code, "download_error");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("queued.bin")).unwrap(),
        b"queued bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn paused_download_resumes_from_its_saved_byte_range() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = (0..5 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let url = serve_ranged(body.clone(), 2);
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "resumed.bin"), &settings)
        .expect("download should start");
    wait_for_downloaded_bytes(&history, &ids[0]).await;
    service
        .action(&ids[0], DownloadAction::Pause)
        .expect("active download should pause");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Paused
    );
    let partial = directory.join(format!(".awesome-downloader-{}.part", ids[0]));
    let partial_size = fs::metadata(&partial)
        .expect("paused download should retain its partial file")
        .len();
    assert!(partial_size > 0);
    assert!(partial_size < body.len() as u64);

    let mut resumed = false;
    for _ in 0..100 {
        match service.action(&ids[0], DownloadAction::Resume) {
            Ok(()) => {
                resumed = true;
                break;
            }
            Err(error) if error.message.contains("still stopping") => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => panic!("paused download should resume: {}", error.message),
        }
    }
    assert!(
        resumed,
        "paused transfer should finish stopping before resume"
    );
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("resumed.bin")).unwrap(),
        body,
        "resumed output should exactly match the source"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn cancelling_download_removes_partial_output() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = vec![42; 5 * 1024 * 1024];
    let url = serve_ranged(body, 1);
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "cancelled.bin"), &settings)
        .expect("download should start");
    wait_for_downloaded_bytes(&history, &ids[0]).await;
    service
        .action(&ids[0], DownloadAction::Cancel)
        .expect("active download should cancel");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Cancelled
    );
    for _ in 0..100 {
        if !directory
            .join(format!(".awesome-downloader-{}.part", ids[0]))
            .exists()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!directory
        .join(format!(".awesome-downloader-{}.part", ids[0]))
        .exists());
    assert!(!directory.join("cancelled.bin").exists());
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn failed_download_can_be_retried() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let url = serve_then_succeed("404 Not Found");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "retried.bin"), &settings)
        .expect("download should start");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Failed
    );
    service
        .action(&ids[0], DownloadAction::Retry)
        .expect("failed download should retry");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("retried.bin")).unwrap(),
        b"retried bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn transient_server_errors_are_retried_automatically() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let url = serve_then_succeed("503 Service Unavailable");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "transient-retry.bin"), &settings)
        .expect("download should be accepted");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("transient-retry.bin")).unwrap(),
        b"retried bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn cancelling_a_paused_download_removes_its_partial_file() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = vec![19; 5 * 1024 * 1024];
    let url = serve_ranged(body, 1);
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "paused-cancelled.bin"), &settings)
        .expect("download should start");
    wait_for_downloaded_bytes(&history, &ids[0]).await;
    service
        .action(&ids[0], DownloadAction::Pause)
        .expect("active download should pause");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Paused
    );
    service
        .action(&ids[0], DownloadAction::Cancel)
        .expect("paused download should cancel");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Cancelled
    );
    for _ in 0..100 {
        if !directory
            .join(format!(".awesome-downloader-{}.part", ids[0]))
            .exists()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!directory
        .join(format!(".awesome-downloader-{}.part", ids[0]))
        .exists());
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn scheduled_download_starts_after_restart_when_its_time_arrives() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let url = serve_once("200 OK", b"scheduled bytes");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history, executor, 1);
    let mut request = download_request(url, "scheduled.bin");
    request.scheduled_at =
        Some((chrono::Utc::now() + chrono::Duration::milliseconds(200)).to_rfc3339());

    let ids = service
        .add(request, &settings)
        .expect("scheduled download should be accepted");
    let restored_history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("scheduled download should reload from history");
    assert_eq!(
        restored_history.downloads().unwrap()[0].status,
        DownloadStatus::Scheduled
    );
    let restored_service = DownloadTransferService::new(
        restored_history.clone(),
        HttpDownloadExecutor::new().expect("HTTP executor should initialize"),
        1,
    );
    restored_service.start_scheduler();

    assert_eq!(
        wait_for_terminal_status(&restored_history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("scheduled.bin")).unwrap(),
        b"scheduled bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn configured_speed_limit_is_shared_across_concurrent_downloads() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let base_url = serve_multiple(vec![7; 32 * 1024], 2);
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 2);
    service.set_speed_limit(64);
    let mut request = download_request(format!("{base_url}/first.bin"), "first.bin");
    request.filename = None;
    request.urls.push(format!("{base_url}/second.bin"));
    let started = std::time::Instant::now();

    let ids = service
        .add(request, &settings)
        .expect("concurrent downloads should start");
    let first = wait_for_terminal_status(&history, &ids[0]);
    let second = wait_for_terminal_status(&history, &ids[1]);
    let (first, second) = tokio::join!(first, second);

    assert_eq!(first, DownloadStatus::Completed);
    assert_eq!(second, DownloadStatus::Completed);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "two 32 KiB downloads should share the configured 64 KiB/s limit"
    );
    assert_eq!(
        fs::metadata(directory.join("first.bin")).unwrap().len(),
        32 * 1024
    );
    assert_eq!(
        fs::metadata(directory.join("second.bin")).unwrap().len(),
        32 * 1024
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn supported_byte_ranges_use_configured_segments_and_assemble_in_order() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = (0..1024 * 1024)
        .map(|index| (index % 239) as u8)
        .collect::<Vec<_>>();
    let url = serve_segmented(body.clone(), 5);
    let settings = settings(directory.clone(), 4);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "segmented.bin"), &settings)
        .expect("segmented download should start");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("segmented.bin")).unwrap(),
        body,
        "assembled segments should preserve the original byte order"
    );
    let record = history.downloads().unwrap().remove(0);
    assert_eq!(record.connections_progress.len(), 4);
    assert!(record
        .connections_progress
        .iter()
        .all(|segment| segment.status == "complete"));
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn segmented_download_resumes_partial_ranges_after_pause() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = (0..2 * 1024 * 1024)
        .map(|index| (index % 241) as u8)
        .collect::<Vec<_>>();
    let url = serve_segmented(body.clone(), 10);
    let settings = settings(directory.clone(), 4);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "segmented-resume.bin"), &settings)
        .expect("segmented download should start");
    wait_for_downloaded_bytes(&history, &ids[0]).await;
    service
        .action(&ids[0], DownloadAction::Pause)
        .expect("segmented download should pause");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Paused
    );
    let partial_segments = fs::read_dir(&directory)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".segment-"))
        .count();
    assert!(partial_segments > 0);

    let mut resumed = false;
    for _ in 0..100 {
        match service.action(&ids[0], DownloadAction::Resume) {
            Ok(()) => {
                resumed = true;
                break;
            }
            Err(error) if error.message.contains("still stopping") => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => panic!("segmented download should resume: {}", error.message),
        }
    }
    assert!(
        resumed,
        "paused segments should finish stopping before resume"
    );
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("segmented-resume.bin")).unwrap(),
        body,
        "resumed segments should assemble to the original bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn server_without_byte_ranges_falls_back_to_single_stream() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = vec![23; 128 * 1024];
    let url = serve_multiple(body.clone(), 2);
    let settings = settings(directory.clone(), 4);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "fallback.bin"), &settings)
        .expect("download should fall back to a single stream");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(fs::read(directory.join("fallback.bin")).unwrap(), body);
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

fn serve_drop_midstream(body: Vec<u8>, drop_at: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        // First connection: send headers and only drop_at bytes, then drop stream
        {
            let (mut stream, _) = listener.accept().expect("first request should connect");
            let mut request = [0; 1_024];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("headers should be written");
            let _ = stream.write_all(&body[..drop_at]);
            // drop stream abruptly
        }
        // Second connection: should request Range: bytes=drop_at-
        {
            let (mut stream, _) = listener.accept().expect("resume request should connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1_024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream.read(&mut chunk).expect("request should be readable");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
            }
            let request_str = String::from_utf8_lossy(&request);
            assert!(
                request_str.contains(&format!("Range: bytes={drop_at}-"))
                    || request_str.contains(&format!("range: bytes={drop_at}-"))
            );
            let remaining = &body[drop_at..];
            write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
                remaining.len(),
                drop_at,
                body.len() - 1,
                body.len()
            )
            .expect("resume headers should be written");
            stream.write_all(remaining).expect("remaining body should be written");
        }
    });
    format!("http://{address}/drop.bin")
}

#[tokio::test]
async fn single_stream_recovers_from_midstream_connection_drop() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = (0..256 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let drop_at = 64 * 1024;
    let url = serve_drop_midstream(body.clone(), drop_at);
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "recovered.bin"), &settings)
        .expect("download should start");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("recovered.bin")).unwrap(),
        body,
        "recovered download should match full file bytes"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

fn serve_segmented_with_midstream_drop(
    body: Vec<u8>,
    expected_requests: usize,
    drop_offset_in_first_segment: usize,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        let mut dropped = false;
        for _ in 0..expected_requests {
            let (mut stream, _) = listener.accept().expect("request should connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1_024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream.read(&mut chunk).expect("readable");
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);
            }
            let request = String::from_utf8_lossy(&request);
            let range = request
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("range")
                        .then(|| value.trim().strip_prefix("bytes="))
                        .flatten()
                })
                .expect("range header required");
            let (start, end) = range.split_once('-').expect("range end required");
            let start: usize = start.parse().expect("valid start");
            let end: usize = end.parse().expect("valid end");

            // If this is the initial probe (bytes=0-0)
            if start == 0 && end == 0 {
                write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: 1\r\nContent-Range: bytes 0-0/{}\r\nETag: \"fixture-v1\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("probe write");
                let _ = stream.write_all(&body[0..1]);
                continue;
            }

            // If this is a segment starting at 0 and we haven't dropped yet, drop midway!
            if start == 0 && !dropped && drop_offset_in_first_segment < end {
                dropped = true;
                let response_body = &body[start..=end];
                write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nETag: \"fixture-v1\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                    response_body.len(),
                    body.len()
                )
                .expect("headers");
                let _ = stream.write_all(&response_body[..drop_offset_in_first_segment]);
                // drop connection abruptly
                continue;
            }

            // Normal or recovered range
            let response_body = &body[start..=end];
            write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nETag: \"fixture-v1\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                response_body.len(),
                body.len()
            )
            .expect("headers");
            let _ = stream.write_all(response_body);
        }
    });
    format!("http://{address}/segmented_drop.bin")
}

#[tokio::test]
async fn segmented_download_recovers_from_midstream_segment_drop() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = (0..512 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    // Probe (1) + 2 segments + 1 retry for the dropped segment = 4 requests
    let url = serve_segmented_with_midstream_drop(body.clone(), 4, 32 * 1024);
    let settings = settings(directory.clone(), 2);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "segmented_recovered.bin"), &settings)
        .expect("download should start");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("segmented_recovered.bin")).unwrap(),
        body,
        "assembled segmented download should match full file after midstream recovery"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}
