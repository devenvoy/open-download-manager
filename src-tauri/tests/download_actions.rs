use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use awesome_downloader_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        shell_service::ShellService,
    },
    domain::{
        download::{DownloadAction, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
    },
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "awesome-downloader-actions-test-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temporary test directory should be created");
    dir
}

fn download_request(url: String, filename: &str, queue_only: bool) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url],
        filename: Some(filename.to_owned()),
        directory: None,
        category: None,
        description: Some("fixture download".to_owned()),
        queue_only,
        scheduled_at: None,
    }
}

fn serve_slow_stream(total_bytes: usize) -> (String, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("local fixture should expose address");
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let stop_rx = Arc::new(Mutex::new(stop_rx));
    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            let rx = stop_rx.clone();
            thread::spawn(move || {
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                let is_head = request.starts_with(b"HEAD");
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {total_bytes}\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(header.as_bytes());
                if is_head {
                    return;
                }
                let chunk = vec![42u8; 8192];
                let mut sent = 0;
                while sent < total_bytes {
                    if rx.lock().unwrap().try_recv().is_ok() {
                        break;
                    }
                    if stream.write_all(&chunk).is_err() {
                        break;
                    }
                    sent += chunk.len();
                    thread::sleep(Duration::from_millis(5));
                }
            });
        }
    });
    (format!("http://{address}/slow.bin"), stop_tx)
}

async fn wait_for_downloaded_bytes(
    history: &DownloadHistoryService,
    id: &str,
    min_bytes: u64,
) -> u64 {
    for _ in 0..300 {
        if let Ok(downloads) = history.downloads() {
            if let Some(record) = downloads.into_iter().find(|r| r.id == id) {
                if record.downloaded >= min_bytes {
                    return record.downloaded;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    panic!("download {id} did not reach {min_bytes} bytes in time");
}

#[derive(Default, Clone)]
struct RecordingShellService {
    open_paths: Arc<Mutex<Vec<PathBuf>>>,
    reveal_paths: Arc<Mutex<Vec<PathBuf>>>,
    open_with_paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl ShellService for RecordingShellService {
    fn open_path(&self, path: &Path) -> AppResult<()> {
        self.open_paths.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn reveal_path(&self, path: &Path) -> AppResult<()> {
        self.reveal_paths.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn open_with(&self, path: &Path) -> AppResult<()> {
        self.open_with_paths.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[test]
fn download_action_deserializes_all_variants_including_camel_case_open_with() {
    let actions = [
        ("\"pause\"", DownloadAction::Pause, "pause"),
        ("\"resume\"", DownloadAction::Resume, "resume"),
        ("\"retry\"", DownloadAction::Retry, "retry"),
        ("\"cancel\"", DownloadAction::Cancel, "cancel"),
        ("\"remove\"", DownloadAction::Remove, "remove"),
        ("\"open\"", DownloadAction::Open, "open"),
        ("\"reveal\"", DownloadAction::Reveal, "reveal"),
        ("\"openWith\"", DownloadAction::OpenWith, "openWith"),
    ];

    for (json, expected_action, expected_str) in actions {
        let deserialized: DownloadAction =
            serde_json::from_str(json).expect("action should deserialize");
        assert_eq!(deserialized, expected_action);
        assert_eq!(deserialized.as_str(), expected_str);
    }
}

#[test]
fn remove_action_removes_queued_download_from_history() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let shell = RecordingShellService::default();
    let service = DownloadTransferService::with_shell(history.clone(), executor, 1, shell);

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "queued.bin", true),
            &settings,
        )
        .expect("download should be added");
    assert_eq!(history.downloads().unwrap().len(), 1);

    service
        .action(&ids[0], DownloadAction::Remove)
        .expect("remove action should succeed");
    assert!(history.downloads().unwrap().is_empty());

    let err = service
        .action(&ids[0], DownloadAction::Remove)
        .expect_err("second remove should fail");
    assert_eq!(err.message, "The download no longer exists.");
}

#[test]
fn remove_action_discards_partial_files_for_paused_download() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let shell = RecordingShellService::default();
    let service = DownloadTransferService::with_shell(history.clone(), executor, 1, shell);

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "paused.bin", true),
            &settings,
        )
        .expect("download should be added");

    let partial_file = directory.join(format!(".awesome-downloader-{}.part", ids[0]));
    fs::write(&partial_file, b"partial bytes").expect("partial file should be written");
    assert!(partial_file.exists());

    service
        .action(&ids[0], DownloadAction::Remove)
        .expect("remove action should succeed");

    assert!(history.downloads().unwrap().is_empty());
    assert!(!partial_file.exists(), "partial file should be removed on discard");
}

#[test]
fn remove_action_preserves_completed_file_on_disk() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let shell = RecordingShellService::default();
    let service = DownloadTransferService::with_shell(history.clone(), executor, 1, shell);

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "completed.bin", true),
            &settings,
        )
        .expect("download should be added");

    let final_path = directory.join("completed.bin");
    fs::write(&final_path, b"finished content").expect("final file should exist");

    // Manually mark download completed with path in store
    let records = history.downloads().unwrap();
    let mut updated_records = records;
    updated_records[0].status = DownloadStatus::Completed;
    updated_records[0].path = Some(final_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&updated_records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        RecordingShellService::default(),
    );

    service
        .action(&ids[0], DownloadAction::Remove)
        .expect("remove action should succeed");

    assert!(history.downloads().unwrap().is_empty());
    assert!(final_path.exists(), "completed file must remain on disk");
}

#[tokio::test]
async fn active_midstream_remove_aborts_transfer_and_cleans_partial_files() {
    let (url, _stop) = serve_slow_stream(500 * 1024);
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        executor,
        2,
        RecordingShellService::default(),
    );

    let ids = service
        .add(download_request(url, "slow_file.bin", false), &settings)
        .expect("download should start");

    // Wait until transfer is actively in progress
    wait_for_downloaded_bytes(&history, &ids[0], 8192).await;

    let partial_file = directory.join(format!(".awesome-downloader-{}.part", ids[0]));
    assert!(partial_file.exists(), "partial file must exist while downloading");

    // Human clicks Remove on active transfer
    service
        .action(&ids[0], DownloadAction::Remove)
        .expect("remove must succeed on active transfer");

    // Wait for cancellation cleanup to complete
    for _ in 0..100 {
        if !partial_file.exists() && history.downloads().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }

    assert!(
        !partial_file.exists(),
        "partial file must be cleaned up on active remove"
    );
    assert!(history.downloads().unwrap().is_empty(), "history must be cleared");

    // Subsequent action fails cleanly
    let err = service.action(&ids[0], DownloadAction::Pause).unwrap_err();
    assert_eq!(err.message, "The download no longer exists.");
}

#[tokio::test]
async fn active_download_remove_frees_concurrency_permit_for_queued_download() {
    let (url1, _stop1) = serve_slow_stream(500 * 1024);
    let (url2, _stop2) = serve_slow_stream(300 * 1024);
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    // max_concurrent = 1
    let service = DownloadTransferService::with_shell(
        history.clone(),
        executor,
        1,
        RecordingShellService::default(),
    );

    let id1 = service
        .add(download_request(url1, "first.bin", false), &settings)
        .unwrap()[0]
        .clone();
    let id2 = service
        .add(download_request(url2, "second.bin", false), &settings)
        .unwrap()[0]
        .clone();

    // id1 starts actively downloading, id2 waits for concurrency permit
    wait_for_downloaded_bytes(&history, &id1, 8192).await;
    let downloads = history.downloads().unwrap();
    assert_eq!(
        downloads.iter().find(|d| d.id == id2).unwrap().downloaded,
        0
    );

    // Human removes id1 while in flight
    service.action(&id1, DownloadAction::Remove).expect("remove must succeed");

    // Once id1 is removed, concurrency permit is released, resuming/starting id2
    service.action(&id2, DownloadAction::Resume).ok();
    wait_for_downloaded_bytes(&history, &id2, 8192).await;
    assert_eq!(history.downloads().unwrap().len(), 1);
}

#[test]
fn concurrent_action_spamming_does_not_deadlock_or_corrupt_state() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&directory)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        executor,
        1,
        RecordingShellService::default(),
    );

    let id = service
        .add(
            download_request("http://127.0.0.1/spam.bin".to_owned(), "spam.bin", true),
            &settings,
        )
        .unwrap()[0]
        .clone();

    let mut handles = Vec::new();
    for i in 0..16 {
        let svc = service.clone();
        let target_id = id.clone();
        handles.push(thread::spawn(move || {
            let action = match i % 4 {
                0 => DownloadAction::Remove,
                1 => DownloadAction::Pause,
                2 => DownloadAction::Resume,
                _ => DownloadAction::Remove,
            };
            let _ = svc.action(&target_id, action);
        }));
    }

    for handle in handles {
        handle.join().expect("thread must not panic");
    }

    // Must be in a consistent state: either completely removed or in a valid status
    let downloads = history.downloads().unwrap();
    assert!(downloads.len() <= 1);
}

#[test]
fn file_tampering_by_user_on_disk_is_handled_gracefully() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let shell = RecordingShellService::default();
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    let ids = service
        .add(
            download_request("http://127.0.0.1/tamper.bin".to_owned(), "tamper.bin", true),
            &settings,
        )
        .unwrap();

    let subfolder = directory.join("custom_folder");
    fs::create_dir_all(&subfolder).unwrap();
    let file_path = subfolder.join("tamper.bin");
    fs::write(&file_path, b"test content").unwrap();

    // Mark complete with existing file in subfolder
    let mut records = history.downloads().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].directory = Some(subfolder.to_string_lossy().into_owned());
    records[0].path = Some(file_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history,
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    // 1. Initial Reveal works and calls reveal_path with file
    service.action(&ids[0], DownloadAction::Reveal).unwrap();
    assert_eq!(
        shell.reveal_paths.lock().unwrap().as_slice(),
        &[file_path.clone()]
    );

    // 2. Human deletes the file in explorer
    fs::remove_file(&file_path).unwrap();

    // Open fails cleanly with expected message
    let err = service.action(&ids[0], DownloadAction::Open).unwrap_err();
    assert_eq!(err.message, "The downloaded file no longer exists.");

    // OpenWith fails cleanly with expected message
    let err = service.action(&ids[0], DownloadAction::OpenWith).unwrap_err();
    assert_eq!(err.message, "The downloaded file no longer exists.");

    // Reveal falls back to opening the folder
    service.action(&ids[0], DownloadAction::Reveal).unwrap();
    assert_eq!(
        shell.open_paths.lock().unwrap().as_slice(),
        &[subfolder.clone()]
    );

    // 3. Human deletes the folder as well
    fs::remove_dir_all(&subfolder).unwrap();
    let err = service.action(&ids[0], DownloadAction::Reveal).unwrap_err();
    assert_eq!(err.message, "The file or folder cannot be found.");
}

#[test]
fn paths_with_spaces_unicode_and_brackets_handled_without_truncation() {
    let base_dir = temporary_directory();
    let unicode_dir = base_dir.join("Folder (Test) [2026] - résumé & docs");
    fs::create_dir_all(&unicode_dir).unwrap();

    let unicode_file = unicode_dir.join("vacation [HD] (final) & summary.pdf");
    fs::write(&unicode_file, b"unicode payload").unwrap();

    let settings = Settings::new(unicode_dir.clone());
    let shell = RecordingShellService::default();
    let store = DownloadHistoryStore::new(&base_dir);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    let ids = service
        .add(
            download_request(
                "http://127.0.0.1/doc.pdf".to_owned(),
                "vacation [HD] (final) & summary.pdf",
                true,
            ),
            &settings,
        )
        .unwrap();

    let mut records = history.downloads().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].directory = Some(unicode_dir.to_string_lossy().into_owned());
    records[0].path = Some(unicode_file.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&base_dir);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    // Test Open
    service.action(&ids[0], DownloadAction::Open).unwrap();
    assert_eq!(
        shell.open_paths.lock().unwrap().as_slice(),
        &[unicode_file.clone()]
    );

    // Test Reveal
    service.action(&ids[0], DownloadAction::Reveal).unwrap();
    assert_eq!(
        shell.reveal_paths.lock().unwrap().as_slice(),
        &[unicode_file.clone()]
    );

    // Test OpenWith
    service.action(&ids[0], DownloadAction::OpenWith).unwrap();
    assert_eq!(
        shell.open_with_paths.lock().unwrap().as_slice(),
        &[unicode_file.clone()]
    );

    // Test Remove preserves file
    service.action(&ids[0], DownloadAction::Remove).unwrap();
    assert!(unicode_file.exists());
    assert!(history.downloads().unwrap().is_empty());
}

#[test]
fn interleaved_batch_removal_and_atomic_disk_reload() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        RecordingShellService::default(),
    );

    let mut ids = Vec::new();
    for i in 1..=5 {
        let id = service
            .add(
                download_request(
                    format!("http://127.0.0.1/{i}.bin"),
                    &format!("file_{i}.bin"),
                    true,
                ),
                &settings,
            )
            .unwrap()[0]
            .clone();
        ids.push(id);
    }

    // d1: Completed, d2: Paused (with partial file), d3: Failed, d4: Completed, d5: Queued
    let partial_file = directory.join(format!(".awesome-downloader-{}.part", ids[1]));
    fs::write(&partial_file, b"partial").unwrap();

    let mut records = history.downloads().unwrap();
    for record in &mut records {
        if record.id == ids[0] || record.id == ids[3] {
            record.status = DownloadStatus::Completed;
        } else if record.id == ids[1] {
            record.status = DownloadStatus::Paused;
        } else if record.id == ids[2] {
            record.status = DownloadStatus::Failed;
        } else if record.id == ids[4] {
            record.status = DownloadStatus::Queued;
        }
    }
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        RecordingShellService::default(),
    );

    // 1. Remove completed
    let removed = service.remove_completed().unwrap();
    assert_eq!(removed.len(), 2);
    assert!(removed.contains(&ids[0]));
    assert!(removed.contains(&ids[3]));

    // 2. Remove paused
    service.action(&ids[1], DownloadAction::Remove).unwrap();
    assert!(!partial_file.exists(), "partial file must be discarded");

    // 3. Drop all memory state and re-open history store from disk
    drop(service);
    drop(history);

    let reloaded_store = DownloadHistoryStore::new(&directory);
    let reloaded_history = DownloadHistoryService::new(reloaded_store).unwrap();
    let reloaded_downloads = reloaded_history.downloads().unwrap();

    assert_eq!(reloaded_downloads.len(), 2);
    let remaining_ids: Vec<_> = reloaded_downloads.iter().map(|d| d.id.clone()).collect();
    assert!(remaining_ids.contains(&ids[2])); // Failed
    assert!(remaining_ids.contains(&ids[4])); // Queued
}

#[test]
fn open_action_validates_status_and_existence_and_calls_shell() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let executor = HttpDownloadExecutor::new().expect("HTTP executor should initialize");
    let shell = RecordingShellService::default();
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(history.clone(), executor, 1, shell.clone());

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "open-test.bin", true),
            &settings,
        )
        .expect("download should be added");

    // 1. Should fail when not complete
    let err = service
        .action(&ids[0], DownloadAction::Open)
        .expect_err("open on queued download must fail");
    assert_eq!(err.message, "This download has not finished.");

    // 2. Set to complete but file does not exist
    let non_existent_path = directory.join("missing-file.bin");
    let mut records = history.downloads().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].path = Some(non_existent_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    let err = service
        .action(&ids[0], DownloadAction::Open)
        .expect_err("open on missing file must fail");
    assert_eq!(err.message, "The downloaded file no longer exists.");

    // 3. File exists -> calls shell.open_path
    let real_path = directory.join("real-file.bin");
    fs::write(&real_path, b"content").unwrap();
    let mut records = history.downloads().unwrap();
    records[0].path = Some(real_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history,
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    service
        .action(&ids[0], DownloadAction::Open)
        .expect("open on completed file should succeed");

    assert_eq!(shell.open_paths.lock().unwrap().as_slice(), &[real_path]);
}

#[test]
fn open_with_action_validates_status_and_existence_and_calls_shell() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let shell = RecordingShellService::default();
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "open-with.bin", true),
            &settings,
        )
        .expect("download should be added");

    // 1. Not complete -> error
    let err = service
        .action(&ids[0], DownloadAction::OpenWith)
        .expect_err("openWith on queued download must fail");
    assert_eq!(err.message, "This download has not finished.");

    // 2. Completed with existing file -> calls shell.open_with
    let real_path = directory.join("open-with.bin");
    fs::write(&real_path, b"content").unwrap();
    let mut records = history.downloads().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].path = Some(real_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history,
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    service
        .action(&ids[0], DownloadAction::OpenWith)
        .expect("openWith should succeed");

    assert_eq!(
        shell.open_with_paths.lock().unwrap().as_slice(),
        &[real_path]
    );
}

#[test]
fn reveal_action_reveals_completed_file_and_falls_back_to_directory() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let shell = RecordingShellService::default();
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    let ids = service
        .add(
            download_request("http://127.0.0.1/file.bin".to_owned(), "reveal-test.bin", true),
            &settings,
        )
        .expect("download should be added");

    // 1. In-progress download without path -> falls back to directory
    service
        .action(&ids[0], DownloadAction::Reveal)
        .expect("reveal should fall back to directory");
    assert_eq!(
        shell.open_paths.lock().unwrap().as_slice(),
        &[directory.clone()]
    );
    assert!(shell.reveal_paths.lock().unwrap().is_empty());

    // 2. Completed download with existing file -> calls shell.reveal_path
    let real_path = directory.join("reveal-test.bin");
    fs::write(&real_path, b"content").unwrap();
    let mut records = history.downloads().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].path = Some(real_path.to_string_lossy().into_owned());
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history,
        HttpDownloadExecutor::new().unwrap(),
        1,
        shell.clone(),
    );

    service
        .action(&ids[0], DownloadAction::Reveal)
        .expect("reveal on existing file should succeed");

    assert_eq!(shell.reveal_paths.lock().unwrap().as_slice(), &[real_path]);
}

#[test]
fn remove_completed_prunes_only_completed_downloads() {
    let directory = temporary_directory();
    let mut settings = Settings::new(directory.clone());
    settings.connections = 1;
    let store = DownloadHistoryStore::new(&directory);
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        RecordingShellService::default(),
    );

    let id1 = service
        .add(
            download_request("http://127.0.0.1/1.bin".to_owned(), "1.bin", true),
            &settings,
        )
        .unwrap()[0]
        .clone();
    let id2 = service
        .add(
            download_request("http://127.0.0.1/2.bin".to_owned(), "2.bin", true),
            &settings,
        )
        .unwrap()[0]
        .clone();
    let id3 = service
        .add(
            download_request("http://127.0.0.1/3.bin".to_owned(), "3.bin", true),
            &settings,
        )
        .unwrap()[0]
        .clone();

    // Mark id1 as completed, id2 as paused, id3 as failed
    let mut records = history.downloads().unwrap();
    for record in &mut records {
        if record.id == id1 {
            record.status = DownloadStatus::Completed;
        } else if record.id == id2 {
            record.status = DownloadStatus::Paused;
        } else if record.id == id3 {
            record.status = DownloadStatus::Failed;
        }
    }
    let store = DownloadHistoryStore::new(&directory);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    store.save(&records).unwrap();
    let history = DownloadHistoryService::new(store).unwrap();
    let service = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        1,
        RecordingShellService::default(),
    );

    let removed = service.remove_completed().expect("remove_completed should succeed");
    assert_eq!(removed, vec![id1]);

    let remaining = history.downloads().unwrap();
    assert_eq!(remaining.len(), 2);
    let remaining_ids: Vec<_> = remaining.into_iter().map(|d| d.id).collect();
    assert!(remaining_ids.contains(&id2));
    assert!(remaining_ids.contains(&id3));

    // Calling again returns empty vector
    let removed_again = service.remove_completed().unwrap();
    assert!(removed_again.is_empty());
}
