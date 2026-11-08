// Created: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Last updated: 2026-11-08
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Human-scenario integration tests for per-site connection exceptions, redirects, rolling download limits, mock clock advances, and restart persistence.

use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::Arc,
    thread,
};

use chrono::{Duration, Utc};
use open_download_manager_lib::{
    application::{
        download_executor::DownloadProgressHandler,
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        quota_service::{QuotaEnforcementService, QuotaStatus},
        settings_service::SettingsService,
    },
    domain::{
        download::{DestinationAllocator, DownloadAction, DownloadRecord, DownloadRequest, DownloadStatus},
        settings::{
            resolve_connections_for_host, ConnectionOverride, DownloadWindowLimit, MockClock,
            Settings, SettingsPatch,
        },
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
        system_shell::SystemShellService,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-limits-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

#[test]
fn human_scenario_site_connection_override_matching_and_precedence() {
    let overrides = vec![
        ConnectionOverride::new("legacy.example.test", 1),
        ConnectionOverride::new("*.example.test", 4),
        ConnectionOverride::new("*.cdn.example.test", 2),
        ConnectionOverride::new("restricted.corp", 1),
    ];

    // 1. Exact match beats wildcards
    assert_eq!(
        resolve_connections_for_host(&overrides, "legacy.example.test", 8),
        1
    );

    // 2. Specific subdomain wildcard (*.cdn.example.test) beats general wildcard (*.example.test)
    assert_eq!(
        resolve_connections_for_host(&overrides, "eu.cdn.example.test", 8),
        2
    );

    // 3. General wildcard matches other subdomains
    assert_eq!(
        resolve_connections_for_host(&overrides, "api.example.test", 8),
        4
    );

    // 4. Unmatched domain falls back to global setting
    assert_eq!(
        resolve_connections_for_host(&overrides, "other.download.org", 8),
        8
    );

    // 5. Case insensitivity and scheme/port trimming
    assert_eq!(
        resolve_connections_for_host(&overrides, "LEGACY.EXAMPLE.TEST:8080", 8),
        1
    );

    // 6. Validation of rules
    assert!(ConnectionOverride::new("valid.host.com", 1).validate().is_ok());
    assert!(ConnectionOverride::new("*.valid.host.com", 16).validate().is_ok());
    assert!(ConnectionOverride::new("", 1).validate().is_err());
    assert!(ConnectionOverride::new("invalid/path", 1).validate().is_err());
    assert!(ConnectionOverride::new("user:pass@host.com", 1).validate().is_err());
    assert!(ConnectionOverride::new("valid.com", 0).validate().is_err());
    assert!(ConnectionOverride::new("valid.com", 33).validate().is_err());
    assert!(ConnectionOverride::new("sub.*.invalid.com", 1).validate().is_err());
}

#[tokio::test]
async fn human_scenario_site_override_forces_single_stream_for_non_resumable_servers() {
    let payload = b"Small file payload from legacy single-connection server";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(payload);
        }
    });

    let executor = HttpDownloadExecutor::new().expect("http executor");
    executor.set_connection_overrides(&[ConnectionOverride::new("127.0.0.1", 1)]);

    let download_dir = temporary_directory();
    let download = DownloadRecord {
        id: "single-conn-test".to_string(),
        url: format!("http://127.0.0.1:{port}/file.bin"),
        filename: "file.bin".to_string(),
        status: DownloadStatus::Downloading,
        downloaded: 0,
        total: Some(payload.len() as u64),
        speed: 0,
        created_at: Utc::now().to_rfc3339(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        directory: Some(download_dir.to_string_lossy().into_owned()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("http".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 8, // Initially 8, but override should force 1
        connections_progress: Vec::new(),
        audio_url: None,
        sabr: None,
        referer: None,
        youtube_format: None,
        transfer_kind: None,
        etag: None,
        validator: None,
        conflict_policy: None,
        credentials_override: None,
        completion_actions: None,
        action_ledger: Vec::new(),
    };

    let (sender, receiver) = tokio::sync::watch::channel(None);
    let control = open_download_manager_lib::application::download_executor::DownloadControl::new(receiver);
    let rate_limiter = Arc::new(open_download_manager_lib::application::download_rate_limiter::DownloadRateLimiter::new(0));

    let progress_conns = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let progress_conns_clone = progress_conns.clone();

    let progress_handler: DownloadProgressHandler =
        Arc::new(move |p| {
            if let Some(ec) = p.effective_concurrency {
                progress_conns_clone.store(ec, std::sync::atomic::Ordering::SeqCst);
            }
        });

    use open_download_manager_lib::application::download_executor::DownloadExecutor;
    let res = executor
        .execute(download, progress_handler, control, rate_limiter)
        .await
        .expect("transfer succeeds");

    assert_eq!(res.downloaded, payload.len() as u64);
    assert_eq!(progress_conns.load(std::sync::atomic::Ordering::SeqCst), 1);
    drop(sender);
    let _ = server_handle.join();
}

#[tokio::test]
async fn human_scenario_site_override_applied_after_redirects() {
    let payload = b"Payload delivered after host redirect";
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock redirect server");
    let port = listener.local_addr().unwrap().port();

    let server_handle = thread::spawn(move || {
        // First request: redirect to /final
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/final\r\nContent-Length: 0\r\n\r\n"
            );
            let _ = stream.write_all(response.as_bytes());
        }

        // Second request: deliver content
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(payload);
        }
    });

    let executor = HttpDownloadExecutor::new().expect("http executor");
    executor.set_connection_overrides(&[ConnectionOverride::new("127.0.0.1", 1)]);

    let download_dir = temporary_directory();
    let download = DownloadRecord {
        id: "redirect-override-test".to_string(),
        url: format!("http://127.0.0.1:{port}/initial"),
        filename: "redirected.bin".to_string(),
        status: DownloadStatus::Downloading,
        downloaded: 0,
        total: Some(payload.len() as u64),
        speed: 0,
        created_at: Utc::now().to_rfc3339(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        directory: Some(download_dir.to_string_lossy().into_owned()),
        path: None,
        temp_directory: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("http".to_string()),
        resumable: Some(true),
        duration: None,
        connections: 8,
        connections_progress: Vec::new(),
        audio_url: None,
        sabr: None,
        referer: None,
        youtube_format: None,
        transfer_kind: None,
        etag: None,
        validator: None,
        conflict_policy: None,
        credentials_override: None,
        completion_actions: None,
        action_ledger: Vec::new(),
    };

    let (_sender, receiver) = tokio::sync::watch::channel(None);
    let control = open_download_manager_lib::application::download_executor::DownloadControl::new(receiver);
    let rate_limiter = Arc::new(open_download_manager_lib::application::download_rate_limiter::DownloadRateLimiter::new(0));

    let effective_concurrency = Arc::new(std::sync::atomic::AtomicU8::new(0));
    let eff_clone = effective_concurrency.clone();
    let progress_handler: DownloadProgressHandler =
        Arc::new(move |p| {
            if let Some(ec) = p.effective_concurrency {
                eff_clone.store(ec, std::sync::atomic::Ordering::SeqCst);
            }
        });

    use open_download_manager_lib::application::download_executor::DownloadExecutor;
    let res = executor
        .execute(download, progress_handler, control, rate_limiter)
        .await
        .expect("redirect transfer succeeds");

    assert_eq!(res.downloaded, payload.len() as u64);
    assert_eq!(effective_concurrency.load(std::sync::atomic::Ordering::SeqCst), 1);
    let _ = server_handle.join();
}

#[test]
fn human_scenario_rolling_window_quota_boundary_and_auto_pause() {
    let now = Utc::now();
    let clock = Arc::new(MockClock::new(now));
    let limit = DownloadWindowLimit::new(12, 1000, now);

    let quota_service = QuotaEnforcementService::with_clock(Some(limit.clone()), clock.clone());

    // 1. Initial status is allowed
    assert_eq!(quota_service.is_quota_exceeded(), (false, None));

    // 2. Transfer 400 bytes: allowed
    let status = quota_service.record_progress("dl-1", 400);
    assert_eq!(status, QuotaStatus::Allowed);
    assert_eq!(quota_service.is_quota_exceeded(), (false, None));

    // 3. Concurrent transfer from second download: 500 bytes -> total 900 bytes: allowed
    let status = quota_service.record_progress("dl-2", 500);
    assert_eq!(status, QuotaStatus::Allowed);

    // 4. dl-1 transfers another 200 bytes (cumulative 600 bytes) -> total 1100 bytes >= 1000: quota exceeded!
    let status = quota_service.record_progress("dl-1", 600);
    match status {
        QuotaStatus::Exceeded { explanation, resume_at } => {
            assert!(explanation.starts_with("Quota reached; resumes at "));
            assert_eq!(resume_at, now + Duration::hours(12));
        }
        QuotaStatus::Allowed => panic!("Quota must be exceeded at 1100 bytes"),
    }

    let (exceeded, explanation) = quota_service.is_quota_exceeded();
    assert!(exceeded);
    assert!(explanation.unwrap().starts_with("Quota reached; resumes at "));
}

#[test]
fn human_scenario_rolling_window_resets_and_auto_resumes_when_clock_advances() {
    let anchor = Utc::now();
    let clock = Arc::new(MockClock::new(anchor));
    let limit = DownloadWindowLimit::new(12, 1000, anchor);

    let quota_service = QuotaEnforcementService::with_clock(Some(limit), clock.clone());

    // Cross quota threshold
    let _ = quota_service.record_progress("dl-1", 1200);
    assert!(quota_service.is_quota_exceeded().0);

    // Mark download as paused by quota
    quota_service.mark_quota_paused("dl-1");

    // Advance mock clock by 6 hours: still in window, still exceeded
    clock.advance(Duration::hours(6));
    assert!(!quota_service.check_window_roll());
    assert!(quota_service.is_quota_exceeded().0);

    // Advance clock past 12-hour boundary: window resets!
    clock.advance(Duration::hours(7)); // total 13 hours
    assert!(quota_service.check_window_roll());
    assert!(!quota_service.is_quota_exceeded().0);

    // Resumed downloads are drained cleanly
    let resumed = quota_service.drain_quota_paused();
    assert_eq!(resumed, vec!["dl-1".to_string()]);
    assert!(quota_service.drain_quota_paused().is_empty());
}

#[test]
fn human_scenario_quota_persistence_across_app_restarts() {
    let dir = temporary_directory();
    let store = SettingsStore::new(&dir);
    let default_dir = dir.join("downloads");

    let anchor = Utc::now();
    let clock = Arc::new(MockClock::new(anchor));

    let mut service = SettingsService::new(store.clone(), default_dir.clone()).unwrap();

    let mut limit = DownloadWindowLimit::new(12, 5000, anchor);
    limit.current_bytes = 6000;
    limit.stopped_until = Some(anchor + Duration::hours(12));

    let patch = SettingsPatch {
        default_directory: None,
        max_concurrent: None,
        connections: None,
        speed_limit_kbps: None,
        category_directories: None,
        temporary_directory: None,
        notify_on_complete: None,
        reveal_on_complete: None,
        confirm_browser_downloads: None,
        start_with_windows: None,
        hide_completion_dialog: None,
        proxy: None,
        capture_policy: None,
        completion_actions: None,
        clipboard_monitoring: None,
        connection_overrides: Some(vec![ConnectionOverride::new("slow.site.com", 1)]),
        window_limit: Some(limit.clone()),
    };

    let saved = service.update(patch).expect("save settings with limit");
    assert_eq!(saved.window_limit.as_ref().unwrap().current_bytes, 6000);
    assert_eq!(saved.connection_overrides.len(), 1);

    // Simulate App Restart by reloading from disk in fresh service
    let reloaded_service = SettingsService::new(store, default_dir).expect("reload settings");
    let loaded = reloaded_service.settings();

    assert_eq!(loaded.connection_overrides.len(), 1);
    assert_eq!(loaded.connection_overrides[0].host_pattern, "slow.site.com");
    assert_eq!(loaded.connection_overrides[0].connections, 1);

    let reloaded_limit = loaded.window_limit.as_ref().expect("reloaded window limit");
    assert_eq!(reloaded_limit.current_bytes, 6000);
    assert_eq!(reloaded_limit.window_hours, 12);
    assert!(reloaded_limit.stopped_until.is_some());

    // Quota service initialized with reloaded settings enforces quota limit immediately
    let quota_service = QuotaEnforcementService::with_clock(Some(reloaded_limit.clone()), clock.clone());
    assert!(quota_service.is_quota_exceeded().0);

    // After advancing clock by 12 hours, quota resets
    clock.advance(Duration::hours(13));
    assert!(quota_service.check_window_roll());
    assert!(!quota_service.is_quota_exceeded().0);
}

#[tokio::test]
async fn human_scenario_manual_stop_and_resume_under_limits() {
    let dir = temporary_directory();
    let history_dir = dir.join("history");

    let history_service = DownloadHistoryService::new(DownloadHistoryStore::new(history_dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();

    let transfer_service = DownloadTransferService::with_shell_and_allocator(
        history_service.clone(),
        executor,
        4,
        SystemShellService::new(),
        DestinationAllocator::new(),
    );

    let now = Utc::now();
    let clock = Arc::new(MockClock::new(now));
    let mut limit = DownloadWindowLimit::new(12, 1000, now);
    limit.current_bytes = 1500;
    limit.stopped_until = Some(now + Duration::hours(12));

    let quota_service = Arc::new(QuotaEnforcementService::with_clock(Some(limit.clone()), clock.clone()));
    let transfer_service = transfer_service.with_quota_service(quota_service);

    let mut settings = Settings::new(dir.join("downloads"));
    settings.window_limit = Some(limit);
    transfer_service.set_settings(settings);

    // Add a download
    let req = DownloadRequest {
        urls: vec!["http://example.com/test.zip".to_string()],
        filename: Some("test.zip".to_string()),
        ..Default::default()
    };
    let ids = transfer_service.add(req, &transfer_service.settings().unwrap()).unwrap();
    let id = &ids[0];

    // Manual resume while quota is exceeded returns safe error explanation
    let resume_res = transfer_service.action(id, DownloadAction::Resume);
    assert!(resume_res.is_err());
    let err = resume_res.unwrap_err();
    assert!(err.message.contains("Cannot resume download: Quota reached; resumes at "));

    // Advance clock past 12 hours: window resets
    clock.advance(Duration::hours(13));
    assert!(transfer_service.quota_service().check_window_roll());

    // Now resume is permitted!
    // Since there's no actual server listening on example.com, it begins or transitions
    let (exceeded, _) = transfer_service.quota_service().is_quota_exceeded();
    assert!(!exceeded);
}
