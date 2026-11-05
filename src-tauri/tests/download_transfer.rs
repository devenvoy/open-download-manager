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
    application::download_history_service::DownloadHistoryService,
    application::download_transfer_service::DownloadTransferService,
    domain::{
        download::{DownloadAction, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::download_history_store::DownloadHistoryStore,
    infrastructure::http_download_executor::HttpDownloadExecutor,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "awesome-downloader-transfer-test-{}-{id}",
        std::process::id()
    ))
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

fn download_request(url: String, filename: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_owned()),
        directory: None,
        category: None,
        description: Some("fixture download".to_owned()),
        queue_only: false,
        scheduled_at: None,
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
            DownloadStatus::Completed | DownloadStatus::Failed
        ) {
            return record.status;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("download did not reach a terminal status");
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
    let settings = Settings::new(directory.clone());
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
    let settings = Settings::new(directory.clone());
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
    let settings = Settings::new(directory.clone());
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
