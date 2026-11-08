use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        integration_service::IntegrationService,
        settings_service::SettingsService,
        shell_service::ShellService,
    },
    domain::download::{
        DownloadAction, DownloadRequest, DownloadStatus, ReviewDownloadRequest,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
    presentation::{
        app_state::AppState,
        commands::{
            add_download, browser_setup_action, download_action, get_integration_info,
            get_media_tools_status, register_browser, remove_completed, review_download, snapshot,
        },
    },
};
use tauri::Manager;

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-cmd-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
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

impl RecordingShellService {
    fn opened_paths(&self) -> Vec<PathBuf> {
        self.open_paths.lock().unwrap().clone()
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
        shell.clone(),
    );

    let integration = IntegrationService::with_shell(
        data_dir.to_path_buf(),
        data_dir.to_path_buf(),
        shell,
    );

    let file_icons = open_download_manager_lib::infrastructure::file_icons::FileIconProvider::new(
        data_dir.join("FileTypeIcons"),
    );

    let update_client = std::sync::Arc::new(
        open_download_manager_lib::infrastructure::updater::GitHubReleaseClient::default_client(),
    );
    let update_service = open_download_manager_lib::application::updater_service::UpdateService::new(
        update_client,
        "0.1.2",
        true,
    );

    let cred_store = std::sync::Arc::new(
        open_download_manager_lib::infrastructure::credentials::CredentialStore::new(data_dir),
    );
    let cred_service = std::sync::Arc::new(
        open_download_manager_lib::application::credential_service::CredentialService::new(cred_store),
    );

    let app_state = AppState::new(
        settings,
        history,
        transfer,
        cred_service,
        integration,
        file_icons,
        update_service,
    );
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
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
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
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
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
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
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
    use open_download_manager_lib::application::download_history_repository::DownloadHistoryRepository;
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
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
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
    use open_download_manager_lib::application::download_history_repository::DownloadHistoryRepository;
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
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    };
    let req2 = DownloadRequest {
        urls: vec!["http://127.0.0.1/2".to_owned()],
        filename: Some("2.bin".to_owned()),
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    };

    let id1 = add_download(state.clone(), req1).await.unwrap()[0].clone();
    let id2 = add_download(state.clone(), req2).await.unwrap()[0].clone();

    // Mark id1 complete
    let store = DownloadHistoryStore::new(&dir);
    use open_download_manager_lib::application::download_history_repository::DownloadHistoryRepository;
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

#[tokio::test]
async fn commands_review_download_through_tauri_state() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let app = create_test_app(&dir, shell);
    let state = app.state::<AppState>();

    // Add download with review_only = true
    let req = DownloadRequest {
        urls: vec!["http://127.0.0.1/needs-confirm".to_owned()],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: false,
        review_only: Some(true),
        scheduled_at: None,
        audio_url: None,
        kind: None,
        ..Default::default()
    };

    let ids = add_download(state.clone(), req).await.unwrap();
    let id = ids[0].clone();

    // Verify snapshot reflects needs_review
    let snap = snapshot(state.clone()).unwrap();
    let d = snap.downloads.into_iter().find(|item| item.id == id).unwrap();
    assert!(d.needs_review);

    // Call review_download command
    let rev_req = ReviewDownloadRequest {
        id: id.clone(),
        filename: Some("confirmed_by_command.bin".to_owned()),
        directory: Some(dir.to_string_lossy().into_owned()),
        category: None,
        description: Some("Reviewed via command".to_owned()),
        queue_only: Some(true),
        scheduled_at: None,
        conflict_policy: None,
        credentials: None,
    };

    let result = review_download(state.clone(), rev_req).unwrap();
    assert_eq!(result, vec![id.clone()]);

    let snap_after = snapshot(state.clone()).unwrap();
    let d_after = snap_after.downloads.into_iter().find(|item| item.id == id).unwrap();
    assert!(!d_after.needs_review);
    assert_eq!(d_after.filename, "confirmed_by_command.bin");
}

#[tokio::test]
async fn commands_integration_info_and_browser_setup_through_tauri_state() {
    let dir = temporary_directory();
    let shell = RecordingShellService::default();
    let ext_dir = dir.join("extension");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("manifest.json"), r#"{"name":"test"}"#).unwrap();

    let app = create_test_app(&dir, shell.clone());
    let state = app.state::<AppState>();

    // 1. get_integration_info command
    let info = get_integration_info(state.clone()).unwrap();
    assert_eq!(info.browsers.len(), 5);
    assert!(info.browsers.iter().any(|b| b.name == "Chrome"));
    assert!(info.browsers.iter().any(|b| b.name == "Firefox"));

    // 2. browser_setup_action command
    let copy_result = browser_setup_action(state.clone(), "copy".to_owned(), "Chrome".to_owned()).await.unwrap();
    assert_eq!(copy_result.browser.as_deref(), Some("Chrome"));
    assert!(copy_result.extension_path.is_some());

    // 3. folder action invokes shell
    let _ = browser_setup_action(state.clone(), "folder".to_owned(), "Chrome".to_owned()).await.unwrap();
    assert_eq!(shell.opened_paths().len(), 1);

    // 4. register_browser command
    let reg_result = register_browser(
        state.clone(),
        "Chrome".to_owned(),
        "abcdefghijklmnopabcdefghijklmnop".to_owned(),
    ).await;
    assert!(reg_result.is_ok());
}

#[tokio::test]
async fn commands_media_tools_status() {
    let statuses = get_media_tools_status().expect("get_media_tools_status succeeds");
    assert_eq!(statuses.len(), 2);
    assert_eq!(
        statuses[0].tool_type,
        open_download_manager_lib::domain::media_tools::MediaToolType::Ffmpeg
    );
    assert_eq!(
        statuses[1].tool_type,
        open_download_manager_lib::domain::media_tools::MediaToolType::YtDlp
    );
}
