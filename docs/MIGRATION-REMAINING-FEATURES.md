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
| **Browser Integration UI** | Live status scanner, setup wizard, extension store deep-links, confirmation review | Loopback server and native host binary exist, but UI APIs and snapshot are completely stubbed or hardcoded to unavailable | **35%** | User cannot view browser status, trigger browser setup, or review browser downloads |
| **Window Management** | Unlimited concurrent progress windows, dedicated options/add/scheduler windows, close confirmation prompts | Single hardcoded progress window label, auxiliary windows stubbed to in-page modals, no close confirmation | **30%** | Only 1 download details window can exist at a time; no safety prompt when closing active transfers |
| **Application Menu & Shortcuts**| Full native menu bar with accelerators (Ctrl+N, Ctrl+F, Del), URL text import/export, about dialog | No application menu, no accelerators, no `downloader:command` event generation, no import/export | **0%** | Traditional desktop menu bar and URL batch file import/export are completely missing |
| **System Notifications** | Windows native notifications on completion and updates | No notification plugin or calls | **0%** | Downloads complete silently without OS desktop toast notifications |
| **File Type Icons** | Native Windows file type icon extraction via shell API with disk caching | Hardcoded to empty list `Vec::new()` | **0%** | File list shows generic icons; OS-associated file icons are not extracted |
| **Auto-Updater** | Background 6-hour polling, manual check, download approval, active transfer guard | No updater plugin or logic | **0%** | No update checks, signing, or updater mechanisms configured |
| **YouTube SABR & Multi-Stream**| SABR protobuf streaming via `googlevideo/protos`, adaptive video+audio muxing, MP3 extraction | Format listing via `yt-dlp` works; SABR is unmigrated; adaptive dual-stream HTTP muxing is unhooked | **40%** | YouTube high-res adaptive streams (video+audio) are not downloaded and muxed into MP4/MP3 |

---

## 2. Download Context Actions & File System Operations (COMPLETED - 100% Parity)

All download context actions (`remove`, `open`, `reveal`, `openWith`, and `removeCompleted`) have been implemented and verified in Phase A:
- **`remove`:** Stops active transfers, discards partial `.part` and segment files via `executor.discard_partial`, and removes the record from `DownloadHistoryRepository` and atomic persistence.
- **`open`:** Verifies completed state and existing file, launching the file via `opener::open`.
- **`reveal`:** Highlights finalized files in Windows Explorer / OS file manager via `opener::reveal`, falling back to opening the folder for in-progress or directory-only downloads.
- **`openWith`:** Added `DownloadAction::OpenWith` (with camelCase serde) and launches Windows `%SystemRoot%\System32\OpenWith.exe` on completed files.
- **`removeCompleted`:** Prunes all completed records from history in a single atomic transaction without deleting finalized files from disk, exposed as Tauri command `remove_completed` and `api.removeCompleted()`.

---

## 3. Browser Integration & UI Workflow Deficits

Even though the loopback bridge server (`LoopbackBridge`) and native host binary (`awesome-downloader-native-host`) have been written in Rust, they are **disconnected from the UI layer**.

### 3.1 Hardcoded `snapshot.integration` Status
- **Legacy Behavior:** (`desktop/main.cjs:120-127`)
  - Polled connected browsers every 15 seconds.
  - Populated `snapshot.integration` with:
    - `port`: Current bridge port.
    - `connectedBrowsers`: List of browsers with active native host manifests.
    - `browsers`: List of all supported browsers with installation state, profile counts, and setup state.
    - `extensionPath`, `integrationPath`, `packaged`.
- **Current Tauri State:**
  - `src-tauri/src/domain/snapshot.rs:32-34`:
    ```rust
    integration: IntegrationStatus::unavailable(
        BROWSER_HANDOFF_UNAVAILABLE_MESSAGE.to_owned(), // "Browser handoff has not been migrated to Tauri yet."
    ),
    ```
  - **Deficit:** The UI unconditionally reports that browser integration is unavailable and disconnected, even when the bridge server is running in the background.

### 3.2 Stubbed Frontend Bridge (`renderer/tauri-api.js`)
- **Legacy Behavior:** Provided full IPC communication for browser management.
- **Current Tauri State:** Lines 77-80 of `renderer/tauri-api.js`:
  ```javascript
  review: () => unavailable('Browser download review'),
  integration: () => unavailable('Browser integration'),
  browserSetup: () => unavailable('Browser setup'),
  openIntegration: () => unavailable('Browser integration'),
  ```
  - **Deficit:** Clicking "Browser Integration" in the toolbar or options throws an explicit JavaScript error: *"Browser integration is not available in the Tauri migration yet."*

### 3.3 Missing Browser Setup & Registration Commands
- **Legacy Behavior:** (`desktop/main.cjs:244-270`, `desktop/browser-setup.cjs`)
  - Handled user actions in the Browser Integration dialog:
    - `'store'`: Registers native messaging manifest and launches the Chrome Web Store / Add-ons page for the browser.
    - `'open'`: Launches the browser directly.
    - `'prepare'`: Generates Firefox manifest and folder for temporary/unpacked loading.
    - `'copy'`: Copies extension folder path to system clipboard.
    - `'folder'`: Opens the extension folder in Windows Explorer.
- **Current Tauri State:**
  - `src-tauri/src/infrastructure/browser/registration.rs` contains helper functions (`register_browser_manifest`), but **no Tauri presentation commands** exist to call them from the webview.

### 3.4 Missing Browser Download Review Workflow (`confirmBrowserDownloads`)
- **Legacy Behavior:** (`desktop/main.cjs:50-55`, `core/engine.mjs`)
  - When `confirmBrowserDownloads` was enabled in settings, incoming downloads from the browser extension were added with `needsReview: true`.
  - Automatically popped up a confirmation dialog allowing the user to review the URL, change filename, select category/folder, or cancel before transferring bytes.
  - Closing the confirmation dialog automatically cancelled the pending download.
  - `api.review({ id, action: 'start' | 'cancel', ... })` finalized the download.
- **Current Tauri State:**
  - `src-tauri/src/infrastructure/bridge/server.rs:319` unconditionally queues or starts downloads directly.
  - `api.review` is stubbed as `unavailable`.
  - Closing the details window does not cancel pending unapproved downloads.

---

## 4. Desktop Shell, Window Management & Lifecycle

### 4.1 System Tray, Close-to-Tray, Autostart & Single Instance (COMPLETED - 100% Parity)
Implemented and verified in Phase B:
- **System Tray:** Configured in `src-tauri/src/presentation/tray.rs` with tooltip `"Awesome Downloader"`, custom icon, and context menu (`Show downloads`, `Quit`). Left click and double click activate and unminimize main window.
- **Close-to-Tray:** Intercepts main window `CloseRequested` events in `lib.rs`; hides the window instead of quitting unless `is_quitting` atomic flag is set.
- **Single Instance Lock:** `tauri-plugin-single-instance` enforces a single app instance, focusing and unminimizing the running window when a second instance is invoked.
- **Autostart with Windows:** Windows Registry persistence (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Awesome Downloader`) with `--background` parameter via `reg.exe` commands, automatically synchronized with `startWithWindows` setting.
- **Silent Background Launch:** Supports `--background` CLI argument to launch directly to tray without window flashing.

### 4.2 Multi-Window Progress / Details Limitation
- **Legacy Behavior:** (`desktop/main.cjs:17-20, 172-215`)
  - Maintained a map of independent `BrowserWindow` instances (`progressWindows = new Map<id, BrowserWindow>()`).
  - The user could open multiple progress windows simultaneously for different downloads.
  - Each window had dynamic titles reflecting real-time progress (e.g. `45% archive.zip`).
- **Current Tauri State:**
  - `src-tauri/src/presentation/window_manager.rs:7, 29-43`:
    - Hardcodes a single window label: `const PROGRESS_WINDOW_LABEL: &str = "progress";`.
    - If a progress window is already open, opening another download evaluates JavaScript on the existing window to change its URL search parameter.
  - **Deficit:** Concurrent download progress windows cannot exist simultaneously.

### 4.3 Missing Progress Window Close Confirmation Prompt
- **Legacy Behavior:** (`desktop/main.cjs:183-209`, `desktop/progress-window.cjs`)
  - If the user clicks `X` on a progress window while a download is actively running (`connecting`, `downloading`, `assembling`), Electron intercepted the close and displayed a modal prompt:
    > *"What would you like to do? [Minimize] [Cancel download]"*
    - Selecting "Minimize" kept the download running in the background.
    - Selecting "Cancel download" aborted the transfer, cleaned up partial files, and closed the window.
- **Current Tauri State:**
  - `close_auxiliary_window` in `window_manager.rs:60-70` calls `window.close()` directly.
  - **Deficit:** No confirmation prompt. Users have no safety guard against accidentally closing an active transfer window.

### 4.4 Auxiliary Windows Stubbed to In-Page Modals
- **Legacy Behavior:** (`desktop/main.cjs:129-169`)
  - Electron spawned separate OS windows for:
    - Options Window (`width: 540, height: 650, index.html?options=1`)
    - Add Download Window (`width: 565, height: 430, index.html?add=1`)
    - Scheduler Window (`width: 550, height: 430, index.html?scheduler=1`)
    - Integration Window (`width: 700, height: 620, index.html?integration=1`)
- **Current Tauri State:**
  - `renderer/tauri-api.js:80-84`:
    ```javascript
    openOptions: () => Promise.resolve(),
    openAdd: () => Promise.resolve(),
    openScheduler: () => Promise.resolve()
    ```
  - **Deficit:** Stubs force the renderer to display in-page HTML `<dialog>` elements inside the main window rather than opening independent native windows.

### 4.5 Windows Autostart Registration (COMPLETED in Phase B)
- Ported with Windows Registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` synchronization.

### 4.6 Single-Instance Lock & CLI Launch Arguments (COMPLETED in Phase B)
- Ported via `tauri-plugin-single-instance`, focusing/unminimizing on second instance launch, and handling `--background` and `--browser-setup` flags.

---

## 5. Application Menu, Keyboard Shortcuts & Commands

### 5.1 Native Application Menu (100% Missing)
- **Legacy Behavior:** (`desktop/main.cjs:286-309`)
  - Provided a full Windows standard menu bar:
    - **Tasks:** Add new download (Ctrl+N), Add batch, Add batch from clipboard, Export to text file, Import from text file, Exit.
    - **File:** Download properties, Remove selected (Delete), Remove completed downloads.
    - **Downloads:** Pause all, Stop all, Delete all completed, Find (Ctrl+F), Scheduler, Start queue, Stop queue, Speed limiter, Options.
    - **View:** All downloads, Fullscreen, Zoom In/Out/Reset.
    - **Help:** Browser integration, Check for updates, About Awesome Downloader.
- **Current Tauri State:**
  - `tauri.conf.json` and `lib.rs` do not configure an application menu (`tauri::menu`).
  - No menu accelerators (Ctrl+N, Ctrl+F, Delete).
  - The renderer has a listener for `downloader:command` events (`api.onCommand`), but the Rust backend never emits any menu commands.

### 5.2 Export & Import Downloads to/from Text File
- **Legacy Behavior:** (`desktop/main.cjs:288-303`)
  - **Export:** Opened `dialog.showSaveDialog` and exported all download URLs to a clean text file (`downloads.txt`).
  - **Import:** Opened `dialog.showOpenDialog`, validated file size (< 2 MiB), parsed URLs line-by-line, and batch-queued them in paused state.
- **Current Tauri State:**
  - Completely absent. No commands, dialogs, or service methods exist to export or import URL lists.

### 5.3 Native About Dialog
- **Legacy Behavior:** (`desktop/main.cjs:308`)
  - Showed a native message box displaying the app version and IDM inspiration details.
- **Current Tauri State:**
  - Absent.

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

## 7. Auto-Updater (100% Missing)

- **Legacy Behavior:** (`desktop/main.cjs:340-359`, `desktop/updater.cjs`)
  - Integrated `electron-updater`.
  - Automatic silent background checks every 6 hours.
  - Manual check via Help -> *Check for updates...*
  - Safety check: If active downloads were running, defaulted to "Later" rather than restarting.
  - Prompted user approval before downloading and installing.
- **Current Tauri State:**
  - `tauri-plugin-updater` is not configured.
  - No update endpoints, signing keys, or update checking commands exist.

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
