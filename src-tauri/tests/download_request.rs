use std::path::PathBuf;

use awesome_downloader_lib::domain::download::{
    redact_download_url, sanitize_filename, DownloadRequest,
};

fn request(url: &str) -> DownloadRequest {
    DownloadRequest {
        urls: vec![url.to_owned()],
        filename: None,
        directory: None,
        category: None,
        description: None,
        queue_only: false,
        scheduled_at: None,
    }
}

#[test]
fn accepts_http_and_https_addresses_and_uses_default_directory() {
    for url in [
        "http://example.test/file.bin",
        "https://example.test/file.bin",
    ] {
        let valid = request(url)
            .validate(PathBuf::from("/tmp/downloads"))
            .expect("HTTP and HTTPS should be supported");
        assert_eq!(valid.directory, PathBuf::from("/tmp/downloads"));
        assert_eq!(valid.urls.len(), 1);
    }
}

#[test]
fn rejects_unsupported_schemes_embedded_credentials_and_media_streams() {
    for url in [
        "file:///etc/passwd",
        "https://user:password@example.test/file.bin",
        "https://example.test/video.m3u8",
        "https://youtube.com/watch?v=private",
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
fn rejects_relative_download_directories_and_scheduled_requests() {
    let mut invalid_directory = request("https://example.test/file.bin");
    invalid_directory.directory = Some("downloads".to_owned());
    assert!(invalid_directory
        .validate(PathBuf::from("/tmp/downloads"))
        .is_err());

    let mut scheduled = request("https://example.test/file.bin");
    scheduled.scheduled_at = Some("2030-01-01T00:00:00.000Z".to_owned());
    assert!(scheduled.validate(PathBuf::from("/tmp/downloads")).is_err());
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
}
