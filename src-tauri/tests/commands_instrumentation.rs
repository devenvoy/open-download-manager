use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use awesome_downloader_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        settings_service::SettingsService,
        shell_service::ShellService,
    },
    domain::download::{DownloadAction, DownloadRequest, DownloadStatus},
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
    presentation::{
        app_state::AppState,
        commands::{add_download, download_action, remove_completed, snapshot},
    },
};
use tauri::Manager;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn temporary_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "awesome-downloader-cmd-test-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temporary test directory should be created");
    dir
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

fn create_test_app(data_dir: &Path, shell: RecordingShellService) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let settings = SettingsService::new(
        SettingsStore::new(data_dir.to_path_buf()),
        data_dir.to_path_buf(),
    )
    .expect("settings service should initialize");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(data_dir))
        .expect("history service should initialize");

    let transfer = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().expect("http executor should initialize"),
        2,
        shell,
    );

    let app_state = AppState::new(settings, history, transfer);
    app.manage(app_state);
    app
}

#[tokio::test]
async fn commands_add_and_snapshot_lifecycle() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell);
    let state = app.state::<AppState>();

    let req = DownloadRequest {
        urls: vec!["http://127.0.0.1/test.zip".to_owned()],
        filename: Some("test.zip".to_owned()),
        directory: None,
        category: None,
        description: Some("cmd test".to_owned()),
        queue_only: true,
        scheduled_at: None,
    };

    let ids = add_download(state.clone(), req).await.expect("add_download command must succeed");
    assert_eq!(ids.len(), 1);

    let snap = snapshot(state.clone()).expect("snapshot command must succeed");
    assert_eq!(snap.downloads.len(), 1);
    assert_eq!(snap.downloads[0].id, ids[0]);
    assert_eq!(snap.downloads[0].filename, "test.zip");
}

#[tokio::test]
async fn commands_remove_action_removes_from_snapshot() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell);
    let state = app.state::<AppState>();

    let req = DownloadRequest {
        urls: vec!["http://127.0.0.1/item.bin".to_owned()],
        filename: Some("item.bin".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        scheduled_at: None,
    };

    let ids = add_download(state.clone(), req).await.unwrap();
    assert_eq!(snapshot(state.clone()).unwrap().downloads.len(), 1);

    // Human calls download_action with "remove"
    download_action(state.clone(), ids[0].clone(), DownloadAction::Remove)
        .expect("remove command must succeed");

    // Snapshot is now empty
    let snap = snapshot(state.clone()).unwrap();
    assert!(snap.downloads.is_empty());

    // Repeating remove fails with proper domain error
    let err = download_action(state.clone(), ids[0].clone(), DownloadAction::Remove)
        .expect_err("second remove must fail");
    assert_eq!(err.message, "The download no longer exists.");
}

#[tokio::test]
async fn commands_open_and_open_with_through_tauri_state() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell.clone());
    let state = app.state::<AppState>();

    let file_path = dir.join("finished.doc");
    fs::write(&file_path, b"document content").unwrap();

    let req = DownloadRequest {
        urls: vec!["http://127.0.0.1/doc".to_owned()],
        filename: Some("finished.doc".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        scheduled_at: None,
    };

    let ids = add_download(state.clone(), req).await.unwrap();

    // 1. Calling Open on incomplete download fails
    let err = download_action(state.clone(), ids[0].clone(), DownloadAction::Open).unwrap_err();
    assert_eq!(err.message, "This download has not finished.");

    // 2. Calling OpenWith on incomplete download fails
    let err = download_action(state.clone(), ids[0].clone(), DownloadAction::OpenWith).unwrap_err();
    assert_eq!(err.message, "This download has not finished.");

    // Mark complete with existing file on disk in the store
    let store = DownloadHistoryStore::new(&dir);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    let mut records = store.load().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].path = Some(file_path.to_string_lossy().into_owned());
    store.save(&records).unwrap();

    // Create app with updated store
    let app = create_test_app(&dir, shell.clone());
    let state = app.state::<AppState>();

    // 3. Open succeeds and triggers shell.open_path
    download_action(state.clone(), ids[0].clone(), DownloadAction::Open)
        .expect("open command should succeed");
    assert_eq!(shell.open_paths.lock().unwrap().as_slice(), &[file_path.clone()]);

    // 4. OpenWith succeeds and triggers shell.open_with
    download_action(state.clone(), ids[0].clone(), DownloadAction::OpenWith)
        .expect("openWith command should succeed");
    assert_eq!(shell.open_with_paths.lock().unwrap().as_slice(), &[file_path]);
}

#[tokio::test]
async fn commands_reveal_through_tauri_state() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell.clone());
    let state = app.state::<AppState>();

    let req = DownloadRequest {
        urls: vec!["http://127.0.0.1/video.mp4".to_owned()],
        filename: Some("video.mp4".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        scheduled_at: None,
    };

    let ids = add_download(state.clone(), req).await.unwrap();

    // For pending download, Reveal falls back to directory
    download_action(state.clone(), ids[0].clone(), DownloadAction::Reveal)
        .expect("reveal should fall back to directory");
    assert_eq!(shell.open_paths.lock().unwrap().as_slice(), &[dir.clone()]);
    assert!(shell.reveal_paths.lock().unwrap().is_empty());

    // Once finished with path, Reveal targets the file
    let file_path = dir.join("video.mp4");
    fs::write(&file_path, b"video bytes").unwrap();

    let store = DownloadHistoryStore::new(&dir);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    let mut records = store.load().unwrap();
    records[0].status = DownloadStatus::Completed;
    records[0].path = Some(file_path.to_string_lossy().into_owned());
    store.save(&records).unwrap();

    let app = create_test_app(&dir, shell.clone());
    let state = app.state::<AppState>();

    download_action(state.clone(), ids[0].clone(), DownloadAction::Reveal)
        .expect("reveal should target file");
    assert_eq!(shell.reveal_paths.lock().unwrap().as_slice(), &[file_path]);
}

#[tokio::test]
async fn commands_remove_completed_through_tauri_state() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell);
    let state = app.state::<AppState>();

    let req1 = DownloadRequest {
        urls: vec!["http://127.0.0.1/1".to_owned()],
        filename: Some("1.bin".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        scheduled_at: None,
    };
    let req2 = DownloadRequest {
        urls: vec!["http://127.0.0.1/2".to_owned()],
        filename: Some("2.bin".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        scheduled_at: None,
    };

    let id1 = add_download(state.clone(), req1).await.unwrap()[0].clone();
    let id2 = add_download(state.clone(), req2).await.unwrap()[0].clone();

    // Mark id1 complete
    let store = DownloadHistoryStore::new(&dir);
    use awesome_downloader_lib::application::download_history_repository::DownloadHistoryRepository;
    let mut records = store.load().unwrap();
    records.iter_mut().find(|r| r.id == id1).unwrap().status = DownloadStatus::Completed;
    store.save(&records).unwrap();

    let app = create_test_app(&dir, RecordingShellService::default());
    let state = app.state::<AppState>();

    // Human clicks Remove Completed
    let pruned = remove_completed(state.clone()).expect("remove_completed command should succeed");
    assert_eq!(pruned, vec![id1.clone()]);

    let snap = snapshot(state.clone()).unwrap();
    assert_eq!(snap.downloads.len(), 1);
    assert_eq!(snap.downloads[0].id, id2);
}
