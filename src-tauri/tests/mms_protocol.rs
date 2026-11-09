// Created: 2026-11-08
// Reason: feat(mms): add MMS streaming support or explicit unsupported guidance (Closes #35)
// Last updated: 2026-11-08
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Integration tests verifying MMS scheme detection, unsupported migration guidance, safe ASX/WVX parsing, streaming, and cancellation.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use open_download_manager_lib::{
    application::{
        download_executor::{DownloadControl, DownloadExecutor, DownloadStopReason},
        download_rate_limiter::DownloadRateLimiter,
    },
    domain::{
        download::{
            BrowserRequestHeaders, DestinationAllocator, DownloadAddress, DownloadProtocol,
            DownloadRecord, FileConflictPolicy,
        },
        protocol::mms::{
            AsxPlaylistParser, MmsProtocolDetector, MmsSessionState, MmsSessionStateMachine,
            MmsVariant, MAX_ASX_PLAYLIST_BYTES,
        },
    },
    infrastructure::mms::{MmsDownloadExecutor, TEMPORARY_DOWNLOAD_PREFIX},
};
use tokio::sync::watch;

fn temporary_directory(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("odm_test_{prefix}_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create test temp dir");
    dir
}

#[test]
fn human_scenario_mms_schemes_detection_and_validation() {
    // 1. Supported MMS schemes parse cleanly into DownloadProtocol::Mms
    let mms_addr = DownloadAddress::parse("mms://media.example.com/live/broadcast.asf")
        .expect("mms scheme parses");
    assert_eq!(mms_addr.protocol(), DownloadProtocol::Mms);

    let mmsh_addr = DownloadAddress::parse("mmsh://media.example.com:8080/stream.wmv")
        .expect("mmsh scheme parses");
    assert_eq!(mmsh_addr.protocol(), DownloadProtocol::Mms);

    // 2. MmsProtocolDetector correctly identifies variants
    assert_eq!(
        MmsProtocolDetector::detect_scheme("mms://example.com/audio"),
        Some(MmsVariant::Mms)
    );
    assert_eq!(
        MmsProtocolDetector::detect_scheme("mmsh://example.com/audio"),
        Some(MmsVariant::Mmsh)
    );
    assert_eq!(
        MmsProtocolDetector::detect_scheme("mmst://example.com/audio"),
        Some(MmsVariant::Mmst)
    );
    assert_eq!(
        MmsProtocolDetector::detect_scheme("mmsu://example.com/audio"),
        Some(MmsVariant::Mmsu)
    );
    assert_eq!(
        MmsProtocolDetector::detect_scheme("https://example.com/file.mp4"),
        None
    );
}

#[test]
fn human_scenario_unsupported_mms_variants_rejected_with_actionable_guidance() {
    // 1. MMST (MMS over TCP port 1755) is rejected at validation layer with migration guidance
    let mmst_err = DownloadAddress::parse("mmst://stream.example.com:1755/stream.asf")
        .expect_err("mmst is unsupported");
    assert!(
        mmst_err.contains("MMST (MMS over TCP port 1755) is an obsolete, unmaintained proprietary protocol"),
        "Unexpected error: {mmst_err}"
    );
    assert!(
        mmst_err.contains("Please use an HTTP, HTTPS, or RTSP media source, or extract the stream URL from an ASX/WVX playlist"),
        "Missing migration guidance in: {mmst_err}"
    );

    // 2. MMSU (MMS over UDP port 1755) is rejected at validation layer with migration guidance
    let mmsu_err = DownloadAddress::parse("mmsu://stream.example.com:1755/live.wmv")
        .expect_err("mmsu is unsupported");
    assert!(
        mmsu_err.contains("MMSU (MMS over UDP port 1755) is an obsolete, unmaintained proprietary protocol"),
        "Unexpected error: {mmsu_err}"
    );
    assert!(
        mmsu_err.contains("Please use an HTTP, HTTPS, or RTSP media source, or extract the stream URL from an ASX/WVX playlist"),
        "Missing migration guidance in: {mmsu_err}"
    );
}

#[test]
fn human_scenario_asx_wvx_playlist_intake_rejected_as_direct_file_with_guidance() {
    // ASX and WVX stream wrapper URLs cannot be directly downloaded as flat files;
    // they must be rejected with actionable guidance directing the user to playlist resolution.
    let asx_err = DownloadAddress::parse("http://radio.example.com/live.asx")
        .expect_err("asx wrapper rejected as direct file");
    assert!(
        asx_err.contains("ASX and WVX playlists are stream wrappers, not direct media files"),
        "Unexpected error: {asx_err}"
    );

    let wvx_err = DownloadAddress::parse("https://video.example.com/feed.wvx?session=123")
        .expect_err("wvx wrapper rejected as direct file");
    assert!(
        wvx_err.contains("ASX and WVX playlists are stream wrappers, not direct media files"),
        "Unexpected error: {wvx_err}"
    );
}

#[test]
fn human_scenario_safe_bounded_asx_and_wvx_parsing() {
    // 1. XML ASX format parsing
    let xml_asx = r#"
        <ASX version="3.0">
            <Title>Classic Radio Broadcast</Title>
            <Entry>
                <Title>Station Stream 1</Title>
                <Ref href="http://cdn.example.com/audio/stream1.wma" />
            </Entry>
            <Entry>
                <Title>Station Stream 2 (Fallback)</Title>
                <Ref href="mms://live.example.com/broadcast.asf" />
            </Entry>
        </ASX>
    "#;

    let playlist = AsxPlaylistParser::parse_str(xml_asx).expect("XML ASX parses");
    assert_eq!(playlist.title.as_deref(), Some("Classic Radio Broadcast"));
    assert_eq!(playlist.entries.len(), 2);
    assert_eq!(playlist.entries[0].title.as_deref(), Some("Station Stream 1"));
    assert_eq!(playlist.entries[0].href, "http://cdn.example.com/audio/stream1.wma");
    assert!(playlist.entries[0].is_supported_fallback);
    assert!(!playlist.entries[0].is_mms);

    assert_eq!(playlist.entries[1].title.as_deref(), Some("Station Stream 2 (Fallback)"));
    assert_eq!(playlist.entries[1].href, "mms://live.example.com/broadcast.asf");
    assert!(!playlist.entries[1].is_supported_fallback);
    assert!(playlist.entries[1].is_mms);

    assert!(playlist.has_supported_fallback());
    assert_eq!(
        playlist.supported_fallback_urls(),
        vec!["http://cdn.example.com/audio/stream1.wma"]
    );
    assert_eq!(
        playlist.mms_urls(),
        vec!["mms://live.example.com/broadcast.asf"]
    );

    // 2. INI ASX format parsing
    let ini_asx = r#"
        [Reference]
        Title=Archive Media Feed
        Ref1=http://archive.example.com/video1.wmv
        Ref2=mmsh://media.example.com/live2.asf
    "#;

    let ini_playlist = AsxPlaylistParser::parse_str(ini_asx).expect("INI ASX parses");
    assert_eq!(ini_playlist.title.as_deref(), Some("Archive Media Feed"));
    assert_eq!(ini_playlist.entries.len(), 2);
    assert_eq!(ini_playlist.entries[0].href, "http://archive.example.com/video1.wmv");
    assert!(ini_playlist.entries[0].is_supported_fallback);
    assert_eq!(ini_playlist.entries[1].href, "mmsh://media.example.com/live2.asf");
    assert!(ini_playlist.entries[1].is_mms);

    // 3. Bounded playlist size rejection: Oversized file (>1 MiB) rejected safely
    let oversized = vec![b' '; MAX_ASX_PLAYLIST_BYTES + 10];
    let oversize_err = AsxPlaylistParser::parse_bytes(&oversized).expect_err("oversized rejected");
    assert!(oversize_err.contains("exceeds the allowed limit"));

    // 4. Empty or malformed input
    let empty_err = AsxPlaylistParser::parse_str("   ").expect_err("empty rejected");
    assert!(empty_err.contains("empty"));
}

#[test]
fn human_scenario_mms_session_state_machine_lifecycle() {
    let mut sm = MmsSessionStateMachine::new();
    assert_eq!(sm.state(), &MmsSessionState::Initial);

    sm.start_connection("http://media.example.com/stream");
    assert!(matches!(sm.state(), MmsSessionState::Connecting { .. }));
    assert!(sm.diagnostic_events().iter().any(|e| e.contains("MMSS:TrStNCn")));

    sm.send_connection_info(0);
    assert!(matches!(sm.state(), MmsSessionState::Handshaking { .. }));
    assert!(sm.diagnostic_events().iter().any(|e| e.contains("MMSSss:SndAlCnInf")));

    sm.begin_streaming();
    assert!(matches!(sm.state(), MmsSessionState::Streaming { .. }));

    sm.record_chunk(4096);
    sm.record_chunk(2048);
    if let MmsSessionState::Streaming { bytes_received, chunks_processed } = sm.state() {
        assert_eq!(*bytes_received, 6144);
        assert_eq!(*chunks_processed, 2);
    } else {
        panic!("Expected Streaming state");
    }

    sm.complete(6144);
    assert_eq!(sm.state(), &MmsSessionState::Completed { total_bytes: 6144 });
}

#[tokio::test]
async fn human_scenario_mms_streaming_execution_with_mock_server() {
    let test_dir = temporary_directory("mms_exec");

    // Construct mock ASF chunked payload:
    // Header chunk: $H + length (12) + payload (12 bytes)
    // Data chunk:   $D + length (16) + payload (16 bytes)
    // End chunk:    $E + length (0)
    let mut mock_stream = Vec::new();

    // $H chunk
    let h_payload = b"ASF_HEADER_DATA_1234";
    mock_stream.extend_from_slice(b"$H");
    mock_stream.extend_from_slice(&(h_payload.len() as u16).to_le_bytes());
    mock_stream.extend_from_slice(h_payload);

    // $D chunk
    let d_payload = b"MEDIA_PACKET_FRAME_5678";
    mock_stream.extend_from_slice(b"$D");
    mock_stream.extend_from_slice(&(d_payload.len() as u16).to_le_bytes());
    mock_stream.extend_from_slice(d_payload);

    // $E chunk
    mock_stream.extend_from_slice(b"$E\x00\x00");

    let server_listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let server_port = server_listener.local_addr().unwrap().port();
    let stream_copy = mock_stream.clone();

    let server_thread = thread::spawn(move || {
        if let Ok((mut socket, _)) = server_listener.accept() {
            let mut buf = [0u8; 1024];
            let read_bytes = socket.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..read_bytes]);

            // Verify client sent MMS session headers (case-insensitive)
            let req_lower = req_str.to_ascii_lowercase();
            assert!(req_lower.contains("pragma: no-cache") || req_lower.contains("pragma:"));
            assert!(req_lower.contains("nsplayer"));

            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.ms.asf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                stream_copy.len()
            );
            let _ = socket.write_all(resp.as_bytes());
            let _ = socket.write_all(&stream_copy);
        }
    });

    let allocator = DestinationAllocator::new();
    let executor = MmsDownloadExecutor::new(allocator);

    let download_id = "test_mms_1".to_string();
    let download = DownloadRecord {
        id: download_id.clone(),
        url: format!("mms://127.0.0.1:{server_port}/live.asf"),
        filename: "test_mms_media.asf".to_string(),
        directory: Some(test_dir.to_str().unwrap().to_string()),
        path: None,
        temp_directory: None,
        status: open_download_manager_lib::domain::download::DownloadStatus::Downloading,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("mms".to_string()),
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: Some("mms".to_string()),
        etag: None,
        validator: None,
        conflict_policy: Some(FileConflictPolicy::Overwrite),
        completion_actions: None,
        action_ledger: Vec::new(),
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        sabr: None,
    };

    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_handler = Arc::new(|_| {});

    let result = executor
        .execute(download, progress_handler, control, rate_limiter)
        .await
        .expect("MMS download succeeds");

    server_thread.join().expect("server completes");

    // Verify downloaded file exists at destination
    let dest_file = test_dir.join("test_mms_media.asf");
    assert!(dest_file.exists());
    assert_eq!(result.path, dest_file);

    // Content should match unpacked framed payload (header + data)
    let saved_bytes = fs::read(&dest_file).unwrap();
    let mut expected_payload = Vec::new();
    expected_payload.extend_from_slice(h_payload);
    expected_payload.extend_from_slice(d_payload);
    assert_eq!(saved_bytes, expected_payload);

    // Verify .part file was cleaned up on completion
    let part_file = test_dir.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{download_id}.part"));
    assert!(!part_file.exists());
}

#[tokio::test]
async fn human_scenario_mms_streaming_instant_cancellation() {
    let test_dir = temporary_directory("mms_cancel");

    let server_listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let server_port = server_listener.local_addr().unwrap().port();
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let server_thread = thread::spawn(move || {
        if let Ok((mut socket, _)) = server_listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf);
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.ms.asf\r\nConnection: keep-alive\r\n\r\n";
            let _ = socket.write_all(resp.as_bytes());

            // Send infinite slow chunks until cancellation
            while running_clone.load(Ordering::Relaxed) {
                let _ = socket.write_all(b"STREAM_SLOW_CHUNK_12345678");
                thread::sleep(Duration::from_millis(50));
            }
        }
    });

    let allocator = DestinationAllocator::new();
    let executor = MmsDownloadExecutor::new(allocator);

    let download_id = "test_mms_cancel".to_string();
    let download = DownloadRecord {
        id: download_id.clone(),
        url: format!("mms://127.0.0.1:{server_port}/endless.asf"),
        filename: "cancelled_mms.asf".to_string(),
        directory: Some(test_dir.to_str().unwrap().to_string()),
        path: None,
        temp_directory: None,
        status: open_download_manager_lib::domain::download::DownloadStatus::Downloading,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("mms".to_string()),
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: Some("mms".to_string()),
        etag: None,
        validator: None,
        conflict_policy: Some(FileConflictPolicy::Overwrite),
        completion_actions: None,
        action_ledger: Vec::new(),
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        sabr: None,
    };

    let (tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_handler = Arc::new(|_| {});

    let exec_task = tokio::spawn(async move {
        executor
            .execute(download, progress_handler, control, rate_limiter)
            .await
    });

    // Let the download connect and stream a few bytes
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Send cancellation signal
    let _ = tx.send(Some(DownloadStopReason::Cancel));

    // Await execution - must exit immediately with download_cancelled error
    let result = exec_task.await.unwrap();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, "download_cancelled");

    running.store(false, Ordering::Relaxed);
    server_thread.join().expect("server completes");

    // Verify partial file was preserved for potential resumption
    let part_file = test_dir.join(format!("{TEMPORARY_DOWNLOAD_PREFIX}{download_id}.part"));
    assert!(part_file.exists());
    assert!(fs::metadata(&part_file).unwrap().len() > 0);
}

#[tokio::test]
async fn human_scenario_mms_stream_reconnect_and_resume() {
    let test_dir = temporary_directory("mms_reconnect");

    let server_listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let server_port = server_listener.local_addr().unwrap().port();

    let part1 = b"FIRST_HALF_OF_MMS_STREAM_CHUNK";
    let part2 = b"SECOND_HALF_OF_MMS_STREAM_CHUNK";
    let full_content = [part1.as_slice(), part2.as_slice()].concat();
    let full_copy = full_content.clone();

    let server_thread = thread::spawn(move || {
        // First connection: sends part1, then abruptly closes connection
        if let Ok((mut socket, _)) = server_listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.ms.asf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                full_copy.len()
            );
            let _ = socket.write_all(resp.as_bytes());
            let _ = socket.write_all(part1);
            // Drop socket to simulate network break
            drop(socket);
        }

        // Reconnect: client sends Range: bytes=30-, server sends remaining part2 with 206 Partial Content
        if let Ok((mut socket, _)) = server_listener.accept() {
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).unwrap_or(0);
            let req_str = String::from_utf8_lossy(&buf[..n]);

            assert!(
                req_str.to_ascii_lowercase().contains(&format!("range: bytes={}-", part1.len())),
                "Expected range header in reconnect request: {req_str}"
            );

            let resp = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: application/vnd.ms.asf\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                part1.len(),
                full_copy.len() - 1,
                full_copy.len(),
                part2.len()
            );
            let _ = socket.write_all(resp.as_bytes());
            let _ = socket.write_all(part2);
        }
    });

    let allocator = DestinationAllocator::new();
    let executor = MmsDownloadExecutor::new(allocator);

    let download_id = "test_mms_resume".to_string();
    let download = DownloadRecord {
        id: download_id.clone(),
        url: format!("mms://127.0.0.1:{server_port}/stream.asf"),
        filename: "reconnected_stream.asf".to_string(),
        directory: Some(test_dir.to_str().unwrap().to_string()),
        path: None,
        temp_directory: None,
        status: open_download_manager_lib::domain::download::DownloadStatus::Downloading,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("mms".to_string()),
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: Some("mms".to_string()),
        etag: None,
        validator: None,
        conflict_policy: Some(FileConflictPolicy::Overwrite),
        completion_actions: None,
        action_ledger: Vec::new(),
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        sabr: None,
    };

    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_handler = Arc::new(|_| {});

    let result = executor
        .execute(download, progress_handler, control, rate_limiter)
        .await
        .expect("MMS reconnected download succeeds");

    server_thread.join().expect("server thread completes");

    assert_eq!(result.downloaded, full_content.len() as u64);
    let saved = fs::read(&result.path).unwrap();
    assert_eq!(saved, full_content);
}

#[tokio::test]
async fn human_scenario_mms_does_not_accidentally_fall_back_to_generic_http() {
    let test_dir = temporary_directory("mms_no_fallback");

    // Mock server expects NSPlayer User-Agent and MMS headers; generic HTTP requests would be rejected with 400
    let server_listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let server_port = server_listener.local_addr().unwrap().port();

    let server_thread = thread::spawn(move || {
        if let Ok((mut socket, _)) = server_listener.accept() {
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);

            // If an HTTP executor made the request without MMS headers, return HTTP 400
            if !req.to_ascii_lowercase().contains("nsplayer") {
                let _ = socket.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
                return;
            }

            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.ms.asf\r\nContent-Length: 10\r\nConnection: close\r\n\r\n0123456789";
            let _ = socket.write_all(resp.as_bytes());
        }
    });

    let allocator = DestinationAllocator::new();
    let executor = MmsDownloadExecutor::new(allocator);

    let download_id = "test_mms_route".to_string();
    let download = DownloadRecord {
        id: download_id.clone(),
        url: format!("mms://127.0.0.1:{server_port}/routed.asf"),
        filename: "routed.asf".to_string(),
        directory: Some(test_dir.to_str().unwrap().to_string()),
        path: None,
        temp_directory: None,
        status: open_download_manager_lib::domain::download::DownloadStatus::Downloading,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-11-08T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        queue_id: None,
        description: None,
        category: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("mms".to_string()),
        resumable: Some(true),
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        audio_url: None,
        referer: None,
        youtube_format: None,
        transfer_kind: Some("mms".to_string()),
        etag: None,
        validator: None,
        conflict_policy: Some(FileConflictPolicy::Overwrite),
        completion_actions: None,
        action_ledger: Vec::new(),
        credentials_override: None,
        request_headers: BrowserRequestHeaders::default(),
        sabr: None,
    };

    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_handler = Arc::new(|_| {});

    let result = executor
        .execute(download, progress_handler, control, rate_limiter)
        .await
        .expect("MMS correctly routed and completed without generic fallback");

    server_thread.join().expect("server completes");
    assert_eq!(result.downloaded, 10);
    assert!(result.path.exists());
}
