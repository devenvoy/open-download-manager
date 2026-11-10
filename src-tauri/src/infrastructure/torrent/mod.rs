// Created: 2026-11-11
// Reason: feat(torrent): add isolated BitTorrent subsystem
// Description: Torrent infrastructure: settings persistence and the embedded engine adapter.

// The torrent engine (`librtbit` / swarmforge) is not supported on Windows ARM64
// (upstream ships x64 only). Refuse to enable the `torrent` feature for that
// target instead of failing with an opaque link/compile error downstream.
#[cfg(all(feature = "torrent", all(target_os = "windows", target_arch = "aarch64")))]
compile_error!(
    "The `torrent` feature is not supported on Windows ARM64 \
     (aarch64-pc-windows-msvc). Build without `--features torrent`."
);

#[cfg(feature = "torrent")]
pub mod engine;
pub mod settings_store;
