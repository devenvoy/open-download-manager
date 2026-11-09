// Created: 2026-11-05
// Reason: feat: scaffold Tauri settings shell
// Last updated: 2026-11-10
// Reason: refactor: modularize monolithic presentation commands into domain submodules
// Description: Tauri IPC command handlers processing frontend requests, decomposed into domain modules.

pub mod browser;
pub mod clipboard;
pub mod credentials;
pub mod downloads;
pub mod media;
pub mod proxies;
pub mod queues;
pub mod settings;

pub use browser::{
    browser_setup_action, disable_advanced_integration, enable_advanced_integration,
    get_advanced_integration_capability, get_integration_info, register_browser,
    repair_advanced_integration,
};
pub use clipboard::dismiss_clipboard_candidate;
pub use credentials::{delete_credential_rule, list_credential_rules, save_credential_rule};
pub use downloads::{
    add_download, add_playlist_downloads, check_destination_conflict, download_action,
    export_downloads_to_file, import_downloads_from_file, remove_completed, retry_download_actions,
    review_download, update_download_completion_actions,
};
pub use media::{
    get_media_tools_status, get_playlist_info, get_video_formats, install_media_tool,
    resolve_asx_playlist,
};
pub use proxies::{apply_detected_proxy, detect_proxy, test_proxy_connection};
pub use queues::{
    assign_download_to_queue, create_queue, delete_queue, list_queues, pause_queue,
    reorder_queue_items, start_queue, update_queue,
};
pub use settings::{
    check_for_updates, show_about_dialog, snapshot, update_settings, validate_directory,
};
