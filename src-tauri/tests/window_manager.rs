use awesome_downloader_lib::{
    domain::download::DownloadStatus,
    presentation::window_manager::{
        add_route, integration_route, needs_close_choice, options_route, progress_label,
        progress_route, scheduler_route, should_cancel_review_on_close, OpenAddOptions,
        ADD_WINDOW_HEIGHT, ADD_WINDOW_MIN_HEIGHT, ADD_WINDOW_MIN_WIDTH, ADD_WINDOW_WIDTH,
        COMPLETE_WINDOW_HEIGHT, COMPLETE_WINDOW_MIN_HEIGHT, COMPLETE_WINDOW_MIN_WIDTH,
        COMPLETE_WINDOW_WIDTH, INTEGRATION_WINDOW_HEIGHT, INTEGRATION_WINDOW_MIN_HEIGHT,
        INTEGRATION_WINDOW_MIN_WIDTH, INTEGRATION_WINDOW_WIDTH, OPTIONS_WINDOW_HEIGHT,
        OPTIONS_WINDOW_MIN_HEIGHT, OPTIONS_WINDOW_MIN_WIDTH, OPTIONS_WINDOW_WIDTH,
        SCHEDULER_WINDOW_HEIGHT, SCHEDULER_WINDOW_MIN_HEIGHT, SCHEDULER_WINDOW_MIN_WIDTH,
        SCHEDULER_WINDOW_WIDTH,
        PROGRESS_WINDOW_COLLAPSED_HEIGHT, PROGRESS_WINDOW_EXPANDED_HEIGHT,
        PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT, PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT,
        PROGRESS_WINDOW_MIN_WIDTH, PROGRESS_WINDOW_WIDTH, REVIEW_WINDOW_HEIGHT,
        REVIEW_WINDOW_MIN_HEIGHT,
    },
};

#[test]
fn progress_route_carries_a_valid_download_id() {
    assert_eq!(
        progress_route("download-123_ab").unwrap(),
        "index.html?progress=download-123_ab"
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
fn auxiliary_window_routes_and_exact_dimensions_match_legacy() {
    // Options window
    assert_eq!(options_route(), "index.html?options=1");
    assert_eq!(OPTIONS_WINDOW_WIDTH, 540.0);
    assert_eq!(OPTIONS_WINDOW_HEIGHT, 650.0);
    assert_eq!(OPTIONS_WINDOW_MIN_WIDTH, 500.0);
    assert_eq!(OPTIONS_WINDOW_MIN_HEIGHT, 500.0);

    // Scheduler window
    assert_eq!(scheduler_route(), "index.html?scheduler=1");
    assert_eq!(SCHEDULER_WINDOW_WIDTH, 550.0);
    assert_eq!(SCHEDULER_WINDOW_HEIGHT, 430.0);
    assert_eq!(SCHEDULER_WINDOW_MIN_WIDTH, 500.0);
    assert_eq!(SCHEDULER_WINDOW_MIN_HEIGHT, 360.0);

    // Integration window
    assert_eq!(integration_route(), "index.html?integration=1");
    assert_eq!(INTEGRATION_WINDOW_WIDTH, 700.0);
    assert_eq!(INTEGRATION_WINDOW_HEIGHT, 620.0);
    assert_eq!(INTEGRATION_WINDOW_MIN_WIDTH, 620.0);
    assert_eq!(INTEGRATION_WINDOW_MIN_HEIGHT, 480.0);

    // Add Download window routes & dimensions
    assert_eq!(add_route(None), "index.html?add=1");
    assert_eq!(
        add_route(Some(&OpenAddOptions {
            paste: Some(true),
            scheduled: None,
        })),
        "index.html?add=1&paste=1"
    );
    assert_eq!(
        add_route(Some(&OpenAddOptions {
            paste: Some(false),
            scheduled: Some("2026-11-05T12:00:00Z".to_string()),
        })),
        "index.html?add=1&scheduled=2026-11-05T12%3A00%3A00Z"
    );
    assert_eq!(ADD_WINDOW_WIDTH, 565.0);
    assert_eq!(ADD_WINDOW_HEIGHT, 430.0);
    assert_eq!(ADD_WINDOW_MIN_WIDTH, 520.0);
    assert_eq!(ADD_WINDOW_MIN_HEIGHT, 390.0);

    // Progress window dimension constants
    assert_eq!(PROGRESS_WINDOW_WIDTH, 565.0);
    assert_eq!(PROGRESS_WINDOW_EXPANDED_HEIGHT, 485.0);
    assert_eq!(PROGRESS_WINDOW_COLLAPSED_HEIGHT, 340.0);
    assert_eq!(PROGRESS_WINDOW_MIN_WIDTH, 540.0);
    assert_eq!(PROGRESS_WINDOW_MIN_EXPANDED_HEIGHT, 440.0);
    assert_eq!(PROGRESS_WINDOW_MIN_COLLAPSED_HEIGHT, 325.0);

    // Review & Complete modes
    assert_eq!(REVIEW_WINDOW_HEIGHT, 365.0);
    assert_eq!(REVIEW_WINDOW_MIN_HEIGHT, 330.0);
    assert_eq!(COMPLETE_WINDOW_WIDTH, 480.0);
    assert_eq!(COMPLETE_WINDOW_HEIGHT, 280.0);
    assert_eq!(COMPLETE_WINDOW_MIN_WIDTH, 470.0);
    assert_eq!(COMPLETE_WINDOW_MIN_HEIGHT, 270.0);
}

#[test]
fn close_confirmation_decision_logic_matches_legacy_specification() {
    // Unfinished downloads ask whether to minimize or cancel
    assert!(needs_close_choice(&DownloadStatus::Downloading, false));
    assert!(needs_close_choice(&DownloadStatus::Queued, false));
    assert!(needs_close_choice(&DownloadStatus::Connecting, false));
    assert!(needs_close_choice(&DownloadStatus::Paused, false));
    assert!(needs_close_choice(&DownloadStatus::Assembling, false));

    // Finished or cancelled transfers do not prompt on close
    assert!(!needs_close_choice(&DownloadStatus::Completed, false));
    assert!(!needs_close_choice(&DownloadStatus::Cancelled, false));

    // Downloads pending review do not ask minimize/cancel prompt, they discard directly
    assert!(!needs_close_choice(&DownloadStatus::Paused, true));
    assert!(should_cancel_review_on_close(true));
    assert!(!should_cancel_review_on_close(false));
}
