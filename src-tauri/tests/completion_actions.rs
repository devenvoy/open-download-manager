use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        completion::{ActionStatus, CompletionAction, PowerActionKind},
        download::{DownloadAction, DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        action::{ActionExecutor, MockActionExecutor, SystemActionExecutor},
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        system_shell::SystemShellService,
    },
};

fn temporary_directory() -> PathBuf {
    tempfile::Builder::new()
        .prefix("open-download-manager-completion-actions-test-")
        .tempdir()
        .expect("temporary test directory")
        .keep()
}

fn serve_simple_file(content: Vec<u8>) -> (String, std::sync::mpsc::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("local fixture should expose address");
    let (stop_tx, stop_rx) = std::sync::mpsc::channel();
    let stop_rx = Arc::new(Mutex::new(stop_rx));
    let content = Arc::new(content);

    thread::spawn(move || {
        while let Ok((mut stream, _)) = listener.accept() {
            if stop_rx.lock().unwrap().try_recv().is_ok() {
                break;
            }
            let data = content.clone();
            thread::spawn(move || {
                let mut request = [0; 1024];
                let _ = stream.read(&mut request);
                let is_head = request.starts_with(b"HEAD");
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    data.len()
                );
                let _ = stream.write_all(header.as_bytes());
                if !is_head {
                    let _ = stream.write_all(&data);
                }
                let _ = stream.flush();
            });
        }
    });

    (format!("http://{address}/test_payload.bin"), stop_tx)
}

fn create_service(
    dir: &Path,
    action_executor: impl ActionExecutor + 'static,
) -> (DownloadTransferService, DownloadHistoryService) {
    let history_store = DownloadHistoryStore::new(dir);
    let history = DownloadHistoryService::new(history_store).unwrap();
    let allocator = open_download_manager_lib::domain::download::DestinationAllocator::new();
    let http_executor = HttpDownloadExecutor::new().unwrap();
    let service = DownloadTransferService::with_all(
        history.clone(),
        http_executor,
        2,
        SystemShellService::new(),
        allocator,
        action_executor,
    );
    (service, history)
}

#[tokio::test]
async fn scanner_invocation_uses_safe_arguments_and_substitutes_file() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"VALID_DOWNLOAD_BYTES".to_vec());
    let mock_executor = MockActionExecutor::new();
    let executed_commands = mock_executor.executed_commands.clone();
    let (service, history) = create_service(&dir, mock_executor);

    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::AntivirusScan {
            executable: "/usr/bin/mock-scanner".to_string(),
            arguments: vec![
                "--mode=deep".to_string(),
                "--target={file}".to_string(),
                "--log".to_string(),
            ],
        },
    ];

    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("my report (2026).pdf".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    // Wait for completion
    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record = history.download(id).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);

    // Verify command was invoked with safe arguments without shell interpolation
    let commands = executed_commands.lock().unwrap();
    assert_eq!(commands.len(), 1);
    let (executable, args) = &commands[0];
    assert_eq!(executable, "/usr/bin/mock-scanner");
    assert_eq!(args.len(), 3);
    assert_eq!(args[0], "--mode=deep");
    let target_path = dir.join("my report (2026).pdf").to_string_lossy().into_owned();
    assert_eq!(args[1], format!("--target={target_path}"));
    assert_eq!(args[2], "--log");

    // Verify action ledger records success
    assert_eq!(record.action_ledger.len(), 1);
    let entry = &record.action_ledger[0];
    assert_eq!(entry.action_type, "antivirusScan");
    assert_eq!(entry.status, ActionStatus::Success);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn scanner_failure_records_error_without_marking_download_failed() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"INFECTED_OR_SCAN_FAIL_BYTES".to_vec());
    let mock_executor = MockActionExecutor::with_command_error("Infection detected: Trojan.Generic.123");
    let (service, history) = create_service(&dir, mock_executor);

    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::AntivirusScan {
            executable: "/usr/bin/clamscan".to_string(),
            arguments: vec!["--infected".to_string()],
        },
    ];

    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("suspicious.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    // Wait for completion
    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record = history.download(id).unwrap();
    // Critical: download status must remain Completed!
    assert_eq!(
        record.status,
        DownloadStatus::Completed,
        "Scanner failure must NOT change download status back to Failed"
    );

    // Verify failure is clearly exposed in action ledger and notice
    assert_eq!(record.action_ledger.len(), 1);
    let entry = &record.action_ledger[0];
    assert_eq!(entry.action_type, "antivirusScan");
    assert_eq!(entry.status, ActionStatus::Failed);
    assert!(entry.message.as_ref().unwrap().contains("Trojan.Generic.123"));

    assert!(record.notice.is_some());
    assert!(record.notice.as_ref().unwrap().contains("Trojan.Generic.123"));

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn real_system_executor_reports_missing_executable_safely() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"DOWNLOADED_BYTES".to_vec());
    let system_executor = SystemActionExecutor::new();
    let (service, history) = create_service(&dir, system_executor);

    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::AntivirusScan {
            executable: "/non_existent_binary_xyz_123".to_string(),
            arguments: vec!["--scan".to_string()],
        },
    ];

    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("clean_file.txt".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record = history.download(id).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);
    assert_eq!(record.action_ledger.len(), 1);
    let entry = &record.action_ledger[0];
    assert_eq!(entry.status, ActionStatus::Failed);
    assert!(
        entry.message.as_ref().unwrap().contains("Executable not found"),
        "Should cleanly report missing executable"
    );

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn power_action_requires_explicit_confirmation() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"DATA".to_vec());
    let mock_executor = MockActionExecutor::new();
    let power_actions = mock_executor.power_actions.clone();
    let (service, history) = create_service(&dir, mock_executor);

    // 1. Unconfirmed power action must be skipped
    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::PowerAction {
            kind: PowerActionKind::Shutdown,
            confirmed: false,
        },
    ];

    let request = DownloadRequest {
        urls: vec![url.clone()],
        filename: Some("unconfirmed_shutdown.dat".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record = history.download(id).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);
    assert_eq!(record.action_ledger.len(), 1);
    assert_eq!(record.action_ledger[0].status, ActionStatus::Skipped);
    assert!(record.action_ledger[0].message.as_ref().unwrap().contains("explicit confirmation"));
    assert_eq!(power_actions.lock().unwrap().len(), 0, "No power action must be dispatched without confirmation");

    // 2. Confirmed power action executes successfully
    let (url2, _stop2) = serve_simple_file(b"DATA2".to_vec());
    settings.completion_actions = vec![
        CompletionAction::PowerAction {
            kind: PowerActionKind::Sleep,
            confirmed: true,
        },
    ];

    let request2 = DownloadRequest {
        urls: vec![url2],
        filename: Some("confirmed_sleep.dat".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids2 = service.add(request2, &settings).unwrap();
    let id2 = &ids2[0];

    for _ in 0..100 {
        let record = history.download(id2).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record2 = history.download(id2).unwrap();
    assert_eq!(record2.status, DownloadStatus::Completed);
    assert_eq!(record2.action_ledger.len(), 1);
    assert_eq!(record2.action_ledger[0].status, ActionStatus::Success);
    assert_eq!(power_actions.lock().unwrap().as_slice(), &[PowerActionKind::Sleep]);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn per_download_actions_override_global_settings_deterministically() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"PAYLOAD".to_vec());
    let mock_executor = MockActionExecutor::new();
    let played_sounds = mock_executor.played_sounds.clone();
    let executed_commands = mock_executor.executed_commands.clone();
    let (service, history) = create_service(&dir, mock_executor);

    // Global settings specify PlaySound
    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::PlaySound { sound_file: Some("/sounds/global.wav".to_string()) },
    ];

    // Download-specific override specifies ExecuteCommand instead
    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("override.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: Some(vec![
            CompletionAction::ExecuteCommand {
                executable: "/usr/local/bin/notify_slack".to_string(),
                arguments: vec!["--finished".to_string(), "{file}".to_string()],
            },
        ]),
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let record = history.download(id).unwrap();
    assert_eq!(record.status, DownloadStatus::Completed);

    // Sound was NOT played because per-download action took precedence
    assert_eq!(played_sounds.lock().unwrap().len(), 0);

    // Custom command WAS executed
    assert_eq!(executed_commands.lock().unwrap().len(), 1);
    assert_eq!(executed_commands.lock().unwrap()[0].0, "/usr/local/bin/notify_slack");

    // Action ledger reflects per-download command
    assert_eq!(record.action_ledger.len(), 1);
    assert_eq!(record.action_ledger[0].action_type, "executeCommand");

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn actions_do_not_run_twice_on_restart_but_run_when_retried() {
    let dir = temporary_directory();
    let (url, _stop) = serve_simple_file(b"IMMUTABLE_OUTPUT".to_vec());
    let mock_executor = MockActionExecutor::new();
    let executed_commands = mock_executor.executed_commands.clone();
    let (service, history) = create_service(&dir, mock_executor);

    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::ExecuteCommand {
            executable: "/bin/log_completion".to_string(),
            arguments: vec!["--done".to_string()],
        },
    ];

    let request = DownloadRequest {
        urls: vec![url],
        filename: Some("single_run.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Completed && !record.action_ledger.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert_eq!(executed_commands.lock().unwrap().len(), 1);

    // Simulate calling run_completion_actions again (e.g. app restart or duplicate completion trigger)
    let final_path = dir.join("single_run.bin");
    service.run_completion_actions(id, &final_path).await;

    // Single-run ledger must prevent duplicate invocation!
    assert_eq!(
        executed_commands.lock().unwrap().len(),
        1,
        "Actions must NOT run twice when action is already recorded in ledger"
    );

    // Now test retry: user explicitly retries completion actions via retry_completion_actions
    service.retry_completion_actions(id).await.unwrap();

    // After explicit retry, action runs again
    assert_eq!(
        executed_commands.lock().unwrap().len(),
        2,
        "Explicit retry must allow actions to run again"
    );

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn cancelled_download_never_executes_completion_actions() {
    let dir = temporary_directory();
    let listener = TcpListener::bind("127.0.0.1:0").expect("local fixture should bind");
    let address = listener.local_addr().expect("local fixture should expose address");

    // Slow stream that allows cancelling in progress
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req = [0; 1024];
            let _ = stream.read(&mut req);
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 10000000\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
            // Write partial chunk and pause
            let _ = stream.write_all(&[0x42; 512]);
            let _ = stream.flush();
            thread::sleep(Duration::from_secs(5));
        }
    });

    let mock_executor = MockActionExecutor::new();
    let executed_commands = mock_executor.executed_commands.clone();
    let power_actions = mock_executor.power_actions.clone();
    let (service, history) = create_service(&dir, mock_executor);

    let mut settings = Settings::new(dir.clone());
    settings.completion_actions = vec![
        CompletionAction::PowerAction { kind: PowerActionKind::Shutdown, confirmed: true },
        CompletionAction::ExecuteCommand { executable: "/bin/echo".to_string(), arguments: vec![] },
    ];

    let request = DownloadRequest {
        urls: vec![format!("http://{address}/large_stream.bin")],
        filename: Some("large_stream.bin".to_string()),
        directory: Some(dir.to_string_lossy().into_owned()),
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
        conflict_policy: None,
        credentials: None,
        completion_actions: None,
    };

    let ids = service.add(request, &settings).unwrap();
    let id = &ids[0];

    // Wait until downloading starts
    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Downloading {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Cancel download
    service.action(id, DownloadAction::Cancel).unwrap();

    // Wait for cancellation to settle
    for _ in 0..100 {
        let record = history.download(id).unwrap();
        if record.status == DownloadStatus::Cancelled {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let record = history.download(id).unwrap();
    assert_eq!(record.status, DownloadStatus::Cancelled);

    // Verify ZERO completion actions were executed
    assert_eq!(executed_commands.lock().unwrap().len(), 0);
    assert_eq!(power_actions.lock().unwrap().len(), 0);
    assert_eq!(record.action_ledger.len(), 0);

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn system_executor_process_timeout_guard_kills_child_and_records_failure() {
    let system_executor = SystemActionExecutor::new();
    let result = system_executor
        .execute_process("sleep", &["5".to_string()], Duration::from_millis(150))
        .await;

    assert!(result.is_err(), "Sleep 5s should time out within 150ms");
    let err = result.unwrap_err();
    assert!(
        err.contains("timed out"),
        "Error message should indicate timeout: {err}"
    );
}

#[tokio::test]
async fn safe_argument_handling_prevents_shell_injection_and_passes_special_characters() {
    let system_executor = SystemActionExecutor::new();
    let raw_arg = r#"payload'; rm -rf /; $(whoami) & "double quoted""#;
    let result = system_executor
        .execute_process("echo", &[raw_arg.to_string()], Duration::from_secs(5))
        .await;

    assert!(result.is_ok(), "Echo with special arguments should succeed");
    let output = result.unwrap();
    assert_eq!(
        output, raw_arg,
        "Arguments must be passed verbatim without shell evaluation or command injection"
    );
}
