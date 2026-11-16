// Created: 2026-11-05
// Reason: feat: migrate history and adopt Bun
// Last updated: 2026-11-16
// Reason: fix(windows): route by window label, remove ?query URLs, verify ready fallback
// Description: Integration tests verifying window manager functionality, edge cases, and error recovery.

use open_download_manager_lib::{
    application::torrent_service::parse_magnet_link,
    domain::download::DownloadStatus,
    presentation::window_manager::{
        add_route, integration_route, needs_close_choice, options_route, progress_label,
        progress_route, scheduler_route, should_cancel_review_on_close, torrent_route,
        OpenAddOptions, ADD_WINDOW_HEIGHT, ADD_WINDOW_MIN_HEIGHT, ADD_WINDOW_MIN_WIDTH,
        ADD_WINDOW_WIDTH, COMPLETE_WINDOW_HEIGHT, COMPLETE_WINDOW_MIN_HEIGHT,
        COMPLETE_WINDOW_MIN_WIDTH, COMPLETE_WINDOW_WIDTH, INTEGRATION_WINDOW_HEIGHT,
        INTEGRATION_WINDOW_MIN_HEIGHT, INTEGRATION_WINDOW_MIN_WIDTH, INTEGRATION_WINDOW_WIDTH,
        OPTIONS_WINDOW_HEIGHT, OPTIONS_WINDOW_MIN_HEIGHT, OPTIONS_WINDOW_MIN_WIDTH,
        OPTIONS_WINDOW_WIDTH, PROGRESS_WINDOW_COLLAPSED_HEIGHT, PROGRESS_WINDOW_EXPANDED_HEIGHT,
        PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT, PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT,
        PROGRESS_WINDOW_MIN_WIDTH, PROGRESS_WINDOW_WIDTH, REVIEW_WINDOW_HEIGHT,
        REVIEW_WINDOW_MIN_HEIGHT, SCHEDULER_WINDOW_HEIGHT, SCHEDULER_WINDOW_MIN_HEIGHT,
        SCHEDULER_WINDOW_MIN_WIDTH, SCHEDULER_WINDOW_WIDTH, TORRENT_WINDOW_HEIGHT,
        TORRENT_WINDOW_MIN_HEIGHT, TORRENT_WINDOW_MIN_WIDTH, TORRENT_WINDOW_WIDTH,
    },
};

#[test]
fn progress_route_validates_download_id_and_returns_index_html() {
    assert_eq!(
        progress_route("download-123_ab").unwrap(),
        "index.html"
    );
}

#[test]
fn progress_route_rejects_empty_or_unsafe_download_ids() {
    assert!(progress_route("").is_err());
    assert!(progress_route("../settings").is_err());
    assert!(progress_route("download?id=1").is_err());
}

#[test]
fn progress_route_rejects_overlong_download_ids() {
    let id = "a".repeat(129);
    assert!(progress_route(&id).is_err());
}

#[test]
fn dynamic_progress_window_labels_prevent_overwriting() {
    let label1 = progress_label("dl-alpha");
    let label2 = progress_label("dl-beta");
    assert_eq!(label1, "progress-dl-alpha");
    assert_eq!(label2, "progress-dl-beta");
    assert_ne!(label1, label2);
}

#[test]
fn auxiliary_window_routes_return_plain_index_html_without_query_strings() {
    // All aux windows use plain index.html to prevent blank window issues on Windows WebView2
    assert_eq!(options_route(), "index.html");
    assert_eq!(scheduler_route(), "index.html");
    assert_eq!(integration_route(), "index.html");
    assert_eq!(torrent_route(), "index.html");
    assert_eq!(add_route(None), "index.html");
    assert_eq!(
        add_route(Some(&OpenAddOptions {
            paste: Some(true),
            scheduled: None,
        })),
        "index.html"
    );

    // Verify dimension constants
    assert_eq!(OPTIONS_WINDOW_WIDTH, 540.0);
    assert_eq!(OPTIONS_WINDOW_HEIGHT, 650.0);
    assert_eq!(OPTIONS_WINDOW_MIN_WIDTH, 500.0);
    assert_eq!(OPTIONS_WINDOW_MIN_HEIGHT, 500.0);

    assert_eq!(SCHEDULER_WINDOW_WIDTH, 550.0);
    assert_eq!(SCHEDULER_WINDOW_HEIGHT, 430.0);
    assert_eq!(SCHEDULER_WINDOW_MIN_WIDTH, 500.0);
    assert_eq!(SCHEDULER_WINDOW_MIN_HEIGHT, 360.0);

    assert_eq!(INTEGRATION_WINDOW_WIDTH, 700.0);
    assert_eq!(INTEGRATION_WINDOW_HEIGHT, 620.0);
    assert_eq!(INTEGRATION_WINDOW_MIN_WIDTH, 620.0);
    assert_eq!(INTEGRATION_WINDOW_MIN_HEIGHT, 480.0);

    assert_eq!(TORRENT_WINDOW_WIDTH, 900.0);
    assert_eq!(TORRENT_WINDOW_HEIGHT, 700.0);
    assert_eq!(TORRENT_WINDOW_MIN_WIDTH, 700.0);
    assert_eq!(TORRENT_WINDOW_MIN_HEIGHT, 500.0);

    assert_eq!(ADD_WINDOW_WIDTH, 565.0);
    assert_eq!(ADD_WINDOW_HEIGHT, 370.0);
    assert_eq!(ADD_WINDOW_MIN_WIDTH, 520.0);
    assert_eq!(ADD_WINDOW_MIN_HEIGHT, 300.0);

    assert_eq!(PROGRESS_WINDOW_WIDTH, 565.0);
    assert_eq!(PROGRESS_WINDOW_EXPANDED_HEIGHT, 485.0);
    assert_eq!(PROGRESS_WINDOW_COLLAPSED_HEIGHT, 340.0);
    assert_eq!(PROGRESS_WINDOW_MIN_WIDTH, 540.0);
    assert_eq!(PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT, 440.0);
    assert_eq!(PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT, 325.0);

    assert_eq!(REVIEW_WINDOW_HEIGHT, 370.0);
    assert_eq!(REVIEW_WINDOW_MIN_HEIGHT, 300.0);
    assert_eq!(COMPLETE_WINDOW_WIDTH, 480.0);
    assert_eq!(COMPLETE_WINDOW_HEIGHT, 230.0);
    assert_eq!(COMPLETE_WINDOW_MIN_WIDTH, 460.0);
    assert_eq!(COMPLETE_WINDOW_MIN_HEIGHT, 190.0);
}

#[test]
fn close_confirmation_decision_logic_matches_legacy_specification() {
    assert!(needs_close_choice(&DownloadStatus::Downloading, false));
    assert!(needs_close_choice(&DownloadStatus::Queued, false));
    assert!(needs_close_choice(&DownloadStatus::Connecting, false));
    assert!(needs_close_choice(&DownloadStatus::Paused, false));
    assert!(needs_close_choice(&DownloadStatus::Assembling, false));

    assert!(!needs_close_choice(&DownloadStatus::Completed, false));
    assert!(!needs_close_choice(&DownloadStatus::Cancelled, false));

    assert!(!needs_close_choice(&DownloadStatus::Paused, true));
    assert!(should_cancel_review_on_close(true));
    assert!(!should_cancel_review_on_close(false));
}

#[tokio::test]
async fn test_ui_ready_fallback_timeout_fires_safely() {
    let label = "test_window_fallback";
    open_download_manager_lib::infrastructure::perf::record_window_open(label);

    let start = std::time::Instant::now();
    // Test wait_for_window_ui_ready with a 100ms fallback timeout
    let res = open_download_manager_lib::infrastructure::perf::wait_for_window_ui_ready(
        label,
        std::time::Duration::from_millis(100),
    )
    .await;

    assert!(res.is_err(), "Should time out when ui_ready signal is never sent");
    assert!(
        start.elapsed() >= std::time::Duration::from_millis(90),
        "Must wait for the full timeout duration before falling back"
    );
}

#[tokio::test]
async fn test_ui_ready_signal_notifies_listener_immediately() {
    let label = "test_window_ready_signal";
    open_download_manager_lib::infrastructure::perf::record_window_open(label);

    let label_str = label.to_string();
    let waiter = tokio::spawn(async move {
        open_download_manager_lib::infrastructure::perf::wait_for_window_ui_ready(
            &label_str,
            std::time::Duration::from_millis(1500),
        )
        .await
    });

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    open_download_manager_lib::infrastructure::perf::mark_window_ui_ready(label);

    let res = waiter.await.expect("task join");
    assert!(res.is_ok(), "Waiter should successfully receive ui_ready duration");
}

#[test]
fn test_parse_magnet_link_synchronous_and_instant() {
    let magnet = "magnet:?xt=urn:btih:2b6c31677c447e5b9d3b84ef6f530e1590fb3b34&dn=ubuntu-22.04.5-desktop-amd64.iso";
    let start = std::time::Instant::now();
    let (hash, name) = parse_magnet_link(magnet).expect("valid magnet parse");
    let elapsed = start.elapsed();

    assert_eq!(hash, "2b6c31677c447e5b9d3b84ef6f530e1590fb3b34");
    assert_eq!(name, "ubuntu-22.04.5-desktop-amd64.iso");
    assert!(
        elapsed.as_millis() < 5,
        "Magnet parse must complete synchronously in < 5ms (took {:?})",
        elapsed
    );

    // Invalid magnets
    assert!(parse_magnet_link("http://example.com/test").is_err());
    assert!(parse_magnet_link("magnet:?dn=test_no_hash").is_err());
}
