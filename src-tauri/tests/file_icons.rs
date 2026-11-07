use std::{
    collections::BTreeMap,
    path::PathBuf,
};

use open_download_manager_lib::{
    domain::{download::DownloadRecord, download::DownloadStatus, settings::Settings, snapshot::Snapshot},
    infrastructure::file_icons::{extension_key, is_safe_extension, FileIconProvider},
};

#[test]
fn file_icon_keys_are_safe_lowercase_extensions() {
    assert_eq!(extension_key("Movie.MP4"), ".mp4");
    assert_eq!(extension_key("C:\\Downloads\\picture.JPEG"), ".jpeg");
    assert_eq!(extension_key("/home/user/downloads/archive.tar.gz"), ".gz");
    assert_eq!(extension_key("no-extension"), "");
    assert_eq!(extension_key("unsafe.very-long-extension"), "");
    assert_eq!(extension_key("invalid..ext"), ".ext");
    assert_eq!(extension_key("symbols.ext!"), "");

    assert!(is_safe_extension(".mp4"));
    assert!(is_safe_extension(".jpeg"));
    assert!(is_safe_extension(".7z"));
    assert!(!is_safe_extension("mp4")); // missing leading dot
    assert!(!is_safe_extension(".")); // empty extension
    assert!(!is_safe_extension(".toolongextensionname")); // > 12 chars
}

#[test]
fn file_icon_provider_loads_and_caches_probe_files() {
    let temp = tempfile::tempdir().unwrap();
    let cache_dir = temp.path().join("FileTypeIcons");
    let provider = FileIconProvider::new(cache_dir.clone());

    let downloads = vec![
        DownloadRecord {
            id: "1".to_string(),
            url: "https://example.com/one.mp4".to_string(),
            filename: "one.mp4".to_string(),
            status: DownloadStatus::Queued,
            downloaded: 0,
            total: None,
            speed: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            needs_review: false,
            scheduled_at: None,
            description: None,
            category: None,
            directory: None,
            path: None,
            last_try: None,
            error: None,
            notice: None,
            estimated: false,
            kind: None,
            resumable: None,
            duration: None,
            connections: 1,
            connections_progress: Vec::new(),
            transfer_kind: None,
            audio_url: None,
            sabr: None,
            referer: None,
            youtube_format: None,
            etag: None,
            validator: None,
            conflict_policy: None,
        },
        DownloadRecord {
            id: "2".to_string(),
            url: "https://example.com/two.MP4".to_string(),
            filename: "two.MP4".to_string(),
            status: DownloadStatus::Queued,
            downloaded: 0,
            total: None,
            speed: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            needs_review: false,
            scheduled_at: None,
            description: None,
            category: None,
            directory: None,
            path: None,
            last_try: None,
            error: None,
            notice: None,
            estimated: false,
            kind: None,
            resumable: None,
            duration: None,
            connections: 1,
            connections_progress: Vec::new(),
            transfer_kind: None,
            audio_url: None,
            sabr: None,
            referer: None,
            youtube_format: None,
            etag: None,
            validator: None,
            conflict_policy: None,
        },
        DownloadRecord {
            id: "3".to_string(),
            url: "https://example.com/sound.mp3".to_string(),
            filename: "sound.mp3".to_string(),
            status: DownloadStatus::Queued,
            downloaded: 0,
            total: None,
            speed: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            needs_review: false,
            scheduled_at: None,
            description: None,
            category: None,
            directory: None,
            path: None,
            last_try: None,
            error: None,
            notice: None,
            estimated: false,
            kind: None,
            resumable: None,
            duration: None,
            connections: 1,
            connections_progress: Vec::new(),
            transfer_kind: None,
            audio_url: None,
            sabr: None,
            referer: None,
            youtube_format: None,
            etag: None,
            validator: None,
            conflict_policy: None,
        },
    ];

    let loaded = provider.load(&downloads);
    assert!(loaded, "first load should return true as new extensions were loaded");

    // Probe files must exist on disk in cacheDirectory
    assert!(cache_dir.join("associated-file.mp4").exists());
    assert!(cache_dir.join("associated-file.mp3").exists());

    let snapshot = provider.snapshot();
    let keys: Vec<_> = snapshot.keys().cloned().collect();
    assert_eq!(keys, vec![".mp3", ".mp4"]);
    assert!(snapshot[".mp4"].starts_with("data:image/svg+xml;base64,"));
    assert!(snapshot[".mp3"].starts_with("data:image/svg+xml;base64,"));

    // Second load with another .mp4 should be a no-op (cached)
    let extra = vec![DownloadRecord {
        id: "4".to_string(),
        url: "https://example.com/three.mp4".to_string(),
        filename: "three.mp4".to_string(),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total: None,
        speed: 0,
        created_at: "2026-01-01T00:00:00Z".to_string(),
        needs_review: false,
        scheduled_at: None,
        description: None,
        category: None,
        directory: None,
        path: None,
        last_try: None,
        error: None,
        notice: None,
        estimated: false,
        kind: None,
        resumable: None,
        duration: None,
        connections: 1,
        connections_progress: Vec::new(),
        transfer_kind: None,
        audio_url: None,
        sabr: None,
        referer: None,
        youtube_format: None,
        etag: None,
        validator: None,
        conflict_policy: None,
    }];
    let loaded_again = provider.load(&extra);
    assert!(!loaded_again, "already loaded extension should not reload");
}

#[test]
fn snapshot_file_icons_serializes_as_dictionary_object() {
    let settings = Settings::new(PathBuf::from("/downloads"));
    let mut file_icons = BTreeMap::new();
    file_icons.insert(".mp4".to_string(), "data:image/png;base64,video".to_string());
    file_icons.insert(".zip".to_string(), "data:image/png;base64,archive".to_string());

    let snap = Snapshot::new(settings, Vec::new()).with_file_icons(file_icons);
    let json = serde_json::to_value(&snap).expect("serialize snapshot");

    assert!(json.get("fileIcons").is_some());
    let icons_obj = json.get("fileIcons").unwrap().as_object().expect("object map");
    assert_eq!(icons_obj.get(".mp4").unwrap().as_str().unwrap(), "data:image/png;base64,video");
    assert_eq!(icons_obj.get(".zip").unwrap().as_str().unwrap(), "data:image/png;base64,archive");
}
