use std::fs;
use std::path::PathBuf;

use awesome_downloader_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
    },
    domain::{
        download::{DownloadRequest, DownloadStatus},
        settings::Settings,
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
    },
    presentation::menu::{format_export_urls, parse_import_urls, MAX_IMPORT_FILE_SIZE_BYTES},
};

fn test_directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "awesome-downloader-import-export-test-{}-{}-{}",
        std::process::id(),
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).expect("temporary test directory should be created");
    dir
}

fn test_settings(dir: PathBuf) -> Settings {
    Settings::new(dir)
}

#[test]
fn parse_import_urls_valid_content_trims_and_skips_empty_lines() {
    let content = "  https://example.com/file1.zip  \r\n\r\nhttps://example.com/file2.tar.gz\n  \nhttps://example.com/file3.iso  \r\n";
    let file_size = content.len() as u64;

    let urls = parse_import_urls(content, file_size).expect("valid content should parse");
    assert_eq!(
        urls,
        vec![
            "https://example.com/file1.zip",
            "https://example.com/file2.tar.gz",
            "https://example.com/file3.iso",
        ]
    );
}

#[test]
fn parse_import_urls_rejects_files_larger_than_2_mib() {
    let fake_oversized = MAX_IMPORT_FILE_SIZE_BYTES + 1;
    let content = "https://example.com/file.bin\n";

    let err = parse_import_urls(content, fake_oversized).expect_err("oversized file must be rejected");
    assert_eq!(err, "URL list must be smaller than 2 MiB.");
}

#[test]
fn parse_import_urls_rejects_empty_or_whitespace_only_content() {
    let empty_err = parse_import_urls("", 0).expect_err("empty content must be rejected");
    assert_eq!(empty_err, "No valid URLs found in file.");

    let whitespace_err = parse_import_urls("   \r\n   \n   ", 10).expect_err("whitespace must be rejected");
    assert_eq!(whitespace_err, "No valid URLs found in file.");
}

#[test]
fn format_export_urls_joins_with_windows_crlf() {
    let urls = vec![
        "https://example.com/alpha.mp4".to_string(),
        "https://example.com/beta.zip".to_string(),
        "https://example.com/gamma.pdf".to_string(),
    ];

    let exported = format_export_urls(&urls);
    assert_eq!(
        exported,
        "https://example.com/alpha.mp4\r\nhttps://example.com/beta.zip\r\nhttps://example.com/gamma.pdf"
    );
}

#[test]
fn batch_queue_imported_urls_into_transfer_service() {
    let dir = test_directory("import_queue");
    let settings = test_settings(dir.clone());
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&dir))
        .expect("history service should initialize");
    let executor = HttpDownloadExecutor::new().expect("executor should initialize");
    let service = DownloadTransferService::new(history.clone(), executor, 4);

    let content = "https://example.com/1.bin\r\nhttps://example.com/2.bin\r\nhttps://example.com/3.bin";
    let urls = parse_import_urls(content, content.len() as u64).unwrap();

    let request = DownloadRequest {
        urls,
        queue_only: true,
        ..Default::default()
    };

    let ids = service
        .add(request, &settings)
        .expect("imported downloads should be accepted");

    assert_eq!(ids.len(), 3);

    let downloads = history.downloads().expect("downloads should load");
    assert_eq!(downloads.len(), 3);
    for download in &downloads {
        assert_eq!(download.status, DownloadStatus::Queued);
        assert_eq!(download.downloaded, 0);
    }
}
