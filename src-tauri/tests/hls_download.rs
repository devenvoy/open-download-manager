// Created: 2026-11-05
// Reason: feat: add media handling for HLS, FFmpeg, yt-dlp, and stream progress
// Last updated: 2026-11-08
// Reason: feat(media): expand browser media panel and adaptive stream support (Closes #37)
// Description: Integration tests verifying hls download functionality, edge cases, and error recovery.

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
        download::{DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        media::CompositeDownloadExecutor,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-hls-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn serve_hls(pieces: Vec<Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");

    thread::spawn(move || {
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(conn) => conn,
                Err(_) => break,
            };
            let mut request = [0; 1024];
            let bytes_read = match stream.read(&mut request) {
                Ok(n) if n > 0 => n,
                _ => continue,
            };
            let req_str = String::from_utf8_lossy(&request[..bytes_read]);
            let path = req_str.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");

            if path == "/master.m3u8" {
                let body = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=1280x720\n/video.m3u8\n";
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            } else if path == "/video.m3u8" {
                let segments_list = pieces
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("#EXTINF:5.0,\n/{i}.ts"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let body = format!("#EXTM3U\n{segments_list}\n#EXT-X-ENDLIST\n");
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            } else if let Some(stripped) = path.strip_prefix('/') {
                if let Some(index_str) = stripped.strip_suffix(".ts") {
                    if let Ok(index) = index_str.parse::<usize>() {
                        if let Some(data) = pieces.get(index) {
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                data.len()
                            );
                            let _ = stream.write_all(data);
                        }
                    }
                }
            }
        }
    });

    format!("http://{address}/master.m3u8")
}

#[tokio::test]
async fn hls_vod_downloads_segments_and_assembles_in_order() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");

    let pieces = (0..4)
        .map(|i| vec![i as u8 + 1; 32 * 1024])
        .collect::<Vec<_>>();
    let mut expected_bytes = Vec::new();
    for p in &pieces {
        expected_bytes.extend_from_slice(p);
    }

    let url = serve_hls(pieces);
    let mut settings = Settings::new(directory.clone());
    settings.connections = 2;

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let http_executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let executor = CompositeDownloadExecutor::new(http_executor, reqwest::Client::new());
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let request = DownloadRequest {
        urls: vec![url],
        filename: None,
        directory: None,
        category: None,
        description: Some("HLS download test".to_owned()),
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    };

    let ids = service
        .add(request, &settings)
        .expect("HLS download should be accepted");

    for _ in 0..200 {
        let downloads = history.downloads().unwrap();
        let record = downloads.iter().find(|r| r.id == ids[0]).unwrap();
        if record.status == DownloadStatus::Completed {
            break;
        }
        if record.status == DownloadStatus::Failed {
            panic!("HLS download failed: {:?}", record.error);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let downloads = history.downloads().unwrap();
    let record = downloads.iter().find(|r| r.id == ids[0]).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);
    assert_eq!(record.filename, "master.ts");

    let output_path = directory.join("master.ts");
    assert!(output_path.exists());
    let content = fs::read(output_path).unwrap();
    assert_eq!(content, expected_bytes, "assembled HLS bytes must match segment order");

    fs::remove_dir_all(directory).expect("clean directory");
}

fn serve_byte_range_hls(file_bytes: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener
        .local_addr()
        .expect("local fixture should expose its address");

    let chunk_size = 32 * 1024_usize;
    let total_len = file_bytes.len();
    let num_chunks = (total_len + chunk_size - 1) / chunk_size;

    thread::spawn(move || {
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(conn) => conn,
                Err(_) => break,
            };
            let mut request = [0; 2048];
            let bytes_read = match stream.read(&mut request) {
                Ok(n) if n > 0 => n,
                _ => continue,
            };
            let req_str = String::from_utf8_lossy(&request[..bytes_read]);
            let path = req_str
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("");

            if path == "/playlist.m3u8" {
                let mut lines = vec!["#EXTM3U".to_string(), "#EXT-X-VERSION:4".to_string()];
                for i in 0..num_chunks {
                    let len = chunk_size.min(total_len - i * chunk_size);
                    let offset = i * chunk_size;
                    lines.push(format!("#EXTINF:5.0,\n#EXT-X-BYTERANGE:{len}@{offset}\n/video.ts"));
                }
                lines.push("#EXT-X-ENDLIST".to_string());
                let body = lines.join("\n") + "\n";
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.apple.mpegurl\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            } else if path == "/video.ts" {
                // Extract Range header: Range: bytes=start-end
                let range_line = req_str
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("range:"));

                if let Some(r_line) = range_line {
                    let range_val = r_line.split_once(':').map(|(_, r)| r.trim()).unwrap_or("");
                    if let Some(spec) = range_val.strip_prefix("bytes=") {
                        if let Some((start_s, end_s)) = spec.split_once('-') {
                            let start = start_s.parse::<usize>().unwrap_or(0);
                            let end = end_s
                                .parse::<usize>()
                                .unwrap_or(total_len.saturating_sub(1));
                            let end = end.min(total_len.saturating_sub(1));
                            if start <= end && start < total_len {
                                let slice = &file_bytes[start..=end];
                                let _ = write!(
                                    stream,
                                    "HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp2t\r\nContent-Range: bytes {start}-{end}/{total_len}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    slice.len()
                                );
                                let _ = stream.write_all(slice);
                                continue;
                            }
                        }
                    }
                }

                // Fallback: send full file
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nContent-Length: {total_len}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(&file_bytes);
            }
        }
    });

    format!("http://{address}/playlist.m3u8")
}

#[tokio::test]
async fn hls_byte_range_segments_download_and_assemble_correctly() {
    let directory = temporary_directory();
    fs::create_dir_all(&directory).expect("temporary directory should be created");

    let full_bytes: Vec<u8> = (0..128 * 1024).map(|i| (i % 251) as u8).collect();
    let url = serve_byte_range_hls(full_bytes.clone());

    let mut settings = Settings::new(directory.clone());
    settings.connections = 2;

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let http_executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let executor = CompositeDownloadExecutor::new(http_executor, reqwest::Client::new());
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("assembled_video.ts".to_string()),
        directory: None,
        category: None,
        description: Some("HLS byte range test".to_owned()),
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    };

    let ids = service
        .add(request, &settings)
        .expect("HLS byte range download should be accepted");

    for _ in 0..200 {
        let downloads = history.downloads().unwrap();
        let record = downloads.iter().find(|r| r.id == ids[0]).unwrap();
        if record.status == DownloadStatus::Completed {
            break;
        }
        if record.status == DownloadStatus::Failed {
            panic!("HLS byte range download failed: {:?}", record.error);
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let downloads = history.downloads().unwrap();
    let record = downloads.iter().find(|r| r.id == ids[0]).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);

    let output_path = directory.join("assembled_video.ts");
    assert!(output_path.exists());
    let content = fs::read(output_path).unwrap();
    assert_eq!(
        content, full_bytes,
        "assembled byte-range HLS chunks must exactly match original file bytes"
    );

    fs::remove_dir_all(directory).expect("clean directory");
}
