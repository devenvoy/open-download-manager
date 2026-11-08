// Created: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(clipboard): add optional clipboard link monitoring (Closes #33)
// Reason: feat(limits): add per-site connection overrides and rolling download limits (Closes #36)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Integration tests verifying category and temp directories functionality, edge cases, and error recovery.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        settings_service::SettingsService,
    },
    domain::{
        category::DownloadCategory,
        download::{DownloadRecord, DownloadRequest, DownloadStatus},
        settings::{Settings, SettingsPatch},
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        fs_utils::{available_space, validate_directory_writable, verified_cross_volume_copy},
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
};

fn temp_dir_fixture() -> PathBuf {
    tempfile::Builder::new()
        .prefix("odm-category-test-")
        .tempdir()
        .expect("temporary directory")
        .keep()
}

#[tokio::test]
async fn extension_based_category_detection_routes_to_category_directories() {
    let base = temp_dir_fixture();
    let default_dir = base.join("default");
    let video_dir = base.join("videos");
    let archives_dir = base.join("archives");
    let docs_dir = base.join("docs");
    let music_dir = base.join("music");
    let programs_dir = base.join("programs");

    for d in [&default_dir, &video_dir, &archives_dir, &docs_dir, &music_dir, &programs_dir] {
        std::fs::create_dir_all(d).unwrap();
    }

    let mut category_dirs = BTreeMap::new();
    category_dirs.insert(DownloadCategory::Video, video_dir.clone());
    category_dirs.insert(DownloadCategory::Archives, archives_dir.clone());
    category_dirs.insert(DownloadCategory::Documents, docs_dir.clone());
    category_dirs.insert(DownloadCategory::Audio, music_dir.clone());
    category_dirs.insert(DownloadCategory::Programs, programs_dir.clone());

    let mut settings = Settings::new(default_dir.clone());
    settings.category_directories = category_dirs;

    let history = DownloadHistoryService::new(
        DownloadHistoryStore::new(&base.join("history")),
    )
    .unwrap();
    let transfer = DownloadTransferService::new(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        4,
    );

    let test_cases = [
        ("https://example.com/movie.mp4", DownloadCategory::Video, video_dir.clone()),
        ("https://example.com/archive.zip", DownloadCategory::Archives, archives_dir.clone()),
        ("https://example.com/manual.pdf", DownloadCategory::Documents, docs_dir.clone()),
        ("https://example.com/song.flac", DownloadCategory::Audio, music_dir.clone()),
        ("https://example.com/setup.exe", DownloadCategory::Programs, programs_dir.clone()),
        ("https://example.com/unknown.xyz", DownloadCategory::Other, default_dir.clone()),
    ];

    for (url_str, expected_cat, expected_dir) in test_cases {
        let req = DownloadRequest {
            urls: vec![url_str.to_string()],
            filename: None,
            directory: None,
            temp_directory: None,
            category: None,
            description: None,
            queue_only: true,
            review_only: None,
            scheduled_at: None,
            queue_id: None,
            audio_url: None,
            kind: None,
            sabr: None,
            reuse_active: None,
            referer: None,
            youtube_format: None,
            conflict_policy: None,
            credentials: None,
            completion_actions: None,
        };

        let ids = transfer.add(req, &settings).unwrap();
        let record = history.download(&ids[0]).unwrap();

        assert_eq!(record.category, Some(expected_cat));
        assert_eq!(
            record.directory,
            Some(expected_dir.to_string_lossy().into_owned())
        );
    }

    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn changed_defaults_do_not_relocate_existing_downloads() {
    let base = temp_dir_fixture();
    let dir_a = base.join("folder_a");
    let temp_a = base.join("temp_a");
    let dir_b = base.join("folder_b");
    let temp_b = base.join("temp_b");

    for d in [&dir_a, &temp_a, &dir_b, &temp_b] {
        std::fs::create_dir_all(d).unwrap();
    }

    let mut settings_a = Settings::new(dir_a.clone());
    settings_a.temporary_directory = Some(temp_a.clone());

    let history = DownloadHistoryService::new(
        DownloadHistoryStore::new(&base.join("history")),
    )
    .unwrap();
    let transfer = DownloadTransferService::new(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        4,
    );

    // 1. Add download with settings_a
    let req1 = DownloadRequest {
        urls: vec!["https://example.com/first_download.dat".to_string()],
        filename: Some("first.dat".to_string()),
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        queue_id: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };
    let ids1 = transfer.add(req1, &settings_a).unwrap();
    let record1 = history.download(&ids1[0]).unwrap();
    assert_eq!(record1.directory, Some(dir_a.to_string_lossy().into_owned()));
    assert_eq!(record1.temp_directory, Some(temp_a.to_string_lossy().into_owned()));

    // 2. Change global settings to settings_b
    let mut settings_b = Settings::new(dir_b.clone());
    settings_b.temporary_directory = Some(temp_b.clone());

    // 3. Add second download with settings_b
    let req2 = DownloadRequest {
        urls: vec!["https://example.com/second_download.dat".to_string()],
        filename: Some("second.dat".to_string()),
        directory: None,
        temp_directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        queue_id: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };
    let ids2 = transfer.add(req2, &settings_b).unwrap();
    let record2 = history.download(&ids2[0]).unwrap();
    assert_eq!(record2.directory, Some(dir_b.to_string_lossy().into_owned()));
    assert_eq!(record2.temp_directory, Some(temp_b.to_string_lossy().into_owned()));

    // 4. Verify existing record1 was NOT modified or relocated
    let record1_rechecked = history.download(&ids1[0]).unwrap();
    assert_eq!(
        record1_rechecked.directory,
        Some(dir_a.to_string_lossy().into_owned()),
        "Existing download directory must remain locked to initial path"
    );
    assert_eq!(
        record1_rechecked.temp_directory,
        Some(temp_a.to_string_lossy().into_owned()),
        "Existing temporary directory must remain locked to initial path"
    );

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn restart_and_resume_retains_recorded_final_and_temp_paths() {
    let base = temp_dir_fixture();
    let final_dir = base.join("final_saved");
    let temp_parts_dir = base.join("temporary_parts");

    let mut record = DownloadRecord {
        id: "restart-test-1".to_string(),
        url: "https://example.com/large_video.mkv".to_string(),
        filename: "large_video.mkv".to_string(),
        status: DownloadStatus::Downloading,
        downloaded: 1024 * 1024,
        total: Some(10 * 1024 * 1024),
        speed: 50000,
        created_at: "2026-11-08T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: Some("Video download".to_string()),
        category: Some(DownloadCategory::Video),
        directory: Some(final_dir.to_string_lossy().into_owned()),
        path: None,
        temp_directory: Some(temp_parts_dir.to_string_lossy().into_owned()),
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

    record.prepare_for_restart();

    assert_eq!(record.status, DownloadStatus::Paused);
    assert_eq!(record.speed, 0);
    assert_eq!(
        record.directory,
        Some(final_dir.to_string_lossy().into_owned())
    );
    assert_eq!(
        record.temp_directory,
        Some(temp_parts_dir.to_string_lossy().into_owned())
    );

    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn temporary_parts_directory_is_used_during_download_and_assembled_to_destination() {
    let base = temp_dir_fixture();
    let destination_dir = base.join("final_output");
    let temp_parts_dir = base.join("dedicated_temp");
    std::fs::create_dir_all(&destination_dir).unwrap();
    std::fs::create_dir_all(&temp_parts_dir).unwrap();

    let payload = b"TEMPORARY_PARTS_ASSEMBLY_TEST_CONTENT";

    // Setup mock server that supports HEAD and GET
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let file_data = payload.to_vec();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let data = file_data.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req_str = String::from_utf8_lossy(&buf[..n]);

                if req_str.starts_with("HEAD") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                } else if req_str.starts_with("GET") {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        data.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.write_all(&data).await;
                }
            });
        }
    });

    let mut settings = Settings::new(destination_dir.clone());
    settings.temporary_directory = Some(temp_parts_dir.clone());

    let history = DownloadHistoryService::new(
        DownloadHistoryStore::new(&base.join("history")),
    )
    .unwrap();
    let transfer = DownloadTransferService::new(
        history.clone(),
        HttpDownloadExecutor::new().unwrap(),
        4,
    );

    let req = DownloadRequest {
        urls: vec![format!("http://{addr}/test_payload.bin")],
        filename: Some("assembled.bin".to_string()),
        directory: Some(destination_dir.to_string_lossy().into_owned()),
        temp_directory: Some(temp_parts_dir.to_string_lossy().into_owned()),
        category: None,
        description: None,
        queue_only: false,
        review_only: None,
        scheduled_at: None,
        queue_id: None,
        audio_url: None,
        kind: None,
        sabr: None,
        reuse_active: None,
        referer: None,
        youtube_format: None,
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = transfer.add(req, &settings).unwrap();
    let id = &ids[0];

    // Wait for completion
    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let final_record = history.download(id).unwrap();
    assert_eq!(final_record.status, DownloadStatus::Completed);

    let final_file = destination_dir.join("assembled.bin");
    assert!(final_file.exists(), "Final assembled file must exist in destination directory");
    let content = std::fs::read(&final_file).unwrap();
    assert_eq!(content, payload);

    // Verify temp directory contains no lingering partial files
    let temp_entries: Vec<_> = std::fs::read_dir(&temp_parts_dir)
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert!(
        temp_entries.is_empty(),
        "All temporary part files must be cleaned up after successful assembly"
    );

    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn verified_cross_volume_copy_verifies_size_flushes_and_cleans_up_staging() {
    let base = temp_dir_fixture();
    let source_dir = base.join("source_vol");
    let dest_dir = base.join("dest_vol");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&dest_dir).unwrap();

    let source_file = source_dir.join("source.dat");
    let dest_file = dest_dir.join("dest.dat");
    let payload = b"VERIFIED_CROSS_VOLUME_PAYLOAD_TEST_DATA";
    std::fs::write(&source_file, payload).unwrap();

    assert!(available_space(&dest_dir).unwrap() > 0);

    // 1. Successful verified copy
    verified_cross_volume_copy(
        &source_file,
        &dest_file,
        payload.len() as u64,
        "xvol-1",
    )
    .await
    .expect("verified cross-volume copy should succeed");

    assert!(dest_file.exists(), "Destination file must exist after verified copy");
    assert_eq!(std::fs::read(&dest_file).unwrap(), payload);
    assert!(!source_file.exists(), "Source file must be removed after successful move");

    // 2. Size mismatch rejection and cleanup
    let source_file2 = source_dir.join("source2.dat");
    let dest_file2 = dest_dir.join("dest2.dat");
    std::fs::write(&source_file2, payload).unwrap();

    // Claim expected size is 99999 while file is actually smaller
    let err = verified_cross_volume_copy(
        &source_file2,
        &dest_file2,
        99999,
        "xvol-2",
    )
    .await
    .unwrap_err();

    assert!(err.message.contains("size mismatch") || err.message.contains("verification failed"));
    assert!(!dest_file2.exists(), "Destination file must not exist if verification failed");

    // Ensure no staging files remained in dest_dir
    let dest_entries: Vec<_> = std::fs::read_dir(&dest_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(".staging-"))
        .collect();
    assert!(dest_entries.is_empty(), "Staging files must be removed on copy error");

    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn verified_cross_volume_copy_surfaces_free_space_failure() {
    let base = temp_dir_fixture();
    let source_dir = base.join("source_vol");
    let dest_dir = base.join("dest_vol");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&dest_dir).unwrap();

    let source_file = source_dir.join("source.dat");
    let dest_file = dest_dir.join("dest.dat");
    let payload = b"FREE_SPACE_CHECK_PAYLOAD";
    std::fs::write(&source_file, payload).unwrap();

    // Request an astronomically large size that exceeds any physical volume (500 Petabytes)
    let impossible_size: u64 = 500 * 1024 * 1024 * 1024 * 1024 * 1024;
    let err = verified_cross_volume_copy(
        &source_file,
        &dest_file,
        impossible_size,
        "xvol-space-1",
    )
    .await
    .unwrap_err();

    assert!(
        err.message.contains("Insufficient disk space on destination volume"),
        "Must surface clear free space error message: {}",
        err.message
    );
    assert!(!dest_file.exists());

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn writable_directory_validation_rejects_invalid_or_non_writable_folders() {
    let base = temp_dir_fixture();

    // 1. Valid existing directory
    assert!(validate_directory_writable(&base).is_ok());

    // 2. Valid nested directory that does not exist yet (create_dir_all succeeds)
    let nested = base.join("sub1").join("sub2");
    assert!(validate_directory_writable(&nested).is_ok());
    assert!(nested.exists());

    // 3. Relative path rejected
    let relative = Path::new("some/relative/path");
    let err_rel = validate_directory_writable(relative).unwrap_err();
    assert!(err_rel.contains("Path must be an absolute path"));

    // 4. Read-only directory on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let readonly_dir = base.join("readonly_dir");
        std::fs::create_dir_all(&readonly_dir).unwrap();
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o444)).unwrap();

        let err_ro = validate_directory_writable(&readonly_dir).unwrap_err();
        assert!(err_ro.contains("is not writable"));

        // Restore permissions so cleanup succeeds
        let _ = std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o755));
    }

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn options_settings_patch_supports_editing_and_resetting_category_and_temp_directories() {
    let base = temp_dir_fixture();
    let default_dir = base.join("default_dl");
    let videos_dir = base.join("videos_custom");
    let temp_parts_dir = base.join("temp_parts_custom");

    for d in [&default_dir, &videos_dir, &temp_parts_dir] {
        std::fs::create_dir_all(d).unwrap();
    }

    let store = SettingsStore::new(&base.join("store"));
    let mut service = SettingsService::new(store, default_dir.clone()).unwrap();

    // 1. Set custom category directory and temporary directory
    let mut cat_dirs = BTreeMap::new();
    cat_dirs.insert(DownloadCategory::Video, videos_dir.to_string_lossy().into_owned());

    let updated = service
        .update(SettingsPatch {
            default_directory: None,
            max_concurrent: None,
            connections: None,
            speed_limit_kbps: None,
            category_directories: Some(cat_dirs),
            temporary_directory: Some(temp_parts_dir.to_string_lossy().into_owned()),
            notify_on_complete: None,
            reveal_on_complete: None,
            confirm_browser_downloads: None,
            start_with_windows: None,
            hide_completion_dialog: None,
            proxy: None,
            capture_policy: None,
            completion_actions: None,
            clipboard_monitoring: None,
            connection_overrides: None,
            window_limit: None,
        })
        .unwrap();

    assert_eq!(
        updated.category_directories.get(&DownloadCategory::Video),
        Some(&videos_dir)
    );
    assert_eq!(updated.temporary_directory, Some(temp_parts_dir.clone()));

    // 2. Reset category directory (send empty string) and temporary directory (send empty string)
    let mut reset_cats = BTreeMap::new();
    reset_cats.insert(DownloadCategory::Video, "   ".to_string());

    let reset_updated = service
        .update(SettingsPatch {
            default_directory: None,
            max_concurrent: None,
            connections: None,
            speed_limit_kbps: None,
            category_directories: Some(reset_cats),
            temporary_directory: Some("".to_string()),
            notify_on_complete: None,
            reveal_on_complete: None,
            confirm_browser_downloads: None,
            start_with_windows: None,
            hide_completion_dialog: None,
            proxy: None,
            capture_policy: None,
            completion_actions: None,
            clipboard_monitoring: None,
            connection_overrides: None,
            window_limit: None,
        })
        .unwrap();

    assert_eq!(
        reset_updated.category_directories.get(&DownloadCategory::Video),
        None,
        "Blank category directory string must reset/remove the category override"
    );
    assert_eq!(
        reset_updated.temporary_directory,
        None,
        "Blank temporary directory string must reset to None"
    );

    let _ = std::fs::remove_dir_all(base);
}
