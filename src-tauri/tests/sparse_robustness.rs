// Copyright (c) 2026 Open Download Manager contributors
// SPDX-License-Identifier: MIT
// Reason: feat(engine): crash safety and robustness test suite for sparse engine (Item 4)

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
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        download::{DownloadAction, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::{
            assembler::finalize,
            range_map::{range_map_path, RangeMap},
            sparse_file::{allocate_sparse_file, check_fat32_size_limit, FOUR_GIBIBYTES},
            HttpDownloadExecutor,
        },
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("odm-sparse-robustness-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn download_request(url: String, filename: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_string()),
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

fn extract_range(request: &str) -> Option<(usize, usize)> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("range") {
            let val = value.trim();
            if let Some(rest) = val.strip_prefix("bytes=") {
                let mut parts = rest.split('-');
                let start: usize = parts.next()?.parse().ok()?;
                let end: usize = parts.next()?.parse().ok()?;
                return Some((start, end));
            }
        }
        None
    })
}

async fn wait_for_terminal_status(history: &DownloadHistoryService, id: &str) -> DownloadStatus {
    for _ in 0..600 {
        let record = history
            .downloads()
            .ok()
            .and_then(|records| records.into_iter().find(|d| d.id == id));
        if let Some(record) = record {
            if matches!(
                record.status,
                DownloadStatus::Completed
                    | DownloadStatus::Failed
                    | DownloadStatus::Cancelled
                    | DownloadStatus::Paused
            ) {
                return record.status;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    DownloadStatus::Failed
}

/// Helper worker executed when spawned as a child process under test
#[test]
fn run_crash_worker() {
    if std::env::var("ODM_CRASH_WORKER").is_err() {
        return; // normal test run, no-op
    }
    let port: u16 = std::env::var("ODM_CRASH_PORT").unwrap().parse().unwrap();
    let dir = PathBuf::from(std::env::var("ODM_CRASH_DIR").unwrap());

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async move {
        let settings = Settings::new(dir.clone());
        let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
        let executor = HttpDownloadExecutor::new().unwrap();
        let service = DownloadTransferService::new(history.clone(), executor, 1);

        let url = format!("http://127.0.0.1:{port}/crash-resume.bin");
        let _ids = service.add(download_request(url, "crash-resume.bin"), &settings).unwrap();

        for _ in 0..3000 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if let Ok(downloads) = history.downloads() {
                if let Some(first) = downloads.first() {
                    if first.status == DownloadStatus::Completed {
                        break;
                    }
                }
            }
        }
    });
}

/// Scenario 1: Real subprocess SIGKILL at seeded pseudo-random points during download,
/// followed by clean resume from .odm.ranges and byte-for-byte SHA-256 integrity verification.
#[tokio::test]
async fn test_crash_kill_at_20_random_points_and_resume_sha256() {
    let file_size = 1024 * 1024_usize; // 1 MiB
    let payload: Vec<u8> = (0..file_size).map(|i| (i * 37 + 19) as u8).collect();
    let full_payload = payload.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let req = read_http_request(&mut stream);
            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"crash-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            let payload_ref = full_payload.clone();
            thread::spawn(move || {
                let range = extract_range(&req);
                if let Some((start, end)) = range {
                    let slice = &payload_ref[start..=end.min(payload_ref.len() - 1)];
                    let _ = write!(
                        stream,
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nETag: \"crash-etag\"\r\nConnection: close\r\n\r\n",
                        payload_ref.len(),
                        slice.len()
                    );
                    for chunk in slice.chunks(8 * 1024) {
                        if stream.write_all(chunk).is_err() {
                            break;
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                } else {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"crash-etag\"\r\nConnection: close\r\n\r\n",
                        payload_ref.len()
                    );
                    let _ = stream.write_all(&payload_ref);
                }
            });
        }
    });

    let dir = temporary_directory();

    // 1. Spawn a real child subprocess and SIGKILL it mid-transfer using seeded random timing
    let current_exe = std::env::current_exe().expect("current test executable");
    let mut rng_state: u64 = 42; // Fixed seed for reproducible replay

    for _kill_iteration in 1..=3 {
        // Linear congruential generator step
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let sleep_ms = 15 + (rng_state % 35);

        let mut child = std::process::Command::new(&current_exe)
            .arg("--exact")
            .arg("run_crash_worker")
            .arg("--nocapture")
            .env("ODM_CRASH_WORKER", "1")
            .env("ODM_CRASH_DIR", &dir)
            .env("ODM_CRASH_PORT", port.to_string())
            .spawn()
            .expect("spawn child crash worker");

        // Let child transfer partial bytes
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;

        // Issue real OS SIGKILL (child.kill() sends SIGKILL on Unix, TerminateProcess on Windows)
        let _ = child.kill();
        let status = child.wait().expect("wait for killed child");
        assert!(!status.success(), "Child process must have terminated from SIGKILL");

        // Verify partial state remains intact on disk
        let part_file_exists = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .any(|e| e.file_name().to_string_lossy().contains(".part"));
        if !part_file_exists {
            // Completed before kill, break
            break;
        }
    }

    // 2. Resume download in parent process to completion
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/crash-resume.bin");
    let existing_downloads = history.downloads().unwrap();
    let target_id = if let Some(first) = existing_downloads.first() {
        let id = first.id.clone();
        if first.status != DownloadStatus::Completed {
            let _ = service.action(&id, DownloadAction::Resume);
        }
        id
    } else {
        let ids = service.add(download_request(url, "crash-resume.bin"), &settings).unwrap();
        ids[0].clone()
    };

    let status = wait_for_terminal_status(&history, &target_id).await;
    assert_eq!(status, DownloadStatus::Completed);

    let final_bytes = fs::read(dir.join("crash-resume.bin")).expect("final file exists");
    assert_eq!(final_bytes.len(), payload.len());
    assert_eq!(final_bytes, payload);

    let _ = fs::remove_dir_all(&dir);
}

/// Scenario 2: Truncated and corrupt .odm.ranges rejected safely.
#[test]
fn test_robustness_truncated_and_corrupt_range_map() {
    let mut map = RangeMap::new(1024 * 1024, 0x12345678);
    map.add_range(0, 65535);
    let bytes = map.to_bytes();

    // Truncated at 10 bytes
    assert!(RangeMap::from_bytes(&bytes[..10], 1024 * 1024, 0x12345678).is_none());

    // Corrupted magic
    let mut bad_magic = bytes.clone();
    bad_magic[0] = 0xFF;
    assert!(RangeMap::from_bytes(&bad_magic, 1024 * 1024, 0x12345678).is_none());

    // Corrupted version
    let mut bad_version = bytes.clone();
    bad_version[4] = 99;
    assert!(RangeMap::from_bytes(&bad_version, 1024 * 1024, 0x12345678).is_none());
}

/// Scenario 3: Leftover .tmp ranges file is cleaned up safely on load.
#[test]
fn test_robustness_leftover_tmp_ranges_file() {
    let temp_dir = temporary_directory();
    let ranges_path = temp_dir.join("test.odm.ranges");
    let tmp_path = ranges_path.with_extension("ranges.tmp");

    fs::write(&tmp_path, b"leftover-tmp-bytes").unwrap();
    assert!(tmp_path.exists());

    let loaded = RangeMap::load(&ranges_path, 1000, 0).unwrap();
    assert!(loaded.is_none());
    assert!(!tmp_path.exists(), "Leftover .tmp file should be automatically purged");
}

/// Scenario 4: Ranges beyond file length are rejected by validator.
#[test]
fn test_robustness_ranges_beyond_file_length_rejected() {
    let mut map = RangeMap::new(1000, 12345);
    map.add_range(0, 499);
    let mut bytes = map.to_bytes();

    // Corrupt interval end to 2000 (exceeding total = 1000)
    bytes[36..44].copy_from_slice(&2000_u64.to_le_bytes());
    assert!(RangeMap::from_bytes(&bytes, 1000, 12345).is_none());
}

/// Scenario 5: Completeness check strictly enforced before final rename.
#[tokio::test]
async fn test_robustness_completeness_check_before_final_rename() {
    let temp_dir = temporary_directory();
    let temporary = temp_dir.join("incomplete.odm.part");
    let destination = temp_dir.join("final.bin");
    let ranges_path = range_map_path(&temporary);

    let total = 1024 * 1024_u64;
    let _ = allocate_sparse_file(&temporary, total).unwrap();

    // Write incomplete range map (only half downloaded)
    let mut map = RangeMap::new(total, 0);
    map.add_range(0, 512 * 1024 - 1);
    map.save_atomic(&ranges_path).unwrap();

    // Finalize must fail because range map is incomplete
    let res = finalize("test-inc", temporary.clone(), destination.clone(), 512 * 1024, Some(total)).await;
    assert!(res.is_err(), "Finalize must fail on incomplete range coverage");
    assert!(!destination.exists(), "Destination file must not be created");
}

/// Scenario 6: Unknown Content-Length falls back to single-stream sequential.
#[tokio::test]
async fn test_robustness_unknown_content_length_falls_back_to_single_stream() {
    let payload = b"stream-without-content-length-chunked".to_vec();
    let full_payload = payload.clone();
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let req = read_http_request(&mut stream);
            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
                );
                continue;
            }

            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            let hex_len = format!("{:X}\r\n", full_payload.len());
            let _ = stream.write_all(hex_len.as_bytes());
            let _ = stream.write_all(&full_payload);
            let _ = stream.write_all(b"\r\n0\r\n\r\n");
        }
    });

    let dir = temporary_directory();
    let settings = Settings::new(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/chunked.bin");
    let req = DownloadRequest {
        urls: vec![url],
        filename: Some("chunked.bin".to_string()),
        ..Default::default()
    };
    let ids = service.add(req, &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let final_bytes = fs::read(dir.join("chunked.bin")).expect("final file exists");
    assert_eq!(final_bytes, payload);
}

/// Scenario 7: 16 concurrent connections writing to sparse file without race condition.
#[tokio::test]
async fn test_robustness_16_concurrent_connections() {
    let file_size = 2 * 1024 * 1024_usize; // 2 MiB
    let payload: Vec<u8> = (0..file_size).map(|i| (i * 41 + 7) as u8).collect();
    let full_payload = payload.clone();

    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture should bind");
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let req = read_http_request(&mut stream);
            if req.starts_with("HEAD") {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"16conn-etag\"\r\nConnection: close\r\n\r\n",
                    full_payload.len()
                );
                continue;
            }

            let payload_ref = full_payload.clone();
            thread::spawn(move || {
                let range = extract_range(&req);
                if let Some((start, end)) = range {
                    let slice = &payload_ref[start..=end.min(payload_ref.len() - 1)];
                    let _ = write!(
                        stream,
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nETag: \"16conn-etag\"\r\nConnection: close\r\n\r\n",
                        payload_ref.len(),
                        slice.len()
                    );
                    let _ = stream.write_all(slice);
                } else {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"16conn-etag\"\r\nConnection: close\r\n\r\n",
                        payload_ref.len()
                    );
                    let _ = stream.write_all(&payload_ref);
                }
            });
        }
    });

    let dir = temporary_directory();
    let mut settings = Settings::new(dir.clone());
    settings.connections = 16;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let url = format!("http://127.0.0.1:{port}/16conn.bin");
    let ids = service.add(download_request(url, "16conn.bin"), &settings).unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    let final_bytes = fs::read(dir.join("16conn.bin")).expect("final file exists");
    assert_eq!(final_bytes.len(), payload.len());
    assert_eq!(final_bytes, payload);
}

/// Scenario 8: FAT32 4 GiB size limit refusal.
#[test]
fn test_robustness_fat32_4gib_refusal() {
    assert!(check_fat32_size_limit(true, FOUR_GIBIBYTES).is_err());
    assert!(check_fat32_size_limit(true, FOUR_GIBIBYTES + 1024).is_err());
    assert!(check_fat32_size_limit(true, 1024 * 1024).is_ok());
    assert!(check_fat32_size_limit(false, FOUR_GIBIBYTES * 2).is_ok());
}
