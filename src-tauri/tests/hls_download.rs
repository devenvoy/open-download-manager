use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use awesome_downloader_lib::{
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

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-hls-test-{}-{id}",
        std::process::id()
    ))
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
