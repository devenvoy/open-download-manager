// Created: 2026-11-12
// Reason: fix(torrent): stop other apps from stealing magnet and .torrent opens
// Description: Integration tests for the OS integration of the torrent
// subsystem: recognising magnet links and .torrent launch arguments, and the
// guard that keeps a request delivered through several OS channels from
// adding the same torrent twice.

use open_download_manager_lib::application::torrent_os_integration::{
    extract_torrent_launch_args, is_torrent_launch_arg,
};

#[test]
fn magnet_links_and_torrent_files_are_recognised_as_launch_arguments() {
    assert!(is_torrent_launch_arg(
        "magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862&dn=ubuntu.iso"
    ));
    assert!(is_torrent_launch_arg("/Users/dev/Downloads/ubuntu.iso.torrent"));
    assert!(is_torrent_launch_arg("/Users/dev/Downloads/UBUNTU.TORRENT"));
    assert!(!is_torrent_launch_arg("https://example.com/file.torrent"));
    assert!(!is_torrent_launch_arg("/Users/dev/Downloads/ubuntu.iso"));
    assert!(!is_torrent_launch_arg(""));
}

#[test]
fn launch_arguments_skip_the_executable_and_keep_only_torrent_requests() {
    let args = vec![
        "/Applications/Open Download Manager.app/Contents/MacOS/open-download-manager".to_owned(),
        "magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862".to_owned(),
        "--background".to_owned(),
        "/Users/dev/Downloads/ubuntu.iso.torrent".to_owned(),
    ];

    let extracted = extract_torrent_launch_args(&args);

    assert_eq!(
        extracted,
        vec![
            "magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862".to_owned(),
            "/Users/dev/Downloads/ubuntu.iso.torrent".to_owned(),
        ]
    );
}

#[test]
fn launch_arguments_without_a_torrent_request_are_ignored() {
    let args = vec![
        "/Applications/Open Download Manager.app/Contents/MacOS/open-download-manager".to_owned(),
        "--background".to_owned(),
    ];

    assert!(extract_torrent_launch_args(&args).is_empty());
    assert!(extract_torrent_launch_args(&[]).is_empty());
}

#[cfg(feature = "torrent")]
#[test]
fn the_same_request_is_only_handled_once() {
    use open_download_manager_lib::application::torrent_os_integration::claim_handle;

    // Unique keys keep the test independent from the other tests running in
    // parallel against the same shared guard.
    let key = "magnet:?xt=urn:btih:0000000000000000000000000000000000000001";

    assert!(claim_handle(key), "the first request must be handled");
    assert!(
        !claim_handle(key),
        "a repeat of the same request must be ignored"
    );
    assert!(
        claim_handle("magnet:?xt=urn:btih:0000000000000000000000000000000000000002"),
        "a different request must still be handled"
    );
}
