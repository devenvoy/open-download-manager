use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::Arc,
    thread,
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_history_repository::DownloadHistoryRepository,
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        download::{
            ConflictResolutionOutcome, DestinationAllocator, DownloadAction, DownloadRecord,
            DownloadRequest, DownloadStatus, FileConflictPolicy, ReviewDownloadRequest,
        },
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("odm-conflict-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn test_settings(directory: PathBuf, connections: u8) -> Settings {
    let mut settings = Settings::new(directory);
    settings.connections = connections;
    settings
}

fn download_req(
    url: String,
    filename: &str,
    conflict_policy: Option<FileConflictPolicy>,
) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_owned()),
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy,
        credentials: None,
        completion_actions: None,
    }
}

async fn wait_for_terminal_status(
    history: &DownloadHistoryService,
    id: &str,
) -> DownloadStatus {
    for _ in 0..100 {
        if let Ok(record) = history.download(id) {
            if matches!(
                record.status,
                DownloadStatus::Completed
                    | DownloadStatus::Failed
                    | DownloadStatus::Cancelled
                    | DownloadStatus::Skipped
            ) || (record.status == DownloadStatus::Paused && record.needs_review)
            {
                return record.status;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    history.download(id).unwrap().status
}

fn serve_simple_file(bytes: Vec<u8>) -> (String, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("local address");
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set non-blocking");
        while shutdown_rx.try_recv().is_err() {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 1024];
                    let _ = stream.read(&mut buffer);
                    let body = &bytes;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.write_all(body);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    (format!("http://{address}/test.bin"), shutdown_tx)
}

fn serve_segmented_file(body: Vec<u8>) -> (String, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("local address");
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("set non-blocking");
        while shutdown_rx.try_recv().is_err() {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 1024];
                    let n = stream.read(&mut buffer).unwrap_or(0);
                    let req_str = String::from_utf8_lossy(&buffer[..n]);

                    if let Some(range_line) = req_str
                        .lines()
                        .find(|l| l.to_lowercase().starts_with("range:"))
                    {
                        let range_val = range_line.split(':').nth(1).unwrap().trim();
                        if let Some(bytes_part) = range_val.strip_prefix("bytes=") {
                            let parts: Vec<&str> = bytes_part.split('-').collect();
                            let start: usize = parts[0].parse().unwrap_or(0);
                            let end: usize = if parts.len() > 1 && !parts[1].is_empty() {
                                parts[1].parse().unwrap_or(body.len() - 1)
                            } else {
                                body.len() - 1
                            };
                            let end = end.min(body.len() - 1);
                            let slice = &body[start..=end];
                            let resp = format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nETag: \"seg-v1\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                                slice.len(),
                                body.len()
                            );
                            let _ = stream.write_all(resp.as_bytes());
                            let _ = stream.write_all(slice);
                            let _ = stream.flush();
                            continue;
                        }
                    }

                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nETag: \"seg-v1\"\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.write_all(&body);
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    (format!("http://{address}/segmented_test.bin"), shutdown_tx)
}

#[tokio::test]
async fn existing_file_without_policy_reports_conflict_and_preserves_content() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("conflict_target.bin");
    fs::write(&file_path, b"ORIGINAL_BYTES").unwrap();

    let (url, _shutdown) = serve_simple_file(b"NEW_BYTES".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(download_req(url, "conflict_target.bin", None), &settings)
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Paused);

    let record = history.download(&ids[0]).unwrap();
    assert!(record.needs_review, "Download must need review when conflict occurs without policy");
    assert_eq!(
        record.error.as_deref(),
        Some("A file with that name already exists.")
    );

    // Verify original file on disk is completely untouched
    assert_eq!(fs::read(&file_path).unwrap(), b"ORIGINAL_BYTES");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn existing_file_with_overwrite_policy_atomically_replaces_content() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("conflict_target.bin");
    fs::write(&file_path, b"ORIGINAL_BYTES").unwrap();

    let (url, _shutdown) = serve_simple_file(b"REPLACEMENT_CONTENT".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(url, "conflict_target.bin", Some(FileConflictPolicy::Overwrite)),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    // Destination now atomically holds replacement content
    assert_eq!(fs::read(&file_path).unwrap(), b"REPLACEMENT_CONTENT");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn segmented_download_with_overwrite_policy_atomically_replaces_content() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("segmented_target.bin");
    fs::write(&file_path, b"ORIGINAL_BEFORE_SEGMENTED").unwrap();

    let new_body = (0..128 * 1024).map(|i| (i % 256) as u8).collect::<Vec<u8>>();
    let (url, _shutdown) = serve_segmented_file(new_body.clone());
    let settings = test_settings(dir.clone(), 2);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(
                url,
                "segmented_target.bin",
                Some(FileConflictPolicy::Overwrite),
            ),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    assert_eq!(fs::read(&file_path).unwrap(), new_body);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn overwrite_cancelled_mid_download_preserves_original_contents() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("important_document.pdf");
    fs::write(&file_path, b"SACRED_ORIGINAL_DOCUMENT").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("local address");
    let (started_tx, started_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = started_tx.send(());
            let header = "HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&[1_u8; 500]);
            let _ = stream.flush();
            thread::sleep(Duration::from_millis(500));
        }
    });

    let url = format!("http://{address}/doc.pdf");
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(
                url,
                "important_document.pdf",
                Some(FileConflictPolicy::Overwrite),
            ),
            &settings,
        )
        .unwrap();

    let _ = started_rx.recv_timeout(Duration::from_secs(3));
    // Cancel the download mid-stream
    service
        .action(&ids[0], DownloadAction::Cancel)
        .expect("cancel should succeed");

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Cancelled);

    // Original file must be 100% byte-for-byte identical to before!
    assert_eq!(
        fs::read(&file_path).unwrap(),
        b"SACRED_ORIGINAL_DOCUMENT",
        "Original file must be preserved upon cancellation!"
    );

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn existing_file_with_rename_policy_creates_indexed_filename() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let orig_file = dir.join("report.pdf");
    fs::write(&orig_file, b"FIRST_REPORT").unwrap();

    let (url, _shutdown) = serve_simple_file(b"SECOND_REPORT".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(url, "report.pdf", Some(FileConflictPolicy::Rename)),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Completed);

    // Both files must exist!
    assert_eq!(fs::read(&orig_file).unwrap(), b"FIRST_REPORT");
    let renamed_file = dir.join("report (1).pdf");
    assert!(renamed_file.exists());
    assert_eq!(fs::read(&renamed_file).unwrap(), b"SECOND_REPORT");

    let record = history.download(&ids[0]).unwrap();
    assert_eq!(record.filename, "report (1).pdf");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn concurrent_downloads_with_rename_allocate_distinct_filenames() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("data.bin"), b"INITIAL_DISK_DATA").unwrap();

    let (url, _shutdown) = serve_simple_file(b"PARALLEL_DATA".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let allocator = DestinationAllocator::new();
    let client = open_download_manager_lib::infrastructure::http_download_executor::create_http_client(None).unwrap();
    let shared_client = Arc::new(std::sync::RwLock::new(client));
    let executor = HttpDownloadExecutor::with_shared_client_and_allocator(
        shared_client,
        allocator.clone(),
    );
    let service = DownloadTransferService::with_shell_and_allocator(
        history.clone(),
        executor,
        4,
        open_download_manager_lib::infrastructure::system_shell::SystemShellService::new(),
        allocator,
    );

    let mut download_ids = Vec::new();
    for _ in 0..3 {
        let ids = service
            .add(
                download_req(url.clone(), "data.bin", Some(FileConflictPolicy::Rename)),
                &settings,
            )
            .unwrap();
        download_ids.push(ids[0].clone());
    }

    for id in &download_ids {
        assert_eq!(
            wait_for_terminal_status(&history, id).await,
            DownloadStatus::Completed
        );
    }

    // Check all allocated files on disk
    assert_eq!(fs::read(dir.join("data.bin")).unwrap(), b"INITIAL_DISK_DATA");
    assert!(dir.join("data (1).bin").exists());
    assert!(dir.join("data (2).bin").exists());
    assert!(dir.join("data (3).bin").exists());

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn destination_as_existing_directory_is_rejected_cleanly() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let folder_path = dir.join("some_folder");
    fs::create_dir_all(&folder_path).unwrap();

    let (url, _shutdown) = serve_simple_file(b"BYTES".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    // Try overwrite on a directory
    let ids = service
        .add(
            download_req(
                url,
                "some_folder",
                Some(FileConflictPolicy::Overwrite),
            ),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Failed);

    let record = history.download(&ids[0]).unwrap();
    assert!(
        record
            .error
            .as_deref()
            .unwrap_or("")
            .contains("existing directory"),
        "Error must explain that destination is an existing directory: {:?}",
        record.error
    );

    // Directory must still exist and NOT be replaced by a file
    assert!(folder_path.is_dir());

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn skip_policy_marks_download_skipped_and_leaves_file_intact() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("skip_me.txt");
    fs::write(&file_path, b"UNTOUCHED_CONTENT").unwrap();

    let (url, _shutdown) = serve_simple_file(b"NEW_CONTENT".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(url, "skip_me.txt", Some(FileConflictPolicy::Skip)),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Skipped);

    let record = history.download(&ids[0]).unwrap();
    assert_eq!(record.downloaded, 0);
    assert!(record.notice.as_deref().unwrap_or("").contains("skipped"));

    assert_eq!(fs::read(&file_path).unwrap(), b"UNTOUCHED_CONTENT");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn cancel_policy_marks_download_cancelled_and_leaves_file_intact() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("cancel_me.txt");
    fs::write(&file_path, b"UNTOUCHED_CONTENT").unwrap();

    let (url, _shutdown) = serve_simple_file(b"NEW_CONTENT".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    let ids = service
        .add(
            download_req(url, "cancel_me.txt", Some(FileConflictPolicy::Cancel)),
            &settings,
        )
        .unwrap();

    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Cancelled);

    assert_eq!(fs::read(&file_path).unwrap(), b"UNTOUCHED_CONTENT");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn review_workflow_allows_user_to_choose_policy_interactively() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("document.pdf");
    fs::write(&file_path, b"INITIAL_DOC").unwrap();

    let (url, _shutdown) = serve_simple_file(b"REVIEWED_REPLACEMENT".to_vec());
    let settings = test_settings(dir.clone(), 1);
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::new(history.clone(), executor, 1);

    // Initial add without policy triggers pause with needs_review = true
    let ids = service
        .add(download_req(url, "document.pdf", None), &settings)
        .unwrap();
    let status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(status, DownloadStatus::Paused);
    assert!(history.download(&ids[0]).unwrap().needs_review);

    // Check conflict outcome before resolving
    let outcome = service
        .check_destination_conflict(&dir.to_string_lossy(), "document.pdf")
        .unwrap();
    match outcome {
        ConflictResolutionOutcome::Conflict {
            existing_path,
            suggested_path,
            candidates,
        } => {
            assert_eq!(existing_path, file_path);
            assert_eq!(suggested_path, dir.join("document (1).pdf"));
            assert_eq!(
                candidates,
                vec![
                    FileConflictPolicy::Overwrite,
                    FileConflictPolicy::Rename,
                    FileConflictPolicy::Skip,
                    FileConflictPolicy::Cancel,
                ]
            );
        }
        other => panic!("Expected Conflict outcome, got {other:?}"),
    }

    // Now resolve review with Overwrite policy
    let review_req = ReviewDownloadRequest {
        id: ids[0].clone(),
        filename: None,
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: None,
        scheduled_at: None,
        conflict_policy: Some(FileConflictPolicy::Overwrite),
        credentials: None,
    };
    service.review_download(review_req).unwrap();

    let final_status = wait_for_terminal_status(&history, &ids[0]).await;
    assert_eq!(final_status, DownloadStatus::Completed);
    assert_eq!(fs::read(&file_path).unwrap(), b"REVIEWED_REPLACEMENT");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn app_restart_during_conflict_retains_state() {
    let dir = temporary_directory();
    fs::create_dir_all(&dir).unwrap();

    let record = DownloadRecord {
        id: "conflict-dl-1".to_string(),
        url: "https://example.com/asset.zip".to_string(),
        filename: "asset.zip".to_string(),
        status: DownloadStatus::Paused,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T01:00:00Z".to_string(),
        needs_review: true,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: Some("A file with that name already exists.".to_string()),
        notice: Some("A file with that name already exists. Please choose an action: overwrite, keep both, skip, or cancel.".to_string()),
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
        validator: None,
        conflict_policy: Some(FileConflictPolicy::Ask),
        credentials_override: None,
        completion_actions: None,
        action_ledger: Vec::new(),
    };

    let store = DownloadHistoryStore::new(&dir);
    store.save(&[record]).unwrap();

    // Simulate app restart by creating a new history service instance pointing to the same store
    let store_reopened = DownloadHistoryStore::new(&dir);
    let history_reloaded = DownloadHistoryService::new(store_reopened).unwrap();

    let reloaded = history_reloaded.download("conflict-dl-1").unwrap();
    assert_eq!(reloaded.status, DownloadStatus::Paused);
    assert!(reloaded.needs_review);
    assert_eq!(reloaded.conflict_policy, Some(FileConflictPolicy::Ask));
    assert_eq!(
        reloaded.error.as_deref(),
        Some("A file with that name already exists.")
    );

    let _ = fs::remove_dir_all(dir);
}
