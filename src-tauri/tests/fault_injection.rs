// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(test-infra): clean-room HTTP fault injection integration tests for chaos network scenarios (W8)

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use open_download_manager_lib::{
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
        .prefix("open-download-manager-fault-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn download_request(url: String, filename: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_string()),
        directory: None,
        temp_directory: None,
        category: None,
        description: Some("fault injection test download".to_string()),
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        queue_id: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    }
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

fn extract_range_start(request: &str) -> Option<u64> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("range") {
            let val = value.trim();
            if let Some(rest) = val.strip_prefix("bytes=") {
                let start_str = rest.split('-').next()?;
                return start_str.parse().ok();
            }
        }
        None
    })
}

async fn wait_for_terminal_status(history: &DownloadHistoryService, id: &str) -> DownloadStatus {
    for _ in 0..600 {
        let record = history
            .downloads()
            .expect("history should remain readable")
            .into_iter()
            .find(|record| record.id == id)
            .expect("download should be present");
        if matches!(
            record.status,
            DownloadStatus::Completed
                | DownloadStatus::Failed
                | DownloadStatus::Cancelled
                | DownloadStatus::Paused
        ) {
            return record.status;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    panic!("download did not reach a terminal status within timeout");
}

/// Scenario 1: Forceful connection drop mid-transfer.
/// Server drops socket after writing first half; client reconnects with Range and finishes.
#[tokio::test]
async fn test_fault_drop_connection_and_retry_recovery() {
    let payload = (0..(256 * 1024)).map(|i| (i * 17 % 256) as u8).collect::<Vec<_>>();
    let full_payload = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let attempt = attempts_clone.fetch_add(1, Ordering::SeqCst);
            let req = read_http_request(&mut stream);
            let range_start = extract_range_start(&req).unwrap_or(0);

            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"drop-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            if attempt == 0 || range_start == 0 {
                // First GET attempt: send header + only 64 KiB, then abruptly close socket
                let half_slice = &full_payload[0..(64 * 1024)];
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"drop-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                let _ = stream.write_all(half_slice);
                // Drop stream abruptly
                drop(stream);
            } else {
                // Second attempt with Range request
                let slice = &full_payload[range_start as usize..];
                let _ = write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nETag: \"drop-etag\"\r\nConnection: close\r\n\r\n",
                    range_start,
                    full_payload.len() - 1,
                    full_payload.len(),
                    slice.len()
                );
                let _ = stream.write_all(slice);
            }
        }
    });

    let dir = temporary_directory();
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/drop-test.bin");
    let ids = service.add(download_request(url, "drop-test.bin"), &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let downloaded_bytes = fs::read(dir.join("drop-test.bin")).expect("downloaded file exists");
    assert_eq!(downloaded_bytes.len(), payload.len());
    assert_eq!(downloaded_bytes, payload);
    assert!(attempts.load(Ordering::SeqCst) >= 2, "Expected at least 2 attempts due to connection drop");
}

/// Scenario 2: HTTP 429 Rate Limiting with Retry-After header.
/// Server returns 429 on the first request, then succeeds on retry.
#[tokio::test]
async fn test_fault_http_429_rate_limiting_and_retry_after() {
    let payload = b"rate-limited-payload-recovered".to_vec();
    let full_payload = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    let requests_count = Arc::new(AtomicUsize::new(0));
    let requests_clone = requests_count.clone();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let count = requests_clone.fetch_add(1, Ordering::SeqCst);
            let req = read_http_request(&mut stream);

            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            if count == 0 {
                // First GET returns 429 with Retry-After: 1
                let _ = write!(
                    stream,
                    "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            } else {
                // Subsequent GET succeeds
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                let _ = stream.write_all(&full_payload);
            }
        }
    });

    let dir = temporary_directory();
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/429-test.bin");
    let ids = service.add(download_request(url, "429-test.bin"), &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let downloaded_bytes = fs::read(dir.join("429-test.bin")).unwrap();
    assert_eq!(downloaded_bytes, payload);
    assert!(requests_count.load(Ordering::SeqCst) >= 2, "Server should receive initial 429 and subsequent retry");
}

/// Scenario 3: Non-range server fallback to single stream.
/// When server indicates Accept-Ranges: none (or ignores Range), segmented download falls back gracefully.
#[tokio::test]
async fn test_fault_no_range_fallback_to_single_stream() {
    let payload = b"non-range-payload-streamed-cleanly".to_vec();
    let full_payload = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let req = read_http_request(&mut stream);
            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: none\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            // Always serve full 200 OK regardless of Range
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: none\r\nConnection: close\r\n\r\n",
                full_payload.len()
            );
            let _ = stream.write_all(&full_payload);
        }
    });

    let dir = temporary_directory();
    let mut settings = Settings::new(dir.clone());
    settings.connections = 4; // Request 4 connections
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/no-range.bin");
    let ids = service.add(download_request(url, "no-range.bin"), &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let downloaded_bytes = fs::read(dir.join("no-range.bin")).unwrap();
    assert_eq!(downloaded_bytes, payload);
}

/// Scenario 4: ETag flip / server-side mutation mid-transfer.
/// When server file changes, 412 Precondition Failed or ETag mismatch resets ranges safely.
#[tokio::test]
async fn test_fault_etag_flip_detects_server_file_change() {
    let payload_v2 = b"new-version-after-server-mutation".to_vec();
    let full_payload_v2 = payload_v2.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
            let req = read_http_request(&mut stream);

            if req.starts_with("HEAD") {
                // If call count is early, return v1 ETag, later return v2
                let etag = if count == 0 { "\"etag-v1\"" } else { "\"etag-v2\"" };
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: {}\r\nConnection: close\r\n\r\n",
                    full_payload_v2.len(),
                    etag
                );
                continue;
            }

            // Check If-Match or If-Range
            if req.contains("If-Match: \"etag-v1\"") || req.contains("if-match: \"etag-v1\"") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                continue;
            }

            // Otherwise deliver v2
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"etag-v2\"\r\nConnection: close\r\n\r\n",
                full_payload_v2.len()
            );
            let _ = stream.write_all(&full_payload_v2);
        }
    });

    let dir = temporary_directory();
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/etag-flip.bin");
    let ids = service.add(download_request(url, "etag-flip.bin"), &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let downloaded_bytes = fs::read(dir.join("etag-flip.bin")).unwrap();
    assert_eq!(downloaded_bytes, payload_v2);
}

/// Scenario 5: Pause and Resume integrity.
/// A stream is throttled, paused mid-flight, verified to safely persist, then resumed and completed.
#[tokio::test]
async fn test_fault_pause_and_resume_integrity() {
    let payload = (0..(512 * 1024)).map(|i| (i * 23 % 256) as u8).collect::<Vec<_>>();
    let full_payload = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let req = read_http_request(&mut stream);
            let range_start = extract_range_start(&req).unwrap_or(0);

            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"pause-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            if range_start > 0 {
                let slice = &full_payload[range_start as usize..];
                let _ = write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nETag: \"pause-etag\"\r\nConnection: close\r\n\r\n",
                    range_start,
                    full_payload.len() - 1,
                    full_payload.len(),
                    slice.len()
                );
                let _ = stream.write_all(slice);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"pause-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                // Send slowly in small 8 KiB chunks with delay so we can pause
                for chunk in full_payload.chunks(8 * 1024) {
                    if stream.write_all(chunk).is_err() {
                        break;
                    }
                    thread::sleep(Duration::from_millis(15));
                }
            }
        }
    });

    let dir = temporary_directory();
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/pause-resume.bin");
    let ids = service.add(download_request(url, "pause-resume.bin"), &settings).unwrap();

    // Wait until some bytes are downloaded
    for _ in 0..100 {
        let record = history.downloads().unwrap().into_iter().find(|r| r.id == ids[0]).unwrap();
        if record.downloaded > 16 * 1024 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Pause download
    service.action(&ids[0], DownloadAction::Pause).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check status is paused
    let record = history.downloads().unwrap().into_iter().find(|r| r.id == ids[0]).unwrap();
    assert_eq!(record.status, DownloadStatus::Paused);
    let paused_downloaded = record.downloaded;
    assert!(paused_downloaded > 0, "Should have downloaded some bytes before pause");

    // Resume download
    service.action(&ids[0], DownloadAction::Resume).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let downloaded_bytes = fs::read(dir.join("pause-resume.bin")).expect("final file exists");
    assert_eq!(downloaded_bytes.len(), payload.len());
    assert_eq!(downloaded_bytes, payload);
}
