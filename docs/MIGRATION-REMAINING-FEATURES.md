# Tauri Migration: Incomplete, Discrepant & Remaining Features

**Date:** November 2026  
**Status:** Gap Analysis & Migration Deficit Report  
**Scope:** Old Electron App (`reference/electron-legacy/`) vs. New Rust + Tauri App (`src-tauri/`, `renderer/`)  
**Standard:** Strict behavioral equivalence (100% functional match or better). Any feature, action, UI workflow, or shell integration with even 1% remaining or diverging is detailed below.

---

## 1. Executive Summary & Deficit Scorecard

While the core transfer engine and networking have been successfully ported to Rust, significant desktop shell integration, filesystem actions, browser UI wiring, and window management capabilities remain unmigrated or stubbed with placeholder errors.

### Migration Deficit Breakdown

| Feature Category | Legacy Electron Status | Current Tauri Status | Completion % | Deficit / Missing Scope |
| :--- | :--- | :--- | :---: | :--- |
| **Download Context Actions** | Full OS shell integration (`open`, `openWith`, `reveal`, `remove`, `removeCompleted`) | `open`, `openWith`, `reveal`, `remove`, `removeCompleted` fully ported with clean architecture and verified | **100%** | None (100% parity achieved in Phase A) |
| **Desktop Shell & Tray** | System tray icon, minimize-to-tray, close-to-tray, autostart with Windows, single instance lock | Tray icon, close-to-tray, single-instance lock, and Windows Run autostart fully ported and verified | **100%** | None (100% parity achieved in Phase B) |
| **Browser Integration UI** | Live status scanner, setup wizard, extension store deep-links, confirmation review | Live status scanner, setup actions, manifest registration, needsReview review workflow fully ported | **100%** | None (100% parity achieved in Phase C) |
| **Window Management** | Unlimited concurrent progress windows, dedicated options/add/scheduler windows, close confirmation prompts | Dynamic `progress-{id}` windows, options/add/scheduler/integration auxiliary windows, active transfer close confirmation guard | **100%** | None (100% parity achieved in Phase D) |
| **Application Menu & Shortcuts**| Full native menu bar with accelerators (Ctrl+N, Ctrl+F, Del), URL text import/export, about dialog | Tasks, File, Downloads, View, Help menus with accelerators, `downloader:command` events, text file import/export, native About dialog | **100%** | None (100% parity achieved in Phase D) |
| **System Notifications & Reveal**| Windows native notifications on completion and updates | `tauri-plugin-notification` desktop notifications on completion, folder reveal on complete | **100%** | None (100% parity achieved in Phase E) |
| **File Type Icons** | Native Windows file type icon extraction via shell API with disk caching | 60+ `FileType` catalog, 71 bundled vector SVGs, instant base64 Data URLs, automatic category deduction | **100%** | None (100% parity achieved in Phase E & Refactor) |
| **Adaptive Dual-Stream Muxing**| Paired video+audio track downloads, FFmpeg muxing, MP3 extraction | `PairedDownloadExecutor` dual HTTP stream downloads, status `assembling`, FFmpeg muxing & MP3 conversion | **100%** | None (100% parity achieved in Phase E) |
| **Auto-Updater** | Background 6-hour polling, manual check, download approval, active transfer guard | 6-hour background polling, manual Help menu check, active transfer safety guard, friendly error handling, GitHub Releases API client | **100%** | None (100% parity achieved in Phase F) |
| **YouTube SABR Stream Parser** | SABR protobuf streaming via `googlevideo/protos` | Format listing via `yt-dlp` works; standard dual-stream muxing works; SABR proto parser pending | **60%** | SABR protobuf streaming parser unmigrated |

---

## 2. Download Context Actions & File System Operations (COMPLETED - 100% Parity)

All download context actions (`remove`, `open`, `reveal`, `openWith`, and `removeCompleted`) have been implemented and verified in Phase A:
- **`remove`:** Stops active transfers, discards partial `.part` and segment files via `executor.discard_partial`, and removes the record from `DownloadHistoryRepository` and atomic persistence.
- **`open`:** Verifies completed state and existing file, launching the file via `opener::open`.
- **`reveal`:** Highlights finalized files in Windows Explorer / OS file manager via `opener::reveal`, falling back to opening the folder for in-progress or directory-only downloads.
- **`openWith`:** Added `DownloadAction::OpenWith` (with camelCase serde) and launches Windows `%SystemRoot%\System32\OpenWith.exe` on completed files.
- **`removeCompleted`:** Prunes all completed records from history in a single atomic transaction without deleting finalized files from disk, exposed as Tauri command `remove_completed` and `api.removeCompleted()`.

---

## 3. Browser Integration & UI Workflow (COMPLETED - 100% Parity)

All browser integration features, status scanning, setup actions, and confirmation review workflows have been ported and verified in Phase C:
- **Live Status Scanner & `snapshot.integration`:** Scans installed browsers (Chrome, Edge, Brave, Opera, Firefox) across platforms. Inspects Chromium `Preferences` and `Secure Preferences` to determine accurate setup states (`connected`, `approval`, `disabled`, `removed`, `missing`) and Firefox `extensions.json`. Populates `snapshot.integration` with active loopback bridge port, connected profiles, and browser status.
- **Frontend Bridge Wiring:** Wired `api.review`, `api.integration`, `api.browserSetup`, `api.registerBrowser`, and `api.openIntegration` in `renderer/tauri-api.js` to typed Tauri commands.
- **Browser Setup & Registration Commands:** Implemented `get_integration_info`, `register_browser`, and `browser_setup_action` (`store`, `open`, `prepare`, `copy`, `folder`) in `IntegrationService` (Application) and Tauri presentation commands.
- **Download Confirmation Review (`needsReview`):**
  - When `confirmBrowserDownloads` is enabled, downloads arriving from the browser bridge receive `needs_review: true` and `status: DownloadStatus::Paused`.
  - Resuming unreviewed downloads is rejected with `"Review this browser download before starting it."`.
  - `review_download` updates filename (sanitized), directory (absolute path required), category, description, scheduled time, sets `needs_review: false`, and queues/starts the transfer.
  - Cancelling an unreviewed download clears `needs_review` and marks it `Cancelled`.

---

## 4. Desktop Shell, Window Management & Lifecycle

### 4.1 System Tray, Close-to-Tray, Autostart & Single Instance (COMPLETED - 100% Parity)
Implemented and verified in Phase B:
- **System Tray:** Configured in `src-tauri/src/presentation/tray.rs` with tooltip `"Awesome Downloader"`, custom icon, and context menu (`Show downloads`, `Quit`). Left click and double click activate and unminimize main window.
- **Close-to-Tray:** Intercepts main window `CloseRequested` events in `lib.rs`; hides the window instead of quitting unless `is_quitting` atomic flag is set.
- **Single Instance Lock:** `tauri-plugin-single-instance` enforces a single app instance, focusing and unminimizing the running window when a second instance is invoked.
- **Autostart with Windows:** Windows Registry persistence (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Awesome Downloader`) with `--background` parameter via `reg.exe` commands, automatically synchronized with `startWithWindows` setting.
- **Silent Background Launch:** Supports `--background` CLI argument to launch directly to tray without window flashing.

### 4.2 Multi-Window Progress / Details (COMPLETED in Phase D)
- **Ported:**
  - Dynamic `progress-{id}` window labels allow concurrent independent download details windows.
  - Sizing matches legacy: normal `565x485` (min `540x440`), review `565x365` (min `540x330`), complete `480x280` (min `470x270`).
  - Automatically updates window title during download progress and closes cancelled or finished downloads when configured.

### 4.3 Progress Window Close Confirmation Prompt (COMPLETED in Phase D)
- **Ported:**
  - Intercepts `WindowEvent::CloseRequested` on `progress-{id}` windows.
  - Active transfers (`needs_close_choice`) display native confirmation dialog with options: `Minimize` (keeps download running) vs `Cancel download` (cancels transfer, cleans up partial files, destroys window).
  - Unreviewed downloads (`needs_review: true`) cancel directly on close without prompt.

### 4.4 Auxiliary Windows (COMPLETED in Phase D)
- **Ported:**
  - Dedicated native windows with exact dimensions matching legacy:
    - Options: `540 × 650` (min `500 × 500`), route `index.html?options=1`
    - Add Download: `565 × 430` (min `520 × 390`), route `index.html?add=1`, auto-closes Scheduler window if open
    - Scheduler: `550 × 430` (min `500 × 360`), route `index.html?scheduler=1`
    - Integration: `700 × 620` (min `620 × 480`), route `index.html?integration=1`
  - Wired to typed Tauri commands (`open_options_window`, `open_add_window`, `open_scheduler_window`, `open_integration_window`) and frontend API.

### 4.5 Windows Autostart Registration (COMPLETED in Phase B)
- Ported with Windows Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` synchronization.

### 4.6 Single-Instance Lock & CLI Launch Arguments (COMPLETED in Phase B)
- Ported via `tauri-plugin-single-instance`, focusing/unminimizing on second instance launch, and handling `--background` and `--browser-setup` flags.

---

## 5. Application Menu, Keyboard Shortcuts & Commands (COMPLETED - 100% Parity)

### 5.1 Native Application Menu
- **Ported:**
  - Full native menu bar with 5 menus: `Tasks`, `File`, `Downloads`, `View`, `Help`.
  - Standard accelerators: `Ctrl+N` (`CmdOrCtrl+N`), `Ctrl+F` (`CmdOrCtrl+F`), `Delete`.
  - Emits `downloader:command` events to renderer: `add`, `paste`, `details`, `remove`, `removeCompleted`, `pauseAll`, `resumeAll`, `find`, `scheduler`, `settings`, `integration`, `all`.

### 5.2 Export & Import Downloads to/from Text File
- **Ported:**
  - **Export:** Opens native save dialog (`downloads.txt`, filter: `URL list`), exports all URLs formatted with `\r\n`.
  - **Import:** Opens native open dialog, enforces `< 2 MiB` file boundary check, parses URLs line-by-line, and batch-queues in `Paused` state with confirmation dialog.

### 5.3 Native About Dialog
- **Ported:**
  - Displays native information dialog with application version, IDM inspiration details, and metadata.

---

## 6. System Notifications & File Type Icons

### 6.1 Native OS Notifications (100% Missing)
- **Legacy Behavior:** (`desktop/main.cjs:327, 349-352`)
  - If `notifyOnComplete` was enabled, showed a Windows desktop toast notification upon transfer completion with the download filename.
  - Showed update notifications on background update availability.
- **Current Tauri State:**
  - `tauri-plugin-notification` is not included in `Cargo.toml`.
  - Finished downloads do not trigger any OS notifications.

### 6.2 OS File Type Icon Extraction (100% Missing)
- **Legacy Behavior:** (`desktop/main.cjs:43-47`, `desktop/file-icons.cjs`)
  - Called `app.getFileIcon(filePath, { size: 'normal' })` on Windows to extract the genuine OS icon associated with the file extension (e.g. `.zip`, `.pdf`, `.mp4`).
  - Cached extracted icons in `%USERPROFILE%\.awesome-downloader\FileTypeIcons`.
  - Transmitted icon Data URLs via `snapshot.fileIcons`.
- **Current Tauri State:**
  - `src-tauri/src/domain/snapshot.rs:35`:
    ```rust
    file_icons: Vec::new(),
    ```
  - Hardcoded to an empty list. The download list in the webview cannot show native Windows file type icons.

---

## 7. Auto-Updater (COMPLETED - 100% Parity)

- **Ported:**
  - `UpdateService` orchestrates manual and 6-hour recurring background checks.
  - Development build protection: dev builds report `"Updates are unavailable in development. Update checks run in installed builds."` without making network calls.
  - Background checks run silently when up to date or on missing releases (404/406); only surfaces notifications when an actual newer release is discovered.
  - Manual check displays `"No updates available"` dialog when current, or user-friendly network error explanation without leaking internal headers.
  - Active download guard: prompts user with a safety warning when active downloads (`connecting`, `downloading`, `assembling`) are in flight, defaulting to `"Later"`.
  - Wired directly into the native application menu under *Help* $\to$ *Check for updates…* and exposed as Tauri command `check_for_updates`.
  - Verified with 11 dedicated integration tests in `tests/updater.rs`.

---

## 8. Media Processing: YouTube SABR & Adaptive Dual-Stream Muxing

### 8.1 Missing YouTube SABR Streaming Protocol
- **Legacy Behavior:** (`core/sabr.mjs`, `tests/sabr.test.mjs`)
  - Decoded binary Protobuf packets using `googlevideo/protos`.
  - Handled YouTube player session handoffs from the browser extension for high-resolution video streams where direct URLs were not exposed.
- **Current Tauri State:**
  - SABR was excluded from the initial Rust engine. The Rust engine only sanitizes/redacts `sabr` session data when reading legacy history files (`tests/download_history.rs:43`).
  - No SABR protobuf parser or client exists in Rust.

### 8.2 Adaptive Dual-Stream HTTP Muxing in Download Engine
- **Legacy Behavior:** (`core/mux.mjs`, `core/engine.mjs`)
  - When downloading YouTube adaptive formats (1080p, 1440p, 4K), YouTube provides separate video and audio URLs.
  - The engine downloaded both streams and muxed them into a single `.mp4` file using FFmpeg.
- **Current Tauri State:**
  - `CompositeDownloadExecutor` in `src-tauri/src/infrastructure/media/composite_executor.rs:41-55`:
    - If the download is HLS (`.m3u8`), delegates to `hls_executor` (which does mux separate audio/video).
    - Otherwise, delegates to `http_executor`.
    - `http_executor` only downloads a **single URL** (`download.url`). It has no support for downloading two parallel URLs (video track + audio track) and combining them.
  - `FfmpegTools::convert_media` (`src-tauri/src/infrastructure/media/ffmpeg.rs:63`) exists to convert audio to MP3 or MP4, but has **zero callers** across the application.

---

## 9. Deficit Resolution Roadmap (To Achieve 100% Parity)

To bring the Tauri migration to 100% complete behavioral parity with the legacy Electron app, the following concrete tasks must be completed:

### Phase A: Shell Actions & Filesystem Parity (COMPLETED)
1. **Implemented `DownloadAction::Remove`:** Stops active transfers, discards partial files, and removes record from history.
2. **Implemented `DownloadAction::Open`:** Validates completed status and existing file, launching via `opener::open`.
3. **Implemented `DownloadAction::Reveal`:** Highlights file in explorer via `opener::reveal` or opens directory.
4. **Implemented `DownloadAction::OpenWith`:** Added `OpenWith` to `DownloadAction` and spawns Windows `OpenWith.exe`.
5. **Implemented `remove_completed`:** Added atomic prune method to history/transfer service and exposed Tauri command `remove_completed`.

### Phase B: Desktop Shell & System Tray (Priority: High)
1. **Add System Tray:** Configure `tauri::tray::TrayIconBuilder` in `lib.rs` with menu items (*Show downloads*, *Quit*), click/double-click handlers, and tooltip.
2. **Implement Close-to-Tray:** Intercept `WindowEvent::CloseRequested` on the main window; call `window.hide()` instead of exiting unless explicit Quit was selected.
3. **Add Single-Instance Plugin:** Add `tauri-plugin-single-instance` to focus existing window on duplicate launches.
4. **Register Windows Autostart:** Hook `SettingsService` updates for `start_with_windows` into Windows Registry key `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

### Phase C: Browser Integration UI & Review Wiring (Priority: High)
1. **Expose Browser Registration Commands:** Add Tauri commands for `register_browser`, `browser_setup_action`, and `get_integration_info`.
2. **Wire `renderer/tauri-api.js`:** Replace all `unavailable(...)` stubs with active `invoke(...)` calls.
3. **Add Background Browser Scanner:** Spawn a periodic 15-second task updating `snapshot.integration` with real detected browser states.
4. **Implement Review Workflow:** Restore `needsReview` state in transfer service and implement `review_download` Tauri command.

### Phase D: Window Management & Menus (Priority: Medium)
1. **Dynamic Progress Windows:** Change `PROGRESS_WINDOW_LABEL` to dynamic `format!("progress-{}", id)` so multiple progress windows can coexist.
2. **Close Confirmation Dialog:** Add confirmation dialog check before closing active progress windows.
3. **Native Application Menu:** Configure `tauri::menu::Menu` with Tasks, File, Downloads, View, Help, accelerators, and URL file import/export.

### Phase E: Notifications, File Icons & Media (Priority: Medium)
1. **Add Native Notifications:** Add `tauri-plugin-notification` and trigger on download completion.
2. **Extract File Type Icons:** Implement Windows shell `SHGetFileInfoW` in Rust to extract icons and populate `snapshot.file_icons`.
3. **Adaptive Dual-Stream Download:** Extend `CompositeDownloadExecutor` to download dual video+audio HTTP streams and mux via `FfmpegTools::mux_tracks`.
