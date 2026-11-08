// Created: 2026-11-05
// Reason: refactor: modularize native download architecture
// Last updated: 2026-11-05
// Reason: rebrand: migrate project to Open Download Manager
// Description: Integration tests verifying download category functionality, edge cases, and error recovery.

use std::collections::BTreeMap;

use open_download_manager_lib::domain::{category::DownloadCategory, settings::Settings};

#[test]
fn categories_serialize_with_the_renderer_contract_names() {
    let settings = Settings {
        category_directories: BTreeMap::from([(
            DownloadCategory::Video,
            std::path::PathBuf::from("/downloads/video"),
        )]),
        ..Settings::new(std::path::PathBuf::from("/downloads"))
    };

    let serialized = serde_json::to_value(settings).expect("settings should serialize");
    assert_eq!(
        serialized["categoryDirectories"]["Video"],
        "/downloads/video"
    );
}

#[test]
fn unsupported_categories_are_rejected_during_deserialization() {
    let result = serde_json::from_str::<DownloadCategory>("\"Games\"");
    assert!(result.is_err());
}
