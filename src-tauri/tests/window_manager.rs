use awesome_downloader_lib::presentation::window_manager::progress_route;

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
