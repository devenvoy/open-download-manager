# Step 0: Inventory of Our Application (Open Download Manager)

Date: 2026-11-16
Workspace: `/Users/devanshpc/Developer/open-download-manager`
Stack: Tauri v2 + Rust Backend (Clean Architecture) + React 19 / TypeScript / Vite / Tailwind v4 / Zustand / TanStack Query

---

## 1. Overview & Architectural Boundaries

Our application follows Clean Architecture with clear layer segregation:
- **Presentation Layer** (`src-tauri/src/presentation/`):
  - IPC command handlers (`src-tauri/src/presentation/commands/`): `downloads.rs`, `queues.rs`, `credentials.rs`, `media.rs`, `torrents.rs`, `clipboard.rs`, `proxies.rs`, `settings.rs`, `browser.rs`.
  - Application state (`src-tauri/src/presentation/app_state.rs`).
  - Event publishers (`src-tauri/src/presentation/download_event_publisher.rs`).
  - Window manager (`src-tauri/src/presentation/window_manager.rs`).
  - Tray & menu (`src-tauri/src/presentation/tray.rs`, `src-tauri/src/presentation/menu.rs`).
- **Application Layer** (`src-tauri/src/application/`):
  - Services: `download_transfer_service/`, `download_history_service.rs`, `settings_service.rs`, `queue_service.rs`, `torrent_service.rs`, `credential_service.rs`, `clipboard_service.rs`, `advanced_integration_service.rs`, `updater_service.rs`, `proxy_discovery_service.rs`, `quota_service.rs`.
- **Domain Layer** (`src-tauri/src/domain/`):
  - Models, policies, validation rules: `download/`, `torrent/`, `settings/`, `browser/`, `media/`, `credentials/`, `queue/`, `completion.rs`, `snapshot/`.
- **Infrastructure Layer** (`src-tauri/src/infrastructure/`):
  - HTTP download execution (`http_download_executor/`: single-stream, segmented, assembler, discovery, retry, validator, auth, client).
  - Loopback HTTP Bridge (`bridge/server.rs`).
  - Browser integration manifests & scanner (`browser/registration.rs`, `browser/scanner/`).
  - BitTorrent engine integration (`torrent/engine.rs`, `torrent/settings_store.rs`).
  - FTP protocol executor (`ftp/`).
  - Media tools discovery & execution (`media/ytdlp.rs`, `media/ffmpeg.rs`, `media/tools/`).
  - System actions, shell & autostart (`action/`, `system_shell.rs`, `autostart.rs`).
  - Data file stores (`download_history_store/`, `settings_store/`, `queue_store.rs`, `credentials/`).
- **Binaries** (`src-tauri/src/bin/`):
  - `open-download-manager` (`src-tauri/src/main.rs`): Main desktop application.
  - `open-download-manager-native-host` (`src-tauri/src/bin/native_host.rs`): Browser native messaging host proxy.
  - `perf_benchmark` (`src-tauri/src/bin/perf_benchmark.rs`): Dev-only serialization benchmark harness.
- **Frontend** (`renderer/`):
  - React 19 + TypeScript + Vite.
  - Features: `downloads/`, `settings/`, `queues/`, `scheduler/`, `integration/`, `torrents/`, `add-download/`.
  - Global query: `ipc.snapshot()` queried every 2000ms (`renderer/src/lib/ipc/queries.ts`).
- **Browser Extension** (`extension/`):
  - Manifest V3 for Chromium (`manifest.json`) and Firefox (`manifest.firefox.json`).
  - Background worker (`background.js`).
  - Injected content scripts (`content.js`, `youtube-*.js`, `chooser-keyboard.js`, `media-choices.js`).

---

## 2. Area-by-Area Inventory

### A. Download Engine
- **Files**:
  - `src-tauri/src/infrastructure/http_download_executor/mod.rs`
  - `src-tauri/src/infrastructure/http_download_executor/segmented/discovery.rs`
  - `src-tauri/src/infrastructure/http_download_executor/segmented/worker.rs`
  - `src-tauri/src/infrastructure/http_download_executor/assembler.rs`
  - `src-tauri/src/infrastructure/http_download_executor/single_stream.rs`
  - `src-tauri/src/domain/settings/connection_override.rs`
  - `src-tauri/src/infrastructure/ftp/`
- **Segmentation & Dynamic Rebalancing**:
  - Spawns $N$ workers (default max connections configured in settings, default 8).
  - Dynamic chunk rebalancing implemented in `worker.rs:73-158`: idle workers steal the upper half of the largest downloading chunk exceeding `MIN_CHUNK_REASSIGN_BYTES` (2 MB).
  - **Assembly bottleneck**: Each worker writes to a separate file on disk (`.open-download-manager-<id>.segment-<idx>.part`). When all segments complete, `assembler::concat_segments` reads each segment file in 64 KB chunks and writes into a concatenated `.part` file, doubling disk write I/O.
- **File Pre-allocation**:
  - None (`set_len` or sparse files are not used). Files grow on demand.
- **Connection Limits**:
  - Static user-defined connection overrides (`ConnectionOverride` in `domain/settings/connection_override.rs`).
  - No adaptive learning or auto back-off on HTTP 429/503.
- **Resume & Crash Recovery**:
  - `download.resume_validator()` in `validator.rs` validates ETag and Last-Modified.
  - Overlap check: None.
  - Inconsistency: If range requests fail or boundaries mismatch, cleans up segment parts and restarts from byte 0.
- **Checksums**:
  - None for HTTP/media downloads. No MD5, SHA1, SHA256, or CRC32 verification.
- **FTP/FTPS**:
  - Hand-rolled plain TCP client (`src-tauri/src/infrastructure/ftp/mod.rs`).
  - Supports passive mode, basic auth, and single stream resume (`REST`).
  - FTPS (TLS) is **unimplemented** despite file header mentioning TLS.
- **Small-file Latency**:
  - Segmented downloads always send an initial `bytes=0-0` range request (`discovery.rs:40`) to inspect capability and metadata before spawning workers. Adds a redundant round-trip for small files.

### B. Browser Integration
- **Files**:
  - `src-tauri/src/bin/native_host.rs`
  - `src-tauri/src/infrastructure/bridge/server.rs`
  - `src-tauri/src/infrastructure/browser/registration.rs`
  - `extension/manifest.json`
  - `extension/background.js`
  - `extension/content.js`
- **Transport & Native Messaging**:
  - `open-download-manager-native-host` reads a single message from `stdin`, connects via HTTP to `127.0.0.1:<port>` with `Authorization: Bearer <token>` (read from `~/.open-download-manager/bridge.json`), writes the HTTP response to `stdout`, and exits.
  - If app is not running, attempts `try_launch_desktop()`, then polls `bridge.json` for up to 7.5 seconds.
- **Giving Download Back to Browser**:
  - Implemented in `extension/background.js:520-566`: pauses download with `api.downloads.pause(item.id)`, attempts handoff. If user cancels or desktop errors, calls `api.downloads.resume(item.id)`. On success, calls `api.downloads.cancel(item.id)` and `api.downloads.erase(item.id)`.
- **Capture Rules**:
  - Evaluated in `domain/browser/capture_policy.rs` and `bridge/server.rs:468`. Excluded host, excluded extension, file size thresholds, confirmation required.
- **Video Button**:
  - Injected via `content.js`, detects HTML5 `<video>`, YouTube players, and HLS `.m3u8` streams.
- **Firefox**:
  - MV3 with background scripts (`manifest.firefox.json`).
- **Unregistration**:
  - Function `unregister_browser_manifest` exists in `registration.rs:320`, but is **not exposed** to any Tauri IPC command or UI setting.

### C. Local API / CLI / MCP
- No CLI commands (only binary flags: `--browser-setup`, `--background`, `--browser-download`, plus magnet link in argv).
- No MCP server.
- No public automation REST or JSON-RPC API (only the internal loopback bridge for extension).

### D. Torrents
- **Files**:
  - `src-tauri/src/infrastructure/torrent/engine.rs`
  - `src-tauri/src/application/torrent_service.rs`
  - `src-tauri/src/domain/torrent/settings.rs`
  - `renderer/src/features/torrents/`
- **Engine**:
  - Uses fork `swarmforge` / `rustTorrent` (`librtbit = { package = "swarmforge", git = "...", rev = "94aae0460fe031af59a532586ccefc10f9a637bd" }`).
  - Configures `listen_addr: [0, 0, 0, 0, 0, 0, 0, 0]:42069` (dual-stack IPv6), `enable_dht: true`, `enable_upnp: true`, `enable_utp: true`.
  - DHT persistence in `dht.json`, session persistence in `session/`.
- **Add-magnet Flow**:
  - `add_magnet_when_ready` calls `api.api_add_torrent` with `magnet_resolution_timeout_secs: Some(45)`.
  - Awaits synchronously for up to 45 seconds while resolving metadata.
  - While resolving, UI shows placeholder `state: "initializing"` without live DHT node counts or connected peer telemetry.
- **Peer & Tracker Management**:
  - Checks `needs_new_peers` and re-announces if connected peers `< 10` and transfer stalled.
  - No peer IP filtering against private LAN / SSRF.

### E. Video Downloads
- **Files**:
  - `src-tauri/src/infrastructure/media/tools/manager.rs`
  - `src-tauri/src/infrastructure/media/tools/downloader.rs`
  - `src-tauri/src/infrastructure/media/ytdlp.rs`
  - `src-tauri/src/infrastructure/media/ffmpeg.rs`
  - `src-tauri/src/infrastructure/media/sabr_executor.rs`
- **Acquisition & Security**:
  - Downloads single-file `yt-dlp` executable directly from GitHub Releases (`yt-dlp.exe` on Windows, `yt-dlp_macos` on macOS).
  - Downloads FFmpeg zip from `gyan.dev` (Windows) or `evermeet.cx` (macOS).
  - **Security Risk**: Checks only `content_length`; does NOT verify SHA256 checksums or GPG signatures.
  - **Cold Start Delay**: Single-file PyInstaller builds extract Python runtime to temp on every execution.

### F. Queues, Scheduler, Speed Limiter, Actions
- **Files**:
  - `src-tauri/src/application/queue_service.rs`
  - `src-tauri/src/application/download_transfer_service/scheduler.rs`
  - `src-tauri/src/application/download_rate_limiter.rs`
  - `src-tauri/src/infrastructure/action/executor.rs`
- **Features**:
  - Named queues, recurring weekly/daily schedules.
  - Token-bucket rate limiter (`DownloadRateLimiter`).
  - Post-download completion actions: Sound playback (`Glass.aiff` on macOS, PowerShell `Media.SoundPlayer` on Windows), Shutdown, Sleep, Hibernate, execute arbitrary process.

### G. UI/UX
- **Files**:
  - `renderer/src/app/App.tsx`
  - `renderer/src/main.tsx`
  - `renderer/src/app/window-mode.ts`
  - `src-tauri/src/presentation/window_manager.rs`
- **Windows**:
  - Main window, progress window (`progress-<id>`), options, add, scheduler, integration, torrents.
  - Aux windows load `index.html` and resolve route using `getCurrentWebviewWindow()?.label`.
  - **Windows Blank Window Issue**: Window created and shown immediately before WebView2 initialization and bundle execution finish.
- **i18n & Accessibility**:
  - English only. No translation pipeline, no RTL support.
  - Basic Radix UI ARIA attributes, but no dedicated screen-reader testing.

### H. Responsiveness & Threading Model
- **Sync IPC & Polling Problem**:
  - Frontend queries `ipc.snapshot()` every 2000 ms (`refetchInterval: 2000`).
  - Backend `snapshot` locks `download_history`, re-extracts base64 file icons, serializes all downloads into a massive JSON payload on every poll.
  - Causes UI stutters when downloading or when list grows.
- **Startup Sequence**:
  - `START_TIME` tracks launch to first window shown (`src-tauri/src/lib.rs:279`).
  - Cold start includes initializing Tokio runtime, reading data directory files, setting up HTTP client, tray, deep-link handler, menu, and LaunchServices associations.

### I. Installer, Updates, Packaging
- **Windows**:
  - Tauri NSIS installer (`src-tauri/tauri.conf.json:92`).
  - Known issue: Default NSIS template triggers an "Uninstall previous version before continuing" page on upgrades.
- **macOS**:
  - DMG bundle (`tauri.conf.json:65-79`).
  - Unsigned/ad-hoc signed builds trigger macOS Gatekeeper warnings.
- **Updates**:
  - Custom `GitHubReleaseClient` polling GitHub releases every 3600 seconds.

### J. Security & Privacy
- Zero telemetry, no remote analytics.
- Loopback bridge bound to `127.0.0.1` with UUIDv4 bearer token.
- `Origin` header rejected; `Host` header validated.
- Filenames sanitized via `sanitize-filename` crate.

### K. Testing & Tooling
- 42 integration test files in `src-tauri/tests/`.
- Vitest unit tests in `renderer/`.
- Bun test suite in `extension/`.
- Serialization benchmark in `src-tauri/src/bin/perf_benchmark.rs`.
- No standalone mock server with fault injection.

---

## 3. Dead Code, Unwired Features, and Gaps

1. **Unwired Manifest Unregistration**: `unregister_browser_manifest` in `registration.rs:320` is fully coded but has no Tauri command and no UI trigger.
2. **FTPS Missing**: `FtpConnection` file header claims TLS negotiation, but implementation is strictly plaintext TCP.
3. **MMS Streaming Protocol**: `src-tauri/src/infrastructure/mms/` is a stub returning explicit unsupported guidance.
4. **Tool Downloader Integrity**: `HttpToolDownloaderClient` verifies only `Content-Length`, omitting SHA256 validation.
5. **Auxiliary Windows Routing**: Aux windows do not pass route parameters in the URL (`route = "index.html"`), depending on asynchronous window label lookup.
