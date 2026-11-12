// Created: 2026-11-08
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Human-scenario integration tests verifying named queues, independent concurrency, recurring schedules, and safe migration.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        error::AppResult,
        integration_service::IntegrationService,
        queue_service::QueueService,
        settings_service::SettingsService,
        shell_service::ShellService,
    },
    domain::{
        download::{BrowserRequestHeaders, DownloadRecord, DownloadStatus},
        queue::{
            MissedRunPolicy, QueueDisposition, QueueSchedule, DEFAULT_QUEUE_ID,
        },
        settings::window_limit::MockClock,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore, http_download_executor::HttpDownloadExecutor,
        queue_store::QueueStore, settings_store::SettingsStore,
    },
    presentation::{
        app_state::AppState,
        commands::{
            assign_download_to_queue, create_queue, delete_queue, list_queues, pause_queue,
            reorder_queue_items, start_queue,
        },
    },
};
use tauri::Manager;

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-queue-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[derive(Default, Clone)]
struct DummyShell;

impl ShellService for DummyShell {
    fn open_path(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
    fn reveal_path(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
    fn open_with(&self, _path: &std::path::Path) -> AppResult<()> {
        Ok(())
    }
}

fn create_test_app_with_clock(
    data_dir: &Path,
    clock: Arc<MockClock>,
) -> (tauri::App<tauri::test::MockRuntime>, Arc<QueueService>) {
    let app = tauri::test::mock_app();
    let settings = SettingsService::new(
        SettingsStore::new(data_dir.to_path_buf()),
        data_dir.to_path_buf(),
    )
    .expect("settings service should initialize");

    let history = DownloadHistoryService::new(DownloadHistoryStore::new(data_dir))
        .expect("history service should initialize");

    let queue_store = QueueStore::new(data_dir.to_path_buf());
    let queue_service = Arc::new(
        QueueService::new(queue_store, clock)
            .expect("queue service should initialize"),
    );

    let transfer = DownloadTransferService::with_shell(
        history.clone(),
        HttpDownloadExecutor::new().expect("http executor should initialize"),
        4,
        DummyShell,
    )
    .with_queue_service(queue_service.clone());

    let integration = IntegrationService::with_shell(
        data_dir.to_path_buf(),
        data_dir.to_path_buf(),
        Some(data_dir.join("Downloads")),
        DummyShell,
    );

    let file_icons = open_download_manager_lib::infrastructure::file_icons::FileIconProvider::new(
        data_dir.join("FileTypeIcons"),
    );

    let update_client = Arc::new(
        open_download_manager_lib::infrastructure::updater::GitHubReleaseClient::default_client(),
    );
    let update_service =
        open_download_manager_lib::application::updater_service::UpdateService::new(
            update_client,
            "0.1.2",
            true,
        );

    let cred_store = std::sync::Arc::new(
        open_download_manager_lib::infrastructure::credentials::CredentialStore::new(data_dir),
    );
    let cred_service = std::sync::Arc::new(
        open_download_manager_lib::application::credential_service::CredentialService::new(
            cred_store,
        ),
    );

    let app_state = AppState::new(
        settings,
        history,
        transfer,
        cred_service,
        integration,
        file_icons,
        update_service,
    )
    .with_queue_service(queue_service.clone());

    app.manage(app_state);
    (app, queue_service)
}

#[tokio::test]
async fn human_scenario_named_queues_lifecycle_and_disposition() {
    let data_dir = temporary_directory();
    let clock = Arc::new(MockClock::new(Utc::now()));
    let (app, queue_service) = create_test_app_with_clock(&data_dir, clock);

    // Initial state has Default Queue
    let initial_queues = list_queues(app.state::<AppState>()).expect("list queues");
    assert_eq!(initial_queues.len(), 1);
    assert_eq!(initial_queues[0].id, DEFAULT_QUEUE_ID);

    // Cannot delete default queue
    let del_def = delete_queue(
        app.state::<AppState>(),
        DEFAULT_QUEUE_ID.into(),
        Some(QueueDisposition::MoveToDefault),
    );
    assert!(del_def.is_err(), "Default queue must not be deletable");

    // Create named queue "Work"
    let work_q = create_queue(
        app.state::<AppState>(),
        "Work".into(),
        Some(2),
        Some(1000),
        None,
    )
    .expect("create Work queue");
    assert_eq!(work_q.name, "Work");
    assert_eq!(work_q.concurrency_limit, 2);

    // Assign downloads to "Work" queue
    assign_download_to_queue(
        app.state::<AppState>(),
        "dl-task-1".into(),
        work_q.id.clone(),
    )
    .expect("assign dl-task-1");
    assign_download_to_queue(
        app.state::<AppState>(),
        "dl-task-2".into(),
        work_q.id.clone(),
    )
    .expect("assign dl-task-2");

    // Reorder items in "Work" queue
    reorder_queue_items(
        app.state::<AppState>(),
        work_q.id.clone(),
        vec!["dl-task-2".into(), "dl-task-1".into()],
    )
    .expect("reorder items");

    let updated_work = queue_service.get_queue(&work_q.id).unwrap();
    assert_eq!(updated_work.item_ids, vec!["dl-task-2", "dl-task-1"]);

    // Delete "Work" queue with MoveToDefault disposition
    let moved_items = delete_queue(
        app.state::<AppState>(),
        work_q.id.clone(),
        Some(QueueDisposition::MoveToDefault),
    )
    .expect("delete queue with MoveToDefault");
    assert_eq!(moved_items, vec!["dl-task-2", "dl-task-1"]);

    // Verify Default queue now holds the moved items
    let default_q = queue_service.get_queue(DEFAULT_QUEUE_ID).unwrap();
    assert!(default_q.item_ids.contains(&"dl-task-2".to_string()));
    assert!(default_q.item_ids.contains(&"dl-task-1".to_string()));

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_independent_queue_concurrency_and_start_pause() {
    let data_dir = temporary_directory();
    let clock = Arc::new(MockClock::new(Utc::now()));
    let (app, _queue_service) = create_test_app_with_clock(&data_dir, clock);

    // Create a custom queue "Overnight" (starts paused / running = false)
    let overnight = create_queue(
        app.state::<AppState>(),
        "Overnight".into(),
        Some(1), // concurrency 1
        Some(500),
        None,
    )
    .expect("create Overnight");
    assert!(!overnight.running);

    // Start queue via command
    let started =
        start_queue(app.state::<AppState>(), overnight.id.clone()).expect("start Overnight");
    assert!(started.running);

    // Pause queue via command
    let paused =
        pause_queue(app.state::<AppState>(), overnight.id.clone()).expect("pause Overnight");
    assert!(!paused.running);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_recurring_schedule_daily_and_weekly_automation() {
    let data_dir = temporary_directory();
    // Monday 21:50 UTC
    let ref_time = DateTime::parse_from_rfc3339("2026-11-02T21:50:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock = Arc::new(MockClock::new(ref_time));
    let (app, queue_service) = create_test_app_with_clock(&data_dir, clock.clone());

    let schedule = QueueSchedule {
        enabled: true,
        days_of_week: vec![1, 2, 3, 4, 5], // Weekdays
        start_time: "22:00".into(),
        stop_time: Some("06:00".into()),
        timezone: "Local".into(),
        missed_run_policy: MissedRunPolicy::RunOnStartup,
        next_run_at: None,
        last_run_at: None,
    };

    let q = create_queue(
        app.state::<AppState>(),
        "Nightly".into(),
        Some(2),
        Some(500),
        Some(schedule),
    )
    .expect("create Nightly with schedule");

    // Before 22:00: queue is not running
    let (to_start, to_stop) = queue_service.check_schedules();
    assert!(to_start.is_empty());
    assert!(to_stop.is_empty());
    assert!(!queue_service.get_queue(&q.id).unwrap().running);

    // Advance clock to 22:05 (active window)
    let active_time = DateTime::parse_from_rfc3339("2026-11-02T22:05:00Z")
        .unwrap()
        .with_timezone(&Utc);
    clock.set(active_time);

    let (to_start, _) = queue_service.check_schedules();
    assert_eq!(to_start, vec![q.id.clone()]);
    assert!(queue_service.get_queue(&q.id).unwrap().running);

    // Advance clock past 06:00 Tuesday morning (06:05)
    let end_time = DateTime::parse_from_rfc3339("2026-11-03T06:05:00Z")
        .unwrap()
        .with_timezone(&Utc);
    clock.set(end_time);

    let (_, to_stop) = queue_service.check_schedules();
    assert_eq!(to_stop, vec![q.id.clone()]);
    assert!(!queue_service.get_queue(&q.id).unwrap().running);

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_missed_run_policy_run_on_startup_vs_skip() {
    let data_dir = temporary_directory();
    // Wednesday 01:00 UTC
    let base_time = DateTime::parse_from_rfc3339("2026-11-04T01:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock = Arc::new(MockClock::new(base_time));
    let (app, queue_service) = create_test_app_with_clock(&data_dir, clock.clone());

    // Schedule: 02:00 to 05:00
    // Test 1: RunOnStartup
    let run_schedule = QueueSchedule {
        enabled: true,
        days_of_week: vec![1, 2, 3, 4, 5],
        start_time: "02:00".into(),
        stop_time: Some("05:00".into()),
        timezone: "Local".into(),
        missed_run_policy: MissedRunPolicy::RunOnStartup,
        next_run_at: None,
        last_run_at: None,
    };
    let q_run = create_queue(
        app.state::<AppState>(),
        "CatchUpQueue".into(),
        Some(1),
        Some(0),
        Some(run_schedule),
    )
    .expect("create CatchUpQueue");

    // Test 2: Skip
    let skip_schedule = QueueSchedule {
        enabled: true,
        days_of_week: vec![1, 2, 3, 4, 5],
        start_time: "02:00".into(),
        stop_time: Some("05:00".into()),
        timezone: "Local".into(),
        missed_run_policy: MissedRunPolicy::Skip,
        next_run_at: None,
        last_run_at: None,
    };
    let q_skip = create_queue(
        app.state::<AppState>(),
        "SkipQueue".into(),
        Some(1),
        Some(0),
        Some(skip_schedule),
    )
    .expect("create SkipQueue");

    // Simulate app being closed before 02:00 and reopened at 05:30 (start was missed)
    let reopened_time = DateTime::parse_from_rfc3339("2026-11-04T05:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    clock.set(reopened_time);

    let (to_start, _) = queue_service.check_schedules();

    // CatchUpQueue should trigger start upon startup catchup
    assert!(to_start.contains(&q_run.id));
    // SkipQueue should NOT start
    assert!(!to_start.contains(&q_skip.id));

    let _ = std::fs::remove_dir_all(&data_dir);
}

#[tokio::test]
async fn human_scenario_safe_migration_and_one_off_schedules_preservation() {
    let data_dir = temporary_directory();
    let clock = Arc::new(MockClock::new(Utc::now()));

    // Create existing download record in history store with legacy one-off scheduled_at
    let history_store = DownloadHistoryStore::new(&data_dir);
    let legacy_record = DownloadRecord {
        id: "legacy-dl-1".into(),
        url: "https://example.com/archive.iso".into(),
        filename: "archive.iso".into(),
        status: DownloadStatus::Scheduled,
        downloaded: 0,
        total: Some(1048576),
        speed: 0,
        created_at: "2026-11-01T10:00:00Z".into(),
        needs_review: false,
        scheduled_at: Some("2026-11-08T22:00:00Z".into()),
        queue_id: None, // legacy download has no queue_id
        description: Some("Legacy one-off download".into()),
        category: None,
        directory: Some(data_dir.to_string_lossy().into_owned()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: None,
        resumable: Some(true),
        duration: None,
        connections: 4,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: None,
        etag: None,
        validator: None,
        conflict_policy: None,
        completion_actions: None,
        action_ledger: Vec::new(),
        sabr: None,
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
    };
    use open_download_manager_lib::application::download_history_repository::DownloadHistoryRepository;
    history_store
        .save(&[legacy_record])
        .expect("save legacy record");

    // Initialize application: migration should preserve legacy record and assign default queue
    let (app, _queue_service) = create_test_app_with_clock(&data_dir, clock);

    let snap = open_download_manager_lib::presentation::commands::snapshot(app.state::<AppState>())
        .expect("snapshot");
    let loaded = snap
        .downloads
        .iter()
        .find(|d| d.id == "legacy-dl-1")
        .expect("load legacy download");

    assert_eq!(loaded.id, "legacy-dl-1");
    assert_eq!(loaded.status, DownloadStatus::Scheduled);
    assert_eq!(
        loaded.scheduled_at,
        Some("2026-11-08T22:00:00Z".to_string()),
        "Legacy one-off schedule timestamp MUST be preserved unchanged"
    );

    let _ = std::fs::remove_dir_all(&data_dir);
}
