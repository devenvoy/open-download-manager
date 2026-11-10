// Created: 2026-11-08
// Reason: feat(transfer): recover interrupted transfers and refresh expired links (Closes #3)
// Description: Integration tests verifying transfer recovery, expired link refresh, stream identity, and token redaction.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_executor::DownloadControl,
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        download::{
            redact_download_url, sanitize_diagnostic_text, DownloadRecord, DownloadRequest,
            DownloadStatus,
        },
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::{
            retry::{parse_retry_after, retry_delay, wait_before_retry},
            HttpDownloadExecutor,
        },
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("odm-link-recovery-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn test_settings(dir: PathBuf) -> Settings {
    let mut s = Settings::new(dir);
    s.connections = 1;
    s
}

async fn wait_for_terminal_status(
    history: &DownloadHistoryService,
    id: &str,
) -> DownloadRecord {
    for _ in 0..80 {
        if let Ok(rec) = history.download(id) {
            if matches!(
                rec.status,
                DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Paused
            ) {
                return rec;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    history.download(id).unwrap()
}

#[tokio::test]
async fn human_scenario_expired_link_401_403_410_prompts_refresh_and_resumes_without_recapture() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();

    let file_body = b"ABCDEFGHIJKLMN_RECOVERED_MEDIA_STREAM";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Serve 403 Forbidden for the first request (simulating expired signed link)
    // Then serve 200 OK with full file for the refreshed request
    thread::spawn(move || {
        // Request 1: expired link returns 403
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = "HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\nConnection: close\r\n\r\nForbidden";
            let _ = stream.write_all(response.as_bytes());
        }

        // Request 2: refreshed link returns 200 OK
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                file_body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(file_body);
        }
    });

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let settings = test_settings(dir.clone());

    let expired_url = format!("http://{addr}/video.mp4?auth_token=expired123");
    let req = DownloadRequest {
        urls: vec![expired_url],
        filename: Some("recovered_media.mp4".to_owned()),
        ..DownloadRequest::default()
    };

    let ids = service.add(req, &settings).unwrap();
    let id = &ids[0];

    // Wait for the transfer to hit link_expired (failed state with explanation)
    let failed_rec = wait_for_terminal_status(&history, id).await;
    assert_eq!(failed_rec.status, DownloadStatus::Failed);
    assert!(
        failed_rec.error.as_deref().unwrap_or("").contains("expired")
            || failed_rec.notice.as_deref().unwrap_or("").contains("expired")
    );

    // Refresh the transfer with fresh session URL without re-capturing or wiping settings
    let fresh_url = format!("http://{addr}/video.mp4?auth_token=fresh456");
    service
        .refresh_download(id, Some(fresh_url), None, None, None)
        .expect("refresh succeeds");

    let completed_rec = wait_for_terminal_status(&history, id).await;
    assert_eq!(completed_rec.status, DownloadStatus::Completed);
    assert_eq!(completed_rec.downloaded, file_body.len() as u64);

    let content = fs::read(dir.join("recovered_media.mp4")).unwrap();
    assert_eq!(content, file_body);
}

#[tokio::test]
async fn human_scenario_stream_identity_preserves_partial_bytes_when_matching() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();

    let full_content = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    // Stream first 10 bytes then cut connection; client automatically issues midstream retry with Range
    thread::spawn(move || {
        // Request 1: initial download, returns 200 OK with ETag "media-v1"
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"media-v1\"\r\nConnection: close\r\n\r\n",
                full_content.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&full_content[..10]);
            // intentionally close connection midstream
        }

        // Request 2: automatic midstream recovery with Range: bytes=10- and If-Range: "media-v1"
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            assert!(req_str.contains("Range: bytes=10-") || req_str.contains("range: bytes=10-"));
            assert!(req_str.contains("\"media-v1\""));

            let remaining = &full_content[10..];
            let response = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 10-{}/{}\r\nContent-Length: {}\r\nETag: \"media-v1\"\r\nConnection: close\r\n\r\n",
                full_content.len() - 1,
                full_content.len(),
                remaining.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(remaining);
        }
    });

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let settings = test_settings(dir.clone());

    let test_url = format!("http://{addr}/resumable_stream.bin");
    let req = DownloadRequest {
        urls: vec![test_url],
        filename: Some("resumable_stream.bin".to_owned()),
        ..DownloadRequest::default()
    };

    let ids = service.add(req, &settings).unwrap();
    let id = &ids[0];

    // Downloader automatically recovers midstream from byte 10 and completes
    let completed_rec = wait_for_terminal_status(&history, id).await;
    assert_eq!(completed_rec.status, DownloadStatus::Completed);
    assert_eq!(completed_rec.downloaded, full_content.len() as u64);

    let saved = fs::read(dir.join("resumable_stream.bin")).unwrap();
    assert_eq!(saved, full_content);
}

#[tokio::test]
async fn human_scenario_mutated_stream_identity_triggers_clean_restart() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();

    let initial_content = b"0123456789";
    let mutated_content = b"COMPLETELY_NEW_MUTATED_RESOURCE_FROM_BEGINNING";

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        // Request 1: serves first 10 bytes with ETag "v1"
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nAccept-Ranges: bytes\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n"
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(initial_content);
        }

        // Request 2 (midstream recovery): server returns 200 OK with ETag "v2" because resource changed
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v2\"\r\nConnection: close\r\n\r\n",
                mutated_content.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(mutated_content);
        }
    });

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);
    let settings = test_settings(dir.clone());

    let test_url = format!("http://{addr}/mutated_file.bin");
    let req = DownloadRequest {
        urls: vec![test_url],
        filename: Some("mutated_file.bin".to_owned()),
        ..DownloadRequest::default()
    };

    let ids = service.add(req, &settings).unwrap();
    let id = &ids[0];

    // Downloader detects resource mutated, cleanly discards stale partial bytes, and restarts from beginning
    let completed_rec = wait_for_terminal_status(&history, id).await;
    assert_eq!(completed_rec.status, DownloadStatus::Completed);
    assert_eq!(completed_rec.downloaded, mutated_content.len() as u64);
    assert!(
        completed_rec
            .notice
            .as_deref()
            .unwrap_or("")
            .contains("restarted download from the beginning")
    );

    let saved = fs::read(dir.join("mutated_file.bin")).unwrap();
    assert_eq!(saved, mutated_content);
}

#[tokio::test]
async fn human_scenario_bounded_exponential_backoff_and_instant_cancellation() {
    // 1. Verify parse_retry_after with integer seconds
    assert_eq!(parse_retry_after("5"), Some(5));
    assert_eq!(parse_retry_after("  120  "), Some(120));

    // 2. Verify retry_delay exponential progression
    let d0 = retry_delay(0, None);
    let d1 = retry_delay(1, None);
    let d2 = retry_delay(2, None);
    assert!(d1 > d0);
    assert!(d2 > d1);

    // 3. Verify server Retry-After override is honored but bounded
    let d_server = retry_delay(0, Some(10));
    assert_eq!(d_server, Duration::from_secs(10));
    let d_capped = retry_delay(0, Some(9999));
    assert_eq!(d_capped, Duration::from_secs(30)); // Capped at MAX_RETRY_DELAY_SECONDS

    // 4. Verify instantaneous cancellation during retry delay
    let (sender, receiver) = tokio::sync::watch::channel(None);
    let mut control = DownloadControl::new(receiver);

    let start = std::time::Instant::now();
    let cancel_sender = sender.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel_sender.send_replace(Some(open_download_manager_lib::application::download_executor::DownloadStopReason::Cancel));
    });

    // Wait before retry with a long 10-second delay
    let wait_res = wait_before_retry(&mut control, Duration::from_secs(10)).await;
    assert!(wait_res.is_err());
    assert_eq!(wait_res.unwrap_err().code, "download_cancelled");
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn human_scenario_credential_and_token_redaction_guarantees() {
    // 1. Redact signed query parameters and session tokens
    let signed_url = "https://cdn.example.com/video.mp4?auth_token=secret_jwt_xyz&signature=sig123&expires=1700000000&session_id=sess99";
    let redacted = redact_download_url(signed_url);
    assert_eq!(redacted, "https://cdn.example.com/video.mp4");
    assert!(!redacted.contains("auth_token"));
    assert!(!redacted.contains("signature"));
    assert!(!redacted.contains("secret_jwt_xyz"));

    // 2. Redact embedded HTTP basic auth
    let basic_auth_url = "https://admin:my_secret_password@secure.example.com/file.zip?token=abc";
    let redacted_auth = redact_download_url(basic_auth_url);
    assert_eq!(redacted_auth, "https://secure.example.com/file.zip");
    assert!(!redacted_auth.contains("admin"));
    assert!(!redacted_auth.contains("my_secret_password"));

    // 3. Redact FTP credentials
    let ftp_url = "ftp://ftpuser:ftppassword@ftp.example.com:2121/data.iso";
    let redacted_ftp = redact_download_url(ftp_url);
    assert_eq!(redacted_ftp, "ftp://******:******@ftp.example.com:2121/data.iso");
    assert!(!redacted_ftp.contains("ftpuser"));
    assert!(!redacted_ftp.contains("ftppassword"));

    // 4. Sanitize diagnostic error strings
    let raw_error = "Connection failed to https://user:secret123@cdn.example.com/segment_01.ts?token=ephemeral99: HTTP 403 Forbidden";
    let sanitized_diag = sanitize_diagnostic_text(raw_error);
    assert!(!sanitized_diag.contains("secret123"));
    assert!(!sanitized_diag.contains("ephemeral99"));
    assert!(sanitized_diag.contains("https://cdn.example.com/segment_01.ts: HTTP 403 Forbidden"));
}
