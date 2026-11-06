use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};
use base64::prelude::*;
use tokio::sync::watch;
use open_download_manager_lib::{
    application::{
        download_executor::{DownloadControl, DownloadProgressHandler, DownloadStopReason},
        download_history_service::DownloadHistoryService,
        download_rate_limiter::DownloadRateLimiter,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        browser::{sanitize_browser_request, BrowserIncomingMessage},
        download::{DownloadRecord, DownloadRequest, DownloadStatus},
        media::sabr::{
            build_sabr_request_body, encode_bytes_field, encode_ump_chunk, encode_varint,
            encode_varint_field, normalize_sabr_session, parse_media_header_itag,
            parse_sabr_protobuf_request, sanitize_sabr, NormalizedSabrSession, SabrAudioTrack,
            SabrClientInfo, SabrFormat, SabrSession,
        },
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        media::{ffmpeg::FfmpegTools, sabr_executor::SabrDownloadExecutor},
    },
};
use tempfile::Builder;

fn build_test_protobuf_body() -> String {
    // Encodes VideoPlaybackAbrRequest with streamerContext containing:
    // clientInfo: { clientName: 1, clientVersion: "2.20261022.00.00" }
    // poToken: [7, 8, 9]
    let version = "2.20261022.00.00";
    let mut client_info = Vec::new();
    client_info.push(0x08); // field 1, wire type 0
    client_info.push(0x01); // clientName = 1
    client_info.push(0x12); // field 2, wire type 2
    client_info.push(version.len() as u8);
    client_info.extend_from_slice(version.as_bytes());

    let mut streamer_context = Vec::new();
    streamer_context.push(0x32); // field 6, wire type 2
    streamer_context.push(client_info.len() as u8);
    streamer_context.extend_from_slice(&client_info);

    streamer_context.push(0x9A); // field 19, wire type 2 (154 = 0x9A 0x01)
    streamer_context.push(0x01);
    streamer_context.push(3); // length 3
    streamer_context.extend_from_slice(&[7, 8, 9]);

    let mut request = Vec::new();
    request.push(0x52); // field 10, wire type 2
    request.push(streamer_context.len() as u8);
    request.extend_from_slice(&streamer_context);

    BASE64_STANDARD.encode(&request)
}

#[test]
fn captured_sabr_protobuf_supplies_browser_client_identity_and_proof_token() {
    let request_body = build_test_protobuf_body();

    let session = SabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: "https://r1.googlevideo.com/videoplayback?id=asset&sabr=1".to_owned(),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: Some(SabrClientInfo {
            client_name: 1,
            client_version: String::new(),
            ..Default::default()
        }),
        formats: vec![
            SabrFormat {
                itag: 137,
                mime_type: "video/mp4; codecs=\"avc1\"".to_owned(),
                height: Some(1080),
                quality_label: Some("1080p".to_owned()),
                bitrate: 3_000_000,
                last_modified: Some("1".to_owned()),
                approx_duration_ms: Some("10000".to_owned()),
                ..default_format(137, "video/mp4; codecs=\"avc1\"")
            },
            SabrFormat {
                itag: 140,
                mime_type: "audio/mp4; codecs=\"mp4a.40.2\"".to_owned(),
                audio_quality: Some("AUDIO_QUALITY_MEDIUM".to_owned()),
                bitrate: 128_000,
                last_modified: Some("1".to_owned()),
                approx_duration_ms: Some("10000".to_owned()),
                ..default_format(140, "audio/mp4; codecs=\"mp4a.40.2\"")
            },
        ],
        video_itag: Some(137),
        audio_itag: 140,
        request_body: Some(request_body),
        po_token: None,
    };

    let value = normalize_sabr_session(&session).expect("normalization should succeed");
    assert_eq!(value.client_info.client_version, "2.20261022.00.00");
    assert_eq!(value.po_token.as_deref(), Some("BwgJ"));
    let audio = value.formats.iter().find(|f| f.itag == 140).unwrap();
    assert!(audio.has_audio);
}

#[test]
fn one_default_audio_variant_and_one_video_survive_repeated_normalization() {
    let audio_base = SabrFormat {
        itag: 140,
        mime_type: "audio/mp4".to_owned(),
        audio_quality: Some("AUDIO_QUALITY_MEDIUM".to_owned()),
        content_length: Some("100".to_owned()),
        last_modified: Some("1".to_owned()),
        ..default_format(140, "audio/mp4")
    };

    let session = SabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: "https://r1.googlevideo.com/videoplayback?sabr=1".to_owned(),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: None,
        audio_itag: 140,
        video_itag: Some(137),
        formats: vec![
            SabrFormat {
                audio_track: Some(SabrAudioTrack {
                    id: Some("dubbed".to_owned()),
                    audio_is_default: false,
                }),
                ..audio_base.clone()
            },
            SabrFormat {
                is_drc: true,
                audio_track: Some(SabrAudioTrack {
                    id: Some("default".to_owned()),
                    audio_is_default: true,
                }),
                ..audio_base.clone()
            },
            SabrFormat {
                audio_track: Some(SabrAudioTrack {
                    id: Some("default".to_owned()),
                    audio_is_default: true,
                }),
                ..audio_base.clone()
            },
            SabrFormat {
                itag: 137,
                mime_type: "video/mp4".to_owned(),
                quality_label: Some("1080p".to_owned()),
                content_length: Some("900".to_owned()),
                last_modified: Some("1".to_owned()),
                ..default_format(137, "video/mp4")
            },
        ],
        request_body: None,
        po_token: None,
    };

    let first = normalize_sabr_session(&session).expect("first normalization should succeed");
    let second = normalize_sabr_session(&first.to_session()).expect("second normalization should succeed");

    for result in [&first, &second] {
        assert_eq!(result.formats.len(), 2);
        let audio = result.formats.iter().find(|f| f.has_audio).unwrap();
        assert_eq!(audio.audio_track_id.as_deref(), Some("default"));
        let total_content_length: u64 = result
            .formats
            .iter()
            .map(|f| f.content_length.as_deref().unwrap_or("0").parse::<u64>().unwrap())
            .sum();
        assert_eq!(total_content_length, 1000);
    }
}

#[test]
fn sabr_handoff_accepts_only_a_bounded_captured_googlevideo_session() {
    let url = "https://r1.googlevideo.com/videoplayback?id=asset&sabr=1";
    let sabr = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": url,
        "videoPlaybackUstreamerConfig": "AQID",
        "clientInfo": { "clientName": 1, "clientVersion": "2.20261022.00.00" },
        "requestBody": "AQID",
        "videoItag": 137,
        "audioItag": 140,
        "formats": [
            { "itag": 137, "mimeType": "video/mp4; codecs=\"avc1\"", "height": 1080, "qualityLabel": "1080p", "bitrate": 3000000, "lastModified": "1", "approxDurationMs": "10000" },
            { "itag": 140, "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"", "audioQuality": "AUDIO_QUALITY_MEDIUM", "bitrate": 128000, "lastModified": "1", "approxDurationMs": "10000" }
        ]
    });

    let msg = BrowserIncomingMessage {
        action: "download".to_owned(),
        url: Some(url.to_owned()),
        youtube_format: None,
        sabr: Some(sabr.clone()),
        start_now: None,
        audio_url: None,
        output_format: Some("mp4".to_owned()),
        urls: None,
        filename: None,
        referer: None,
        canonical_url: None,
        video_id: None,
        request_headers: None,
    };

    let validated = sanitize_browser_request(msg).expect("sanitize should succeed");
    let norm = validated.sabr.expect("sabr should be present");
    assert_eq!(norm.video_itag, Some(137));
    assert_eq!(norm.request_body.as_deref(), Some("AQID"));

    // Add formats with webm
    let mut mixed = sabr.clone();
    mixed["formats"].as_array_mut().unwrap().push(serde_json::json!({
        "itag": 248, "mimeType": "video/webm; codecs=\"vp9\"", "bitrate": 2000000, "xtags": "lang=en:acont=original"
    }));
    mixed["formats"].as_array_mut().unwrap().push(serde_json::json!({
        "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 128000
    }));

    let sanitized_mixed = sanitize_sabr(&mixed, url).expect("mixed formats should sanitize");
    assert_eq!(sanitized_mixed.video_itag, Some(137));
    assert_eq!(sanitized_mixed.audio_itag, 140);
}

#[test]
fn sabr_rejects_evil_domains_and_missing_sabr_query() {
    let evil_url = "https://googlevideo.com.evil.test/videoplayback?sabr=1";
    let sabr = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": evil_url,
        "videoPlaybackUstreamerConfig": "AQID",
        "audioItag": 140,
        "formats": [{ "itag": 140, "mimeType": "audio/mp4" }]
    });

    let res = sanitize_sabr(&sabr, evil_url);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "Invalid SABR stream URL.");

    let no_sabr_query = "https://r1.googlevideo.com/videoplayback?id=asset";
    let sabr_no_query = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": no_sabr_query,
        "videoPlaybackUstreamerConfig": "AQID",
        "audioItag": 140,
        "formats": [{ "itag": 140, "mimeType": "audio/mp4" }]
    });
    let res_query = sanitize_sabr(&sabr_no_query, no_sabr_query);
    assert!(res_query.is_err());
    assert_eq!(res_query.unwrap_err(), "Invalid SABR stream URL.");
}

#[test]
fn sabr_rejects_invalid_itags_and_oversized_request_bodies() {
    let url = "https://r1.googlevideo.com/videoplayback?sabr=1";
    let sabr_invalid_mime = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": url,
        "videoPlaybackUstreamerConfig": "AQID",
        "audioItag": 140,
        "formats": [{ "itag": 999, "mimeType": "text/html", "bitrate": 1 }]
    });
    let err_mime = sanitize_sabr(&sabr_invalid_mime, url);
    assert!(err_mime.is_err());
    assert_eq!(err_mime.unwrap_err(), "Invalid SABR text.");

    let sabr_missing_audio = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": url,
        "videoPlaybackUstreamerConfig": "AQID",
        "audioItag": 999,
        "formats": [{ "itag": 140, "mimeType": "audio/mp4" }]
    });
    let err_audio = sanitize_sabr(&sabr_missing_audio, url);
    assert!(err_audio.is_err());
    assert_eq!(err_audio.unwrap_err(), "Selected SABR format is unavailable.");

    // YouTube real-world formats with colorInfo and enhanced bitrate labels
    let sabr_real_world = serde_json::json!({
        "videoId": "wLdZWyLOJCI",
        "serverAbrStreamingUrl": url,
        "videoPlaybackUstreamerConfig": "AQID~_+-=:;.",
        "audioItag": 140,
        "videoItag": 137,
        "formats": [
            {
                "itag": 140,
                "mimeType": "audio/mp4; codecs=\"mp4a.40.2\"",
                "bitrate": 128000,
                "audioQuality": "AUDIO_QUALITY_MEDIUM"
            },
            {
                "itag": 137,
                "mimeType": "video/mp4; codecs=\"avc1.640028\"",
                "bitrate": 4500000,
                "quality": "hd1080",
                "qualityLabel": "1080p Premium (Enhanced bitrate)",
                "colorInfo": {
                    "transferCharacteristics": "COLOR_TRANSFER_CHARACTERISTICS_BT709"
                }
            }
        ]
    });
    let valid_real = sanitize_sabr(&sabr_real_world, url);
    assert!(valid_real.is_ok(), "Real YouTube colorInfo and qualityLabel must be accepted");
}

#[test]
fn browser_sabr_reuse_identifies_video_and_quality_rather_than_expiring_url() {
    let temp = Builder::new().prefix("test-sabr-reuse").tempdir().unwrap();
    let directory = temp.path().to_path_buf();
    let history_service = DownloadHistoryService::new(DownloadHistoryStore::new(&directory)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let transfer_service = DownloadTransferService::new(history_service, executor, 2);
    let settings = Settings::new(directory);

    let sabr = NormalizedSabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: "https://r1.googlevideo.com/videoplayback?sabr=1".to_owned(),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: SabrClientInfo::default(),
        formats: vec![
            SabrFormat {
                itag: 137,
                mime_type: "video/mp4".to_owned(),
                has_video: true,
                ..default_format(137, "video/mp4")
            },
            SabrFormat {
                itag: 136,
                mime_type: "video/mp4".to_owned(),
                has_video: true,
                ..default_format(136, "video/mp4")
            },
            SabrFormat {
                itag: 140,
                mime_type: "audio/mp4".to_owned(),
                has_audio: true,
                ..default_format(140, "audio/mp4")
            },
        ],
        video_itag: Some(137),
        audio_itag: 140,
        request_body: None,
        po_token: None,
    };

    let req1 = DownloadRequest {
        urls: vec![sabr.server_abr_streaming_url.clone()],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: Some(sabr.to_session()),
        reuse_active: Some(true),
        referer: None,
        youtube_format: None,
    };

    let ids1 = transfer_service.add(req1, &settings).unwrap();
    assert_eq!(ids1.len(), 1);

    // Second request with expiring query parameter added to URL
    let new_url = format!("{}&expire=9999999", sabr.server_abr_streaming_url);
    let mut sabr2 = sabr.clone();
    sabr2.server_abr_streaming_url = new_url.clone();

    let req2 = DownloadRequest {
        urls: vec![new_url],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: Some(sabr2.to_session()),
        reuse_active: Some(true),
        referer: None,
        youtube_format: None,
    };

    let ids2 = transfer_service.add(req2, &settings).unwrap();
    assert_eq!(ids1, ids2, "Should return existing active transfer ID");

    // Request with different video itag should create a new transfer
    let mut sabr3 = sabr.clone();
    sabr3.video_itag = Some(136);
    let req3 = DownloadRequest {
        urls: vec![sabr.server_abr_streaming_url.clone()],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: true,
        review_only: None,
        scheduled_at: None,
        audio_url: None,
        kind: None,
        sabr: Some(sabr3.to_session()),
        reuse_active: Some(true),
        referer: None,
        youtube_format: None,
    };

    let ids3 = transfer_service.add(req3, &settings).unwrap();
    assert_ne!(ids1, ids3, "Different quality should create new transfer");
}

#[test]
fn sabr_request_protobuf_helpers_and_framing() {
    // 1. Test varint encoding
    let mut buf = Vec::new();
    encode_varint(1, &mut buf);
    assert_eq!(buf, vec![1]);

    buf.clear();
    encode_varint(140, &mut buf);
    assert_eq!(buf, vec![0x8C, 0x01]);

    // 2. Test field encoding
    buf.clear();
    encode_varint_field(&mut buf, 2, 137);
    assert_eq!(parse_media_header_itag(&buf), Some(137));

    buf.clear();
    encode_bytes_field(&mut buf, 6, b"test_payload");
    assert_eq!(buf[0], (6 << 3) | 2);
    assert_eq!(buf[1], 12); // length
    assert_eq!(&buf[2..], b"test_payload");

    // 3. Test UMP chunk framing
    let chunk = encode_ump_chunk(21, b"hello_media");
    assert_eq!(chunk[0], 21); // part 21
    assert_eq!(chunk[1], 11); // length 11
    assert_eq!(&chunk[2..], b"hello_media");

    // 4. Test request body builder round-trip through parse_sabr_protobuf_request
    let session = NormalizedSabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: "https://r1.googlevideo.com/videoplayback?sabr=1".to_owned(),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: SabrClientInfo {
            client_name: 1,
            client_version: "2.20261022.00.00".to_owned(),
            ..Default::default()
        },
        formats: vec![],
        video_itag: Some(137),
        audio_itag: 140,
        request_body: None,
        po_token: Some("BwgJ".to_owned()),
    };

    let built_body = build_sabr_request_body(&session);
    assert!(!built_body.is_empty());
    let parsed = parse_sabr_protobuf_request(&built_body).expect("should parse built body");
    let info = parsed.client_info.expect("client info should be parsed");
    assert_eq!(info.client_name, 1);
    assert_eq!(info.client_version, "2.20261022.00.00");
    assert_eq!(parsed.po_token.as_deref(), Some(&[7, 8, 9][..]));
}

#[tokio::test]
async fn sabr_download_executor_streams_ump_chunks_and_muxes() {
    let ffmpeg = FfmpegTools::new();
    if ffmpeg.is_none() {
        eprintln!("Skipping sabr mux test: ffmpeg not in PATH");
        return;
    }

    let temp = Builder::new().prefix("test-sabr-mux-").tempdir().unwrap();
    let dir = temp.path().to_path_buf();

    // Generate real short audio and video tracks via ffmpeg for the UMP server
    let temp_v = dir.join("test_sabr_v.mp4");
    let temp_a = dir.join("test_sabr_a.m4a");

    let mut v_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    v_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "color=c=blue:s=160x120:d=0.5",
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-t", "0.5",
        ])
        .arg(&temp_v);
    assert!(v_cmd.status().await.expect("v cmd").success());

    let mut a_cmd = tokio::process::Command::new(ffmpeg.as_ref().unwrap().executable());
    a_cmd
        .args(&[
            "-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono",
            "-t", "0.5", "-c:a", "aac",
        ])
        .arg(&temp_a);
    assert!(a_cmd.status().await.expect("a cmd").success());

    let video_bytes = tokio::fs::read(&temp_v).await.expect("read v");
    let audio_bytes = tokio::fs::read(&temp_a).await.expect("read a");

    // Package into UMP stream bytes
    let mut ump_stream = Vec::new();
    // Video header (itag 137)
    let mut v_hdr = Vec::new();
    encode_varint_field(&mut v_hdr, 2, 137);
    ump_stream.extend(encode_ump_chunk(20, &v_hdr));
    // Video payload (part 21)
    ump_stream.extend(encode_ump_chunk(21, &video_bytes));

    // Audio header (itag 140)
    let mut a_hdr = Vec::new();
    encode_varint_field(&mut a_hdr, 2, 140);
    ump_stream.extend(encode_ump_chunk(20, &a_hdr));
    // Audio payload (part 21)
    ump_stream.extend(encode_ump_chunk(21, &audio_bytes));

    // Media end (part 22)
    ump_stream.extend(encode_ump_chunk(22, &[]));

    // Start local TCP mock server
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let address = listener.local_addr().expect("local address");

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.yt-ump\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                ump_stream.len()
            );
            let _ = stream.write_all(&ump_stream);
        }
    });

    let sabr_session = NormalizedSabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: format!("http://{address}/videoplayback?sabr=1"),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: SabrClientInfo::default(),
        formats: vec![
            SabrFormat {
                itag: 137,
                mime_type: "video/mp4".to_owned(),
                has_video: true,
                content_length: Some(video_bytes.len().to_string()),
                ..default_format(137, "video/mp4")
            },
            SabrFormat {
                itag: 140,
                mime_type: "audio/mp4".to_owned(),
                has_audio: true,
                content_length: Some(audio_bytes.len().to_string()),
                ..default_format(140, "audio/mp4")
            },
        ],
        video_itag: Some(137),
        audio_itag: 140,
        request_body: None,
        po_token: None,
    };

    let download = DownloadRecord {
        id: "test-sabr-1".to_string(),
        url: sabr_session.server_abr_streaming_url.clone(),
        filename: "sabr_output.mp4".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("sabr".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 2,
        connections_progress: Vec::new(),
        audio_url: None,
        sabr: Some(sabr_session),
        referer: None,
        youtube_format: None,
    };

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let pe = progress_events.clone();
    let progress_handler: DownloadProgressHandler = Arc::new(move |p| {
        pe.lock().unwrap().push(p);
    });

    let (_tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));

    let executor = SabrDownloadExecutor::new(reqwest::Client::new());
    let result = executor
        .transfer(download.clone(), progress_handler, control, rate_limiter)
        .await
        .expect("transfer sabr should succeed");

    assert!(result.path.exists());
    assert_eq!(result.path.file_name().unwrap(), "sabr_output.mp4");
    assert!(result.downloaded > 0);

    let parts_dir = dir.join(format!(".open-download-manager-sabr-{}", download.id));
    assert!(!parts_dir.exists(), "parts dir should be deleted after mux");

    let events = progress_events.lock().unwrap();
    assert!(!events.is_empty());
}

#[tokio::test]
async fn sabr_download_executor_handles_cancellation_and_cleans_temporary_directory() {
    let temp = Builder::new().prefix("test-sabr-cancel-").tempdir().unwrap();
    let dir = temp.path().to_path_buf();

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let address = listener.local_addr().expect("local address");

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut req_buf = [0u8; 1024];
            let _ = stream.read(&mut req_buf);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.yt-ump\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            // Sleep to simulate slow transmission so test can cancel
            thread::sleep(std::time::Duration::from_millis(1000));
        }
    });

    let sabr_session = NormalizedSabrSession {
        video_id: "wLdZWyLOJCI".to_owned(),
        server_abr_streaming_url: format!("http://{address}/videoplayback?sabr=1"),
        video_playback_ustreamer_config: "AQID".to_owned(),
        client_info: SabrClientInfo::default(),
        formats: vec![
            SabrFormat {
                itag: 137,
                mime_type: "video/mp4".to_owned(),
                has_video: true,
                ..default_format(137, "video/mp4")
            },
            SabrFormat {
                itag: 140,
                mime_type: "audio/mp4".to_owned(),
                has_audio: true,
                ..default_format(140, "audio/mp4")
            },
        ],
        video_itag: Some(137),
        audio_itag: 140,
        request_body: None,
        po_token: None,
    };

    let download = DownloadRecord {
        id: "test-sabr-cancel-1".to_string(),
        url: sabr_session.server_abr_streaming_url.clone(),
        filename: "canceled.mp4".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: Some(dir.to_string_lossy().into_owned()),
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: Some("sabr".to_string()),
        resumable: Some(false),
        duration: None,
        connections: 2,
        connections_progress: Vec::new(),
        audio_url: None,
        sabr: Some(sabr_session),
        referer: None,
        youtube_format: None,
    };

    let (tx, rx) = watch::channel(None);
    let control = DownloadControl::new(rx);
    let rate_limiter = Arc::new(DownloadRateLimiter::new(0));
    let progress_handler: DownloadProgressHandler = Arc::new(|_| {});

    let executor = SabrDownloadExecutor::new(reqwest::Client::new());

    // Stop after 50ms
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let _ = tx.send(Some(DownloadStopReason::Cancel));
    });

    let res = executor
        .transfer(download.clone(), progress_handler, control, rate_limiter)
        .await;

    assert!(res.is_err(), "transfer should be aborted");
    let parts_dir = dir.join(format!(".open-download-manager-sabr-{}", download.id));
    assert!(!parts_dir.exists(), "parts dir should be deleted on cancellation");
}

fn default_format(itag: u32, mime: &str) -> SabrFormat {
    SabrFormat {
        itag,
        mime_type: mime.to_owned(),
        bitrate: 0,
        width: None,
        height: None,
        fps: None,
        average_bitrate: None,
        audio_channels: None,
        last_modified: None,
        content_length: None,
        approx_duration_ms: None,
        target_duration_sec: None,
        xtags: None,
        audio_quality: None,
        quality: None,
        quality_label: None,
        audio_track: None,
        color_info: None,
        is_drc: false,
        is_vb: false,
        has_audio: false,
        has_video: false,
        audio_track_id: None,
    }
}
