// Created: 2026-11-05
// Reason: feat: add native HTTP download engine
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Integration tests verifying download transfer functionality, edge cases, and error recovery.

use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use open_download_manager_lib::{
    application::download_history_service::DownloadHistoryService,
    application::download_transfer_service::DownloadTransferService,
    domain::{
        download::{DownloadAction, BrowserRequestHeaders, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::download_history_store::DownloadHistoryStore,
    infrastructure::http_download_executor::HttpDownloadExecutor,
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-transfer-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn settings(directory: PathBuf, connections: u8) -> Settings {
    let mut settings = Settings::new(directory);
    settings.connections = connections;
    settings
}

fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut chunk = [0; 1_024];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let count = stream.read(&mut chunk).expect("request should be readable");
        if count == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8_lossy(&request).into_owned()
}

fn request_has_cookie(request: &str, expected: &str) -> bool {
    request.lines().any(|line| {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        name.eq_ignore_ascii_case("cookie")
            && value.split(';').any(|cookie| cookie.trim() == expected)
    })
}

fn serve_cookie_gated(
    body: Vec<u8>,
    required_cookie: &'static str,
) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_server = seen.clone();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("download request should connect");
        let request = read_http_request(&mut stream);
        let authorized = request_has_cookie(&request, required_cookie);
        seen_for_server.lock().unwrap().push(request);
        if authorized {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("fixture response headers should be written");
            stream
                .write_all(&body)
                .expect("fixture response body should be written");
        } else {
            write!(
                stream,
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .expect("fixture denial should be written");
        }
    });
    (format!("http://{address}/file.bin"), seen)
}

fn serve_redirect_then_truncated(
    body: Vec<u8>,
) -> (String, Arc<Mutex<Vec<String>>>, Arc<Mutex<Vec<String>>>) {
    let target_listener = TcpListener::bind("127.0.0.1:0").expect("target fixture should bind");
    let target_address = target_listener
        .local_addr()
        .expect("target fixture should expose its address");
    let target_seen = Arc::new(Mutex::new(Vec::new()));
    let target_seen_for_server = target_seen.clone();
    let full_body = body.clone();
    thread::spawn(move || {
        for attempt in 0..2 {
            let (mut stream, _) = target_listener
                .accept()
                .expect("target request should connect");
            let request = read_http_request(&mut stream);
            target_seen_for_server.lock().unwrap().push(request);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                full_body.len()
            )
            .expect("target response headers should be written");
            if attempt == 0 {
                // Close early so the client retries after following the redirect.
                stream
                    .write_all(&full_body[..full_body.len() / 2])
                    .expect("truncated target body should be written");
            } else {
                stream
                    .write_all(&full_body)
                    .expect("target response body should be written");
            }
        }
    });

    let redirect_listener = TcpListener::bind("127.0.0.1:0").expect("redirect fixture should bind");
    let redirect_address = redirect_listener
        .local_addr()
        .expect("redirect fixture should expose its address");
    let redirect_seen = Arc::new(Mutex::new(Vec::new()));
    let redirect_seen_for_server = redirect_seen.clone();
    thread::spawn(move || {
        let (mut stream, _) = redirect_listener
            .accept()
            .expect("redirect request should connect");
        let request = read_http_request(&mut stream);
        redirect_seen_for_server.lock().unwrap().push(request);
        write!(
            stream,
            "HTTP/1.1 302 Found\r\nLocation: http://localhost:{}/file.bin\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            target_address.port()
        )
        .expect("redirect response should be written");
    });

    (
        format!("http://{redirect_address}/file.bin"),
        redirect_seen,
        target_seen,
    )
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
    serve_segmented_gated(body, requests, None)
}

fn serve_segmented_gated(body: Vec<u8>, requests: usize, required_cookie: Option<&'static str>) -> String {
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
            let cookie_ok = required_cookie.is_none_or(|expected| {
                request.lines().any(|line| {
                    let Some((name, value)) = line.split_once(':') else {
                        return false;
                    };
                    name.eq_ignore_ascii_case("cookie")
                        && value.split(';').any(|cookie| cookie.trim() == expected)
                })
            });
            if !cookie_ok {
                let _ = write!(
                    stream,
                    "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                continue;
            }
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
        temp_directory: None,
        category: None,
        description: Some("fixture download".to_owned()),
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        queue_id: None,
        audio_url: None,
        kind: None,
        ..Default::default()
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
async fn browser_cookie_headers_are_replayed_for_session_gated_downloads() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = b"cookie-gated payload".to_vec();
    let (url, seen) = serve_cookie_gated(body.clone(), "session=secret");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 3);

    let mut request = download_request(url, "session.bin");
    request.request_headers = Some(BrowserRequestHeaders::sanitize(HashMap::from([(
        "cookie".to_owned(),
        "session=secret".to_owned(),
    )])));

    let ids = service.add(request, &settings).expect("request should be accepted");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(fs::read(directory.join("session.bin")).unwrap(), body);

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "the replay should hit the fixture once");
    assert!(request_has_cookie(&seen[0], "session=secret"));

    let record = history.download(&ids[0]).expect("record should exist");
    assert!(
        !record.request_headers.is_empty(),
        "headers must remain available for the active session"
    );
    let persisted = fs::read_to_string(directory.join("downloads-tauri.json"))
        .expect("Tauri history should be persisted");
    assert!(
        !persisted.contains("session=secret"),
        "session cookies must never be written to history"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn browser_cookie_headers_are_replayed_for_segmented_downloads() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body: Vec<u8> = (0..131_072_u32)
        .map(|index| (index % 251) as u8)
        .collect();
    let url = serve_segmented_gated(body.clone(), 3, Some("session=segmented"));
    let settings = settings(directory.clone(), 2);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 3);

    let mut request = download_request(url, "segmented.bin");
    request.request_headers = Some(BrowserRequestHeaders::sanitize(HashMap::from([(
        "cookie".to_owned(),
        "session=segmented".to_owned(),
    )])));

    let ids = service.add(request, &settings).expect("request should be accepted");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(fs::read(directory.join("segmented.bin")).unwrap(), body);
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn browser_cookie_headers_do_not_leak_to_redirect_target_on_retry() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body: Vec<u8> = (0..8_192_u32).map(|index| (index % 251) as u8).collect();
    let (url, redirect_seen, target_seen) = serve_redirect_then_truncated(body.clone());
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 3);

    let mut request = download_request(url, "redirected.bin");
    request.request_headers = Some(BrowserRequestHeaders::sanitize(HashMap::from([(
        "cookie".to_owned(),
        "session=secret".to_owned(),
    )])));

    let ids = service.add(request, &settings).expect("request should be accepted");
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(fs::read(directory.join("redirected.bin")).unwrap(), body);

    let redirect_seen = redirect_seen.lock().unwrap();
    assert_eq!(redirect_seen.len(), 1);
    assert!(request_has_cookie(&redirect_seen[0], "session=secret"));

    let target_seen = target_seen.lock().unwrap();
    assert_eq!(target_seen.len(), 2, "the truncated response should be retried");
    assert!(
        target_seen
            .iter()
            .all(|request| !request_has_cookie(request, "session=secret")),
        "cookies must never be forwarded to a redirect target: {target_seen:?}"
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
        .join(format!(".open-download-manager-{}.part", ids[0]))
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
    let partial = directory.join(format!(".open-download-manager-{}.part", ids[0]));
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
            .join(format!(".open-download-manager-{}.part", ids[0]))
            .exists()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!directory
        .join(format!(".open-download-manager-{}.part", ids[0]))
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
            .join(format!(".open-download-manager-{}.part", ids[0]))
            .exists()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!directory
        .join(format!(".open-download-manager-{}.part", ids[0]))
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
async fn segmented_download_dynamically_rebalances_chunks_and_assembles_in_order() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    // 5 MB file so initial chunks exceed the MIN_CHUNK_REASSIGN_BYTES (2 MB) threshold
    let body = (0..5 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let url = serve_segmented(body.clone(), 10);
    let settings = settings(directory.clone(), 2);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let ids = service
        .add(download_request(url, "rebalanced.bin"), &settings)
        .expect("segmented download should start");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("rebalanced.bin")).unwrap(),
        body,
        "dynamically reassigned and assembled chunks must match original data bit-for-bit"
    );
    let record = history.downloads().unwrap().remove(0);
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

fn serve_partial_then_changed_full(
    initial_body: Vec<u8>,
    new_full_body: Vec<u8>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");
    thread::spawn(move || {
        // Request 1: sends partial bytes
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n",
                initial_body.len()
            );
            let _ = stream.write_all(&initial_body[..10 * 1024]);
        }

        // Request 2: resume with If-Range / Range -> returns 200 OK with new_full_body because resource changed
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"v2\"\r\nConnection: close\r\n\r\n",
                new_full_body.len()
            );
            let _ = stream.write_all(&new_full_body);
        }
    });
    format!("http://{address}/changed_file.bin")
}

#[tokio::test]
async fn expired_media_link_can_be_refreshed_and_resumed_from_browser() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let expired_url = serve_once("403 Forbidden", b"forbidden");
    let valid_url = serve_once("200 OK", b"refreshed video stream content");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(expired_url, "refreshed_clip.mp4"), &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Failed
    );

    // Refresh the expired download with the fresh URL
    service
        .refresh_download(&ids[0], Some(valid_url), None, None, None)
        .expect("download should be refreshed");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("refreshed_clip.mp4")).unwrap(),
        b"refreshed video stream content"
    );

    let record = history.download(&ids[0]).unwrap();
    assert!(record.notice.as_deref().unwrap_or("").contains("refreshed"));
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn resource_changed_on_resume_explains_restart_and_preserves_correctness() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let initial_body = vec![7u8; 100 * 1024];
    let new_full_body = b"completely new content after server file changed".to_vec();
    let url = serve_partial_then_changed_full(initial_body, new_full_body.clone());
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "changed.bin"), &settings)
        .expect("download should be accepted");

    // Wait until download completes through midstream drop recovery and restart
    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("changed.bin")).unwrap(),
        new_full_body
    );

    let record = history.download(&ids[0]).unwrap();
    assert!(
        record
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("restarted")
            || record
                .notice
                .as_deref()
                .unwrap_or("")
                .contains("changed"),
        "notice should explain restart: {:?}",
        record.notice
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn immediate_cancellation_during_retry_wait_stops_promptly() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("fixture address");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 1024];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 30\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            );
        }
    });
    let url = format!("http://{address}/rate_limited.bin");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let start = Instant::now();
    let ids = service
        .add(download_request(url, "cancelled_retry.bin"), &settings)
        .expect("download should start");

    // Give it a brief moment to receive 429 and enter wait_before_retry
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Immediately cancel the download
    service
        .action(&ids[0], DownloadAction::Cancel)
        .expect("download should cancel");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Cancelled
    );

    // Assert it cancelled immediately rather than waiting 30 seconds
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "Cancellation took too long: {:?}",
        start.elapsed()
    );
    assert!(
        !directory
            .join(format!(".open-download-manager-{}.part", ids[0]))
            .exists(),
        "partial file should be removed on cancel"
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[test]
fn typed_validator_survives_serialization_and_migrates_safely() {
    use open_download_manager_lib::domain::download::{DownloadRecord, TransferValidator};

    // 1. Typed ETag survives serialization/deserialization
    let etag_val = TransferValidator::ETag("\"v1-hash\"".to_string());
    let json_etag = serde_json::to_string(&etag_val).unwrap();
    let de_etag: TransferValidator = serde_json::from_str(&json_etag).unwrap();
    assert_eq!(de_etag, etag_val);

    // 2. Typed LastModified survives serialization/deserialization
    let lm_val = TransferValidator::LastModified("Sun, 08 Nov 2026 01:00:00 GMT".to_string());
    let json_lm = serde_json::to_string(&lm_val).unwrap();
    let de_lm: TransferValidator = serde_json::from_str(&json_lm).unwrap();
    assert_eq!(de_lm, lm_val);

    // 3. DownloadRecord with typed validator
    let record = DownloadRecord {
        id: "rec-1".to_string(),
        url: "https://example.com/file".to_string(),
        filename: "file.bin".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: Some(100),
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        directory: Some("/tmp".to_string()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: None,
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        sabr: None,
        referer: None,
        youtube_format: None,
        transfer_kind: None,
        etag: None,
        validator: Some(lm_val.clone()),
        conflict_policy: None,
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        completion_actions: None,
        action_ledger: Vec::new(),
    };
    let serialized_rec = serde_json::to_string(&record).unwrap();
    let de_rec: DownloadRecord = serde_json::from_str(&serialized_rec).unwrap();
    assert_eq!(de_rec.validator, Some(lm_val));

    // 4. Backward-compatible migration: untyped safe quoted string in etag field
    let legacy_json = r#"{
        "id": "rec-legacy",
        "url": "https://example.com/file",
        "status": "paused",
        "etag": "\"legacy-strong-etag\""
    }"#;
    let legacy_rec: DownloadRecord = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(
        legacy_rec.resume_validator(),
        Some(TransferValidator::ETag("\"legacy-strong-etag\"".to_string()))
    );

    // 5. Unsafe/untyped string does not migrate to ETag and returns None
    let unsafe_legacy_json = r#"{
        "id": "rec-unsafe",
        "url": "https://example.com/file",
        "status": "paused",
        "etag": "unquoted-string"
    }"#;
    let unsafe_rec: DownloadRecord = serde_json::from_str(unsafe_legacy_json).unwrap();
    assert_eq!(unsafe_rec.resume_validator(), None);

    // 6. Unknown raw validator string in legacy document (like "private-validator") safely deserializes as None
    let private_val_json = r#"{
        "id": "rec-private",
        "url": "https://example.com/file",
        "status": "paused",
        "validator": "private-validator"
    }"#;
    let private_rec: DownloadRecord = serde_json::from_str(private_val_json).unwrap();
    assert_eq!(private_rec.validator, None);
    assert_eq!(private_rec.resume_validator(), None);
}

#[tokio::test]
async fn last_modified_validator_resumes_with_conditional_range_and_assembles() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let full_content = vec![42u8; 80 * 1024];
    let full_len = full_content.len();
    let body_clone = full_content.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("fixture address");
    let last_modified = "Sun, 08 Nov 2026 01:23:45 GMT";

    thread::spawn(move || {
        // Request 1: initial GET, send partial 20KB then drop
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {full_len}\r\nAccept-Ranges: bytes\r\nLast-Modified: {last_modified}\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(&body_clone[..20 * 1024]);
            let _ = stream.flush();
        }

        // Request 2: resume request, expect If-Range and Range headers
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let n = stream.read(&mut request).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&request[..n]).to_lowercase();
            assert!(
                req_str.contains("range: bytes=20480-"),
                "Range request header should be sent: {req_str}"
            );
            assert!(
                req_str.contains(&format!("if-range: {}", last_modified.to_lowercase())),
                "If-Range header with Last-Modified should be sent: {req_str}"
            );

            let remaining = &body_clone[20 * 1024..];
            let _ = write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 20480-{}/{full_len}\r\nContent-Length: {}\r\nLast-Modified: {last_modified}\r\nConnection: close\r\n\r\n",
                full_len - 1,
                remaining.len()
            );
            let _ = stream.write_all(remaining);
            let _ = stream.flush();
        }
    });

    let url = format!("http://{address}/test_lm.bin");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "test_lm.bin"), &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("test_lm.bin")).unwrap(),
        full_content
    );

    let record = history.download(&ids[0]).unwrap();
    assert_eq!(
        record.validator,
        Some(open_download_manager_lib::domain::download::TransferValidator::LastModified(
            last_modified.to_string()
        ))
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn weak_etag_with_last_modified_falls_back_to_last_modified_validator() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let full_content = vec![99u8; 60 * 1024];
    let full_len = full_content.len();
    let body_clone = full_content.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("fixture address");
    let last_modified = "Sat, 07 Nov 2026 12:00:00 GMT";

    thread::spawn(move || {
        // Request 1: Server sends weak ETag + Last-Modified
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {full_len}\r\nAccept-Ranges: bytes\r\nETag: W/\"weak-123\"\r\nLast-Modified: {last_modified}\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(&body_clone[..15 * 1024]);
            let _ = stream.flush();
        }

        // Request 2: Resumed request must NOT emit weak ETag; it must emit Last-Modified in If-Range!
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let n = stream.read(&mut request).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&request[..n]).to_lowercase();
            assert!(
                !req_str.contains("if-range: w/\"weak-123\""),
                "Weak ETag must NEVER be used in If-Range: {req_str}"
            );
            assert!(
                req_str.contains(&format!("if-range: {}", last_modified.to_lowercase())),
                "Last-Modified should be used in If-Range: {req_str}"
            );

            let remaining = &body_clone[15 * 1024..];
            let _ = write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 15360-{}/{full_len}\r\nContent-Length: {}\r\nLast-Modified: {last_modified}\r\nConnection: close\r\n\r\n",
                full_len - 1,
                remaining.len()
            );
            let _ = stream.write_all(remaining);
            let _ = stream.flush();
        }
    });

    let url = format!("http://{address}/weak_etag_lm.bin");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "weak_etag_lm.bin"), &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("weak_etag_lm.bin")).unwrap(),
        full_content
    );

    let record = history.download(&ids[0]).unwrap();
    assert_eq!(
        record.validator,
        Some(open_download_manager_lib::domain::download::TransferValidator::LastModified(
            last_modified.to_string()
        ))
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn server_200_ok_on_range_request_safely_restarts_and_discards_partial() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let initial_body = vec![1u8; 50 * 1024];
    let replacement_body = b"completely new content returned via 200 OK on resume".to_vec();
    let rep_len = replacement_body.len();
    let rep_clone = replacement_body.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("fixture address");

    thread::spawn(move || {
        // Request 1: initial GET, send partial then drop
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n",
                initial_body.len()
            );
            let _ = stream.write_all(&initial_body[..10 * 1024]);
            let _ = stream.flush();
        }

        // Request 2: resume with If-Range / Range -> Server responds with 200 OK (range ignored) and full new body
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0; 2048];
            let _ = stream.read(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {rep_len}\r\nAccept-Ranges: bytes\r\nETag: \"v2\"\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(&rep_clone);
            let _ = stream.flush();
        }
    });

    let url = format!("http://{address}/range_ignored.bin");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_request(url, "range_ignored.bin"), &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("range_ignored.bin")).unwrap(),
        replacement_body
    );

    let record = history.download(&ids[0]).unwrap();
    assert!(
        record
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("restarted")
            || record
                .notice
                .as_deref()
                .unwrap_or("")
                .contains("changed"),
        "notice should record restart: {:?}",
        record.notice
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn download_with_referer_passes_referer_header_to_server() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = vec![42_u8; 1024];

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("address");
    let body_clone = body.clone();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connect");
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while !request.windows(4).any(|b| b == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).expect("read");
            if n == 0 { break; }
            request.extend_from_slice(&chunk[..n]);
        }
        let req_str = String::from_utf8_lossy(&request).to_ascii_lowercase();
        let has_expected_referer = req_str.contains("referer: https://example.com/source-page");
        if has_expected_referer {
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_clone.len()
            ).unwrap();
            stream.write_all(&body_clone).unwrap();
        } else {
            write!(
                stream,
                "HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n"
            ).unwrap();
        }
        let _ = stream.flush();
    });

    let url = format!("http://{address}/protected.bin");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let mut req = download_request(url, "protected.bin");
    req.referer = Some("https://example.com/source-page".to_owned());

    let ids = service
        .add(req, &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("protected.bin")).unwrap(),
        body
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn segmented_download_with_referer_passes_referer_header_to_all_chunks() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = vec![77_u8; 128 * 1024];

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("address");
    let body_clone = body.clone();

    // 1 discovery request + 2 segment requests = 3 requests total
    thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().expect("connect");
            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|b| b == b"\r\n\r\n") {
                let n = stream.read(&mut chunk).expect("read");
                if n == 0 { break; }
                request.extend_from_slice(&chunk[..n]);
            }
            let req_str = String::from_utf8_lossy(&request).to_ascii_lowercase();
            let has_expected_referer = req_str.contains("referer: https://example.com/source-page");
            if !has_expected_referer {
                let _ = write!(stream, "HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n");
                let _ = stream.flush();
                continue;
            }
            let range = req_str
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.trim().eq_ignore_ascii_case("range")
                        .then(|| value.trim().strip_prefix("bytes="))
                        .flatten()
                })
                .expect("range");
            let (start, end) = range.split_once('-').expect("range start-end");
            let start: usize = start.parse().unwrap();
            let end: usize = end.parse().unwrap();
            let response_body = &body_clone[start..=end];
            let _ = write!(
                stream,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                response_body.len(),
                body_clone.len()
            );
            let _ = stream.write_all(response_body);
            let _ = stream.flush();
        }
    });

    let url = format!("http://{address}/segmented_protected.bin");
    let settings = settings(directory.clone(), 2);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let mut req = download_request(url, "segmented_protected.bin");
    req.referer = Some("https://example.com/source-page".to_owned());

    let ids = service
        .add(req, &settings)
        .expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );
    assert_eq!(
        fs::read(directory.join("segmented_protected.bin")).unwrap(),
        body
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn content_disposition_adopts_real_name_over_hash_token() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let body = b"Game installer binary content".to_vec();

    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("address");
    let body_clone = body.clone();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("connect");
        let mut request = Vec::new();
        let mut chunk = [0; 1024];
        while !request.windows(4).any(|b| b == b"\r\n\r\n") {
            let n = stream.read(&mut chunk).expect("read");
            if n == 0 { break; }
            request.extend_from_slice(&chunk[..n]);
        }
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Disposition: attachment; filename=\"Modern_Warfare_2.mkv\"\r\nConnection: close\r\n\r\n",
            body_clone.len()
        );
        let _ = stream.write_all(&body_clone);
        let _ = stream.flush();
    });

    let hash_path = "ADGPM2nhJU91qAJ2JVyWcD4LSbvspZQg2amY_trBT-J_TY66ezQRzuphGKxNxgrrE";
    let url = format!("http://{address}/{hash_path}");
    let settings = settings(directory.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let req = download_request(url, hash_path);
    let ids = service.add(req, &settings).expect("download should be accepted");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );

    // Verify file on disk has the REAL name, not the hash!
    assert!(directory.join("Modern_Warfare_2.mkv").exists());
    assert!(!directory.join(hash_path).exists());
    assert_eq!(
        fs::read(directory.join("Modern_Warfare_2.mkv")).unwrap(),
        body
    );
    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}

#[tokio::test]
async fn completed_download_resumed_completes_instantly_without_conflict() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");
    let file_path = directory.join("already_finished.bin");
    fs::write(&file_path, b"1234567890").expect("write file");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let settings = settings(directory.clone(), 1);

    let mut req = download_request("http://127.0.0.1:9999/already_finished.bin".to_string(), "already_finished.bin");
    req.queue_only = true;
    let ids = service.add(req, &settings).expect("download should be accepted");

    history.update_download(&ids[0], |d| {
        d.status = DownloadStatus::Paused;
        d.downloaded = 10;
        d.total = Some(10);
    }).expect("update download");

    // Resuming this download should recognize the completed file and finish as Completed without conflict
    service.action(&ids[0], DownloadAction::Resume).expect("action should succeed");

    assert_eq!(
        wait_for_terminal_status(&history, &ids[0]).await,
        DownloadStatus::Completed
    );

    let updated = history.download(&ids[0]).unwrap();
    assert_eq!(updated.status, DownloadStatus::Completed);
    assert_eq!(updated.downloaded, 10);
    assert_eq!(updated.error, None);

    fs::remove_dir_all(directory).expect("temporary directory should be removed");
}
