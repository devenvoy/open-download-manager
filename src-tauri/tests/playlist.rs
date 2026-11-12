// Created: 2026-11-05
// Reason: feat(playlist): implement YouTube playlist discovery, model, and batch queue controls (#21)
// Last updated: 2026-11-08
// Reason: feat(credentials): add secure site/path password manager for automated basic auth (#30)
// Description: Integration tests verifying playlist functionality, edge cases, and error recovery.

use std::sync::Arc;
use tempfile::tempdir;

use open_download_manager_lib::{
    application::{
        download_history_service::DownloadHistoryService,
        download_transfer_service::DownloadTransferService,
        settings_service::SettingsService,
    },
    domain::{
        category::DownloadCategory,
        download::DownloadStatus,
        media::{
            parse_playlist_json, parse_youtube_playlist_url, AddPlaylistDownloadsRequest,
            PlaylistBatchItemRequest,
        },
    },
    infrastructure::{
        download_history_store::DownloadHistoryStore,
        http_download_executor::HttpDownloadExecutor,
        settings_store::SettingsStore,
    },
};

#[test]
fn test_youtube_playlist_url_parsing() {
    // 1. Pure playlist URL
    let pure = "https://www.youtube.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8";
    let target = parse_youtube_playlist_url(pure).expect("should parse pure playlist URL");
    assert_eq!(target.playlist_id, "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8");
    assert_eq!(
        target.canonical_playlist_url,
        "https://www.youtube.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8"
    );
    assert_eq!(target.video_id, None);
    assert_eq!(target.index, None);

    // 2. Watch URL with list and index
    let watch_with_list = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8&index=7";
    let target = parse_youtube_playlist_url(watch_with_list).expect("should parse watch URL with list");
    assert_eq!(target.playlist_id, "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8");
    assert_eq!(target.video_id, Some("dQw4w9WgXcQ".to_owned()));
    assert_eq!(target.index, Some(7));

    // 3. Shortened youtu.be URL with list
    let youtu_be = "https://youtu.be/dQw4w9WgXcQ?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8";
    let target = parse_youtube_playlist_url(youtu_be).expect("should parse youtu.be URL with list");
    assert_eq!(target.playlist_id, "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8");
    assert_eq!(target.video_id, Some("dQw4w9WgXcQ".to_owned()));

    // 4. Music YouTube playlist
    let music = "https://music.youtube.com/playlist?list=OLAK5uy_k12345678901234567890";
    let target = parse_youtube_playlist_url(music).expect("should parse music.youtube.com URL");
    assert_eq!(target.playlist_id, "OLAK5uy_k12345678901234567890");

    // 5. Mobile YouTube playlist
    let mobile = "https://m.youtube.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8";
    let target = parse_youtube_playlist_url(mobile).expect("should parse m.youtube.com URL");
    assert_eq!(target.playlist_id, "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8");

    // 6. Rejections
    // Invalid host
    assert!(parse_youtube_playlist_url("https://vimeo.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8").is_none());
    // Too short ID (e.g. Watch Later "WL")
    assert!(parse_youtube_playlist_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=WL").is_none());
    // Missing list param
    assert!(parse_youtube_playlist_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").is_none());
    // Invalid characters in list ID
    assert!(parse_youtube_playlist_url("https://www.youtube.com/playlist?list=PL4cUxeGkcC9g<script>").is_none());
    // Credentials in URL
    assert!(parse_youtube_playlist_url("https://user:pass@www.youtube.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8").is_none());
}

#[test]
fn test_playlist_json_parsing_and_filtering() {
    let mock_json = serde_json::json!({
        "_type": "playlist",
        "id": "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8",
        "title": "Rust Programming Tutorial Series",
        "description": "Learn Rust from scratch with clean code.",
        "uploader": "Net Ninja",
        "webpage_url": "https://www.youtube.com/playlist?list=PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8",
        "entries": [
            {
                "id": "11111111111",
                "title": "#1 - Introduction to Rust",
                "duration": 345.5,
                "url": "https://www.youtube.com/watch?v=11111111111",
                "thumbnails": [
                    { "url": "https://i.ytimg.com/vi/11111111111/default.jpg" },
                    { "url": "https://i.ytimg.com/vi/11111111111/hqdefault.jpg" }
                ],
                "channel": "Net Ninja"
            },
            {
                "id": "22222222222",
                "title": "[Private video]",
                "duration": null,
                "url": "https://www.youtube.com/watch?v=22222222222"
            },
            {
                "id": "33333333333",
                "title": "[Deleted video]",
                "duration": null,
                "url": "https://www.youtube.com/watch?v=33333333333"
            },
            {
                "id": "44444444444",
                "title": "#2 - Variables & Mutability",
                "duration": 520.0,
                "url": "44444444444",
                "channel": "Net Ninja"
            }
        ]
    });

    let metadata = parse_playlist_json(&mock_json, "").expect("should parse valid playlist JSON");
    assert_eq!(metadata.id, "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8");
    assert_eq!(metadata.title, "Rust Programming Tutorial Series");
    assert_eq!(metadata.uploader.as_deref(), Some("Net Ninja"));
    assert_eq!(metadata.description.as_deref(), Some("Learn Rust from scratch with clean code."));

    // Exactly 2 valid entries should survive (private & deleted are filtered out)
    assert_eq!(metadata.entries.len(), 2);
    assert_eq!(metadata.entry_count, 2);

    let entry1 = &metadata.entries[0];
    assert_eq!(entry1.id, "11111111111");
    assert_eq!(entry1.title, "#1 - Introduction to Rust");
    assert_eq!(entry1.duration, Some(345.5));
    assert_eq!(entry1.url, "https://www.youtube.com/watch?v=11111111111");
    assert_eq!(entry1.thumbnail.as_deref(), Some("https://i.ytimg.com/vi/11111111111/hqdefault.jpg"));
    assert_eq!(entry1.index, 1);

    let entry2 = &metadata.entries[1];
    assert_eq!(entry2.id, "44444444444");
    assert_eq!(entry2.title, "#2 - Variables & Mutability");
    // Canonicalized url from entry id
    assert_eq!(entry2.url, "https://www.youtube.com/watch?v=44444444444");
    // Fallback thumbnail
    assert_eq!(entry2.thumbnail.as_deref(), Some("https://i.ytimg.com/vi/44444444444/hqdefault.jpg"));
    assert_eq!(entry2.index, 4);

    // Empty entries reject cleanly
    let empty_json = serde_json::json!({
        "id": "PLempty1234567890",
        "title": "Empty Playlist",
        "entries": []
    });
    assert!(parse_playlist_json(&empty_json, "").is_err());
}

#[test]
fn test_add_playlist_downloads_batch() {
    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    let settings_service = SettingsService::new(
        SettingsStore::new(data_dir.clone()),
        data_dir.clone(),
    )
    .unwrap();
    let settings = settings_service.settings().clone();

    let history_service = DownloadHistoryService::new(DownloadHistoryStore::new(&data_dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let transfer_service = DownloadTransferService::new(history_service.clone(), executor, 2);

    let request = AddPlaylistDownloadsRequest {
        playlist_id: "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8".to_owned(),
        playlist_title: "Rust Course".to_owned(),
        items: vec![
            PlaylistBatchItemRequest {
                id: "11111111111".to_owned(),
                url: "https://www.youtube.com/watch?v=11111111111".to_owned(),
                title: "Rust Intro: Setting Up".to_owned(),
                selected: true,
            },
            PlaylistBatchItemRequest {
                id: "22222222222".to_owned(),
                url: "https://www.youtube.com/watch?v=22222222222".to_owned(),
                title: "Rust Skipping This".to_owned(),
                selected: false,
            },
            PlaylistBatchItemRequest {
                id: "33333333333".to_owned(),
                url: "https://www.youtube.com/watch?v=33333333333".to_owned(),
                title: "Rust Ownership & Borrowing".to_owned(),
                selected: true,
            },
        ],
        common_category: Some(DownloadCategory::Video),
        common_directory: Some(data_dir.to_str().unwrap().to_owned()),
    };

    let added_count = transfer_service
        .add_playlist_downloads(request, &settings)
        .expect("should add selected playlist downloads");

    assert_eq!(added_count, 2, "only 2 selected items should be added");

    let downloads = history_service.downloads().expect("should read downloads");
    assert_eq!(downloads.len(), 2);

    // Verify properties of added items
    let item1 = &downloads[0];
    assert_eq!(item1.url, "https://www.youtube.com/watch?v=11111111111");
    assert_eq!(item1.filename, "Rust Intro Setting Up.mp4"); // sanitized filename
    assert_eq!(item1.status, DownloadStatus::Paused);
    assert_eq!(item1.category, Some(DownloadCategory::Video));
    assert_eq!(item1.description.as_deref(), Some("From playlist: Rust Course"));
    assert_eq!(item1.notice.as_deref(), Some("Playlist item 1 of 2."));

    let item2 = &downloads[1];
    assert_eq!(item2.url, "https://www.youtube.com/watch?v=33333333333");
    assert_eq!(item2.filename, "Rust Ownership & Borrowing.mp4");
    assert_eq!(item2.status, DownloadStatus::Paused);
    assert_eq!(item2.notice.as_deref(), Some("Playlist item 2 of 2."));
}

#[tokio::test]
async fn test_playlist_commands_through_app_state() {
    use tauri::Manager;
    use open_download_manager_lib::presentation::commands::add_playlist_downloads;
    use open_download_manager_lib::presentation::app_state::AppState;
    use open_download_manager_lib::application::integration_service::IntegrationService;
    use open_download_manager_lib::infrastructure::file_icons::FileIconProvider;
    use open_download_manager_lib::application::updater_service::UpdateService;

    let temp = tempdir().unwrap();
    let data_dir = temp.path().to_path_buf();

    let settings = SettingsService::new(
        SettingsStore::new(data_dir.clone()),
        data_dir.clone(),
    )
    .unwrap();
    let history = DownloadHistoryService::new(DownloadHistoryStore::new(&data_dir)).unwrap();
    let executor = HttpDownloadExecutor::new().unwrap();
    let transfer = DownloadTransferService::new(history.clone(), executor, 2);
    let integration = IntegrationService::new(data_dir.clone(), data_dir.join("extension"));
    let file_icons = FileIconProvider::new(data_dir.join("FileTypeIcons"));
    let update_client = Arc::new(open_download_manager_lib::infrastructure::updater::GitHubReleaseClient::default_client());
    let updater = UpdateService::new(update_client, "0.1.2", true);

    let cred_store = Arc::new(open_download_manager_lib::infrastructure::credentials::CredentialStore::new(&data_dir));
    let cred_service = Arc::new(open_download_manager_lib::application::credential_service::CredentialService::new(cred_store));
    let app_state = AppState::new(settings, history.clone(), transfer, cred_service, integration, file_icons, updater);

    let app = tauri::test::mock_app();
    app.manage(app_state);
    let state = app.state::<AppState>();

    let request = AddPlaylistDownloadsRequest {
        playlist_id: "PL4cUxeGkcC9gUdxbDCwEIQOfFixLmt5Q8".to_owned(),
        playlist_title: "Command Playlist".to_owned(),
        items: vec![
            PlaylistBatchItemRequest {
                id: "aaaaaaaaaaa".to_owned(),
                url: "https://www.youtube.com/watch?v=aaaaaaaaaaa".to_owned(),
                title: "Command Video 1".to_owned(),
                selected: true,
            },
            PlaylistBatchItemRequest {
                id: "bbbbbbbbbbb".to_owned(),
                url: "https://www.youtube.com/watch?v=bbbbbbbbbbb".to_owned(),
                title: "Command Video 2".to_owned(),
                selected: true,
            },
        ],
        common_category: Some(DownloadCategory::Video),
        common_directory: None,
    };

    let count = add_playlist_downloads(state, request).expect("command should succeed");
    assert_eq!(count, 2);

    let downloads = history.downloads().unwrap();
    assert_eq!(downloads.len(), 2);
    assert_eq!(downloads[0].filename, "Command Video 1.mp4");
    assert_eq!(downloads[1].filename, "Command Video 2.mp4");
}

