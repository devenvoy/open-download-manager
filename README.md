# Open Download Manager

A modern, high-performance download manager built with Rust and Tauri, featuring a compact desktop interface and seamless browser video capture. Original open-source implementation inspired by IDM.

**Early preview:** Private/sign-in-required, DRM and live media are unsupported. Please report bugs without sharing session tokens or signed media links.

## Getting Started

Download **Open-Download-Manager-Setup.exe** from the project's
[latest release](https://github.com/devenvoy/open-download-manager/releases/latest),
then see the [installation and usage guide](docs/INSTALL.md). It is a standard
Windows installer with Start-menu and desktop shortcuts.

The browser extension integrates with Chrome, Edge, Brave, Opera, and Firefox.

## Features

- HTTP(S) downloads with multi-threaded segmented ranges where supported.
- Pause/resume for supported transfers, history management, scheduling, and speed limits.
- Chrome, Edge, Brave, Opera, and Firefox media detection and quality selection; selected video/audio tracks are combined locally with FFmpeg.
- YouTube browser-session capture with SABR / adaptive format streaming and paired muxing.
- YouTube playlist detection and batch download queues.
- Download File Info with category assignment, custom save location, Download Later, and Start/Cancel options.
- Compact progress and transfer windows, duplicate active download prevention, and system tray integration with autostart support.

## Develop

To build and run from source, ensure Bun and the latest Rust toolchain are installed:

```bash
bun install
bun run test
bun run dev
```

Run test suite across Rust backend and frontend/extension:
```bash
cargo test --manifest-path src-tauri/Cargo.toml
bun test extension renderer
```

## Contribute

Bug reports, tests, documentation, and pull requests are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

## Privacy and licensing

Media request credentials remain in memory and are never written to download history.
History and browser integration settings are stored locally in
`%USERPROFILE%\.open-download-manager`. The browser-desktop bridge is authenticated and loopback-only.
Do not publish this directory or browser profiles in public issues.

Original project code is [MIT licensed](LICENSE). Third-party components retain
their own licenses, including GPL-covered FFmpeg components; see
[THIRD-PARTY.md](THIRD-PARTY.md).
Only download content you are permitted to save.
