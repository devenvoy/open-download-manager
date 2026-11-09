// Created: 2026-11-05
// Reason: feat: add native HTTP download engine
// Last updated: 2026-11-08
// Reason: feat(settings): expose category folders and temporary directory in options (Closes #32)
// Reason: feat(queues): add named download queues and recurring schedules (Closes #13)
// Description: Integration tests verifying download request functionality, edge cases, and error recovery.

use std::path::PathBuf;

use open_download_manager_lib::domain::download::{
    extension_for_mime_type, filename_from_url, is_generic_filename,
    parse_content_disposition_filename, redact_download_url, resolve_response_filename,
    sanitize_filename, DownloadProtocol, DownloadRequest,
};

fn request(url: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url.to_owned()],
        filename: None,
        directory: None,
        temp_directory: None,
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
    }
}

#[test]
fn accepts_http_and_https_and_ftp_addresses_and_uses_default_directory() {
    for (url, protocol) in [
        ("http://example.test/file.bin", DownloadProtocol::Http),
        ("https://example.test/file.bin", DownloadProtocol::Https),
        ("ftp://example.test/file.bin", DownloadProtocol::Ftp),
        ("ftp://user:secret@example.test/file.bin", DownloadProtocol::Ftp),
    ] {
        let valid = request(url)
            .validate(PathBuf::from("/tmp/downloads"))
            .expect("HTTP, HTTPS, and FTP should be supported");
        assert_eq!(valid.directory, PathBuf::from("/tmp/downloads"));
        assert_eq!(valid.urls.len(), 1);
        assert_eq!(valid.urls[0].protocol(), protocol);
    }
}

#[test]
fn rejects_unsupported_schemes_embedded_credentials_and_media_streams() {
    for url in [
        "file:///etc/passwd",
        "gopher://example.test/file.bin",
        "https://user:password@example.test/file.bin",
        "https://example.test/video.mpd",
    ] {
        assert!(
            request(url)
                .validate(PathBuf::from("/tmp/downloads"))
                .is_err(),
            "{url} should be rejected"
        );
    }
}

#[test]
fn accepts_youtube_addresses() {
    let valid = request("https://www.youtube.com/watch?v=wLdZWyLOJCI")
        .validate(PathBuf::from("/tmp/downloads"))
        .expect("YouTube address should be accepted");
    assert_eq!(valid.kind, Some("youtube".to_owned()));
}

#[test]
fn accepts_hls_stream_addresses() {
    let valid = request("https://example.test/video.m3u8")
        .validate(PathBuf::from("/tmp/downloads"))
        .expect("HLS address should be accepted");
    assert_eq!(valid.urls[0].url().as_str(), "https://example.test/video.m3u8");
}

#[test]
fn rejects_relative_download_directories_and_invalid_or_past_schedules() {
    let mut invalid_directory = request("https://example.test/file.bin");
    invalid_directory.directory = Some("downloads".to_owned());
    assert!(invalid_directory
        .validate(PathBuf::from("/tmp/downloads"))
        .is_err());

    for scheduled_at in [
        "not-a-date".to_owned(),
        (chrono::Utc::now() - chrono::Duration::minutes(1)).to_rfc3339(),
    ] {
        let mut scheduled = request("https://example.test/file.bin");
        scheduled.scheduled_at = Some(scheduled_at);
        assert!(scheduled.validate(PathBuf::from("/tmp/downloads")).is_err());
    }
}

#[test]
fn accepts_a_future_rfc3339_schedule() {
    let scheduled_at = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let mut scheduled = request("https://example.test/file.bin");
    scheduled.scheduled_at = Some(scheduled_at.clone());
    let validated = scheduled
        .validate(PathBuf::from("/tmp/downloads"))
        .expect("future RFC3339 schedule should be accepted");
    assert_eq!(validated.scheduled_at, Some(scheduled_at));
}

#[test]
fn sanitizes_windows_and_path_separators_in_custom_names() {
    assert_eq!(sanitize_filename("../CON.txt"), ".._CON.txt");
    assert_eq!(
        sanitize_filename("folder\\report?.pdf"),
        "folder_report_.pdf"
    );
    assert_eq!(sanitize_filename("..."), "");
}

#[test]
fn redacts_credentials_queries_and_fragments_from_urls() {
    assert_eq!(
        redact_download_url("https://user:password@example.test/file?token=private#section"),
        "https://example.test/file"
    );
    assert_eq!(
        redact_download_url("ftp://user:password@example.test/file.bin"),
        "ftp://******:******@example.test/file.bin"
    );
    assert_eq!(
        redact_download_url("ftp://user:password@example.test:2121/dir/file.bin?q=1#frag"),
        "ftp://******:******@example.test:2121/dir/file.bin"
    );
    assert_eq!(
        redact_download_url("ftp://example.test/file.bin"),
        "ftp://example.test/file.bin"
    );
}

#[test]
fn malformed_url_is_not_exposed_in_a_snapshot() {
    assert_eq!(
        redact_download_url("https://user:password@bad host/file?token=private"),
        "Invalid URL"
    );
}

#[test]
fn generic_filenames_are_identified_and_rejected() {
    assert!(is_generic_filename("watch"));
    assert!(is_generic_filename("video"));
    assert!(is_generic_filename("videoplayback"));
    assert!(is_generic_filename("stream"));
    assert!(is_generic_filename("download"));
    assert!(is_generic_filename("file"));
    assert!(is_generic_filename(""));
    assert!(!is_generic_filename("my_document.pdf"));
    assert!(!is_generic_filename("my_video.mp4"));
    // Real release and movie filenames longer than 32 characters without spaces MUST NOT be generic
    assert!(!is_generic_filename(
        "A.Love.Story.Of.Assassin.2024.1080p.WEB-DL.Hindi.x264-AllMoviesHub.mkv"
    ));
    assert!(!is_generic_filename(
        "ubuntu-24.04.1-desktop-amd64.iso"
    ));
}

#[test]
fn content_disposition_header_parsing_extracts_and_sanitizes() {
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"report_2026.pdf\""),
        Some("report_2026.pdf".to_string())
    );
    assert_eq!(
        parse_content_disposition_filename("attachment; filename*=UTF-8''my%20archive%2Ezip"),
        Some("my archive.zip".to_string())
    );
    // Real movie filename in content-disposition is correctly extracted
    assert_eq!(
        parse_content_disposition_filename(
            "attachment; filename=\"A.Love.Story.Of.Assassin.2024.1080p.WEB-DL.Hindi.x264-AllMoviesHub.mkv\""
        ),
        Some("A.Love.Story.Of.Assassin.2024.1080p.WEB-DL.Hindi.x264-AllMoviesHub.mkv".to_string())
    );
    // Generic names in content-disposition are discarded
    assert_eq!(
        parse_content_disposition_filename("attachment; filename=\"video\""),
        None
    );
}

#[test]
fn mime_type_extension_derivation_and_response_resolution() {
    assert_eq!(extension_for_mime_type("video/mkv"), Some("mkv"));
    assert_eq!(extension_for_mime_type("video/x-matroska"), Some("mkv"));
    assert_eq!(extension_for_mime_type("video/mp4; charset=binary"), Some("mp4"));
    assert_eq!(extension_for_mime_type("application/pdf"), Some("pdf"));
    assert_eq!(extension_for_mime_type("application/octet-stream"), None);

    // Resolves Content-Disposition with full filename
    assert_eq!(
        resolve_response_filename(
            "download_video-downloads_googleusercontent_com",
            Some("attachment; filename=\"A.Love.Story.Of.Assassin.2024.1080p.WEB-DL.Hindi.x264-AllMoviesHub.mkv\""),
            Some("video/mkv"),
        ),
        Some("A.Love.Story.Of.Assassin.2024.1080p.WEB-DL.Hindi.x264-AllMoviesHub.mkv".to_string())
    );

    // Fallback when Content-Disposition has no extension
    assert_eq!(
        resolve_response_filename(
            "download_video-downloads_googleusercontent_com",
            Some("attachment; filename=\"MyMovie\""),
            Some("video/mkv"),
        ),
        Some("MyMovie.mkv".to_string())
    );

    // Fallback when no Content-Disposition is present and initial name has no extension
    assert_eq!(
        resolve_response_filename(
            "download_video-downloads_googleusercontent_com",
            None,
            Some("video/mkv"),
        ),
        Some("download_video-downloads_googleusercontent_com.mkv".to_string())
    );

    // Keeps good existing filename
    assert_eq!(
        resolve_response_filename(
            "document.pdf",
            None,
            Some("application/pdf"),
        ),
        None
    );
}

#[test]
fn filename_from_url_derives_meaningful_names_and_avoids_watch_or_empty() {
    let yt_url = url::Url::parse("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap();
    assert_eq!(filename_from_url(&yt_url), "YouTube Video dQw4w9WgXcQ.mp4");

    let query_url = url::Url::parse("https://example.com/download?filename=archive_v1.zip").unwrap();
    assert_eq!(filename_from_url(&query_url), "archive_v1.zip");

    let generic_url = url::Url::parse("https://files.example.com/download").unwrap();
    assert_eq!(filename_from_url(&generic_url), "download_files_example_com");
}
