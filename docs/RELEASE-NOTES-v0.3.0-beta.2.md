# Release notes — v0.3.0-beta.2

> The `v0.3.0-beta.1` release was withdrawn after it shipped. The Torrents
> window crashed on the file list, and the error, settings and clipboard paths
> in the main window could throw as well. This release carries the fixes.

## Features
- **🧲 Built-in BitTorrent Engine (Beta)**: The rustTorrent engine is now compiled into every build — no feature flag, no separate build. Add magnets and `.torrent` files from the new **Torrents** tab, pick the files you want, confirm the download, and the engine takes over (DHT, upload/download limits, peer limits and per-torrent progress).
- **🔗 Torrent Hand-off From The Desktop**: Double-click a `.torrent` file or open a `magnet:` link and the review window opens directly with the torrent details and file selection.
- **✅ Truthful Browser Integration Status**: The app no longer guesses whether your extension is installed by reading browser profile folders (macOS blocks that for Chrome and Edge, so it always reported "not detected"). The extension now identifies itself over native messaging, and the app reports **Connected / Setup needed / Disconnected / Not installed** from real traffic.
- **📥 One-Click Extension Delivery**: Browser Integration now delivers the extension to `Downloads/Open Download Manager Extension/` and opens the folder, so the browser can load it from a normal, visible place — no hidden folders, no build-machine paths.
- **🧹 No More `_up_` Resource Folder**: The extension ships next to the app as `Resources/extension` and `Resources/extension-firefox`, resolved through Tauri's own resource directory.
- **🎯 Quiet Heartbeat**: The extension reports itself every two minutes while the desktop app runs and backs off to fifteen minutes while it is closed, so a closed app costs almost nothing.
- **🧩 Monolith Split Into Modules**: `commands.rs`, `scanner.rs`, `proxy_discovery.rs`, `hls_executor.rs` and the download transfer service are split into focused modules; the YouTube media code is now a package instead of one large file.
- **🌐 Browser Request Headers**: Downloads started from the extension now carry the page's `Referer`, `Cookie` and `User-Agent`, with an explicit header allow-list and sanitisation.
- **📝 Better File Names**: `Content-Disposition` and MIME information are used to give downloaded files the right name and extension.

---

## Bugs Fixed
- **Browser badge never cleared**: The toolbar badge stayed on "setup needed" forever, even with a fully working extension, because the app probed browser profile folders that macOS denies access to.
- **Extension pointed at the developer's machine**: Released builds told users to load the extension from the folder of the machine that compiled the binary (`/Users/<developer>/...`). The path now always points at the app.
- **Removed extension still reported as connected**: The connection state is derived from live traffic again, so an extension that was removed or disabled is detected within minutes.
- **Extension could not be loaded from the app bundle**: Browsers cannot pick a folder inside an `.app` bundle; the extension is now delivered to the Downloads folder.
- **Torrent feature needed a build flag**: The `torrent` Cargo feature and the Windows ARM64 compile guard are gone; the engine builds everywhere.
- **Torrents window crashed on the file list**: The window kept the info hash of every cached file list in a map that was never created, so reading the file list of a torrent threw "undefined is not an object". The map is declared now.
- **Error dialog did nothing**: The buttons in the download error dialog called a helper that was never written, so "Retry", "Install ffmpeg" and "Check proxy settings" threw instead of acting.
- **Settings window threw while opening**: The per-category download folders were written to a variable that was never declared, so opening the options window failed with "optionsCategoryDirectories is not defined".
- **Error details and the media tool dialog lost the selected download**: Both read the selection from the wrong place, so the dialog opened without a download and the error details could not be shown.
- **Clipboard monitoring threw**: The handler for a detected clipboard link was declared inside the startup callback and was therefore not in scope when the event arrived.
- **Lint gate for the frontend**: `bun run lint` runs ESLint over the renderer and the extension, and CI fails on an undefined identifier. A static test also fails when a script reads a member of an object that was never declared.

---

## Platform Support & Installer

| Platform | Variant / Architecture | Package Name | Format |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Silicon (M1/M2/M3/M4) | `open-download-manager_0.3.0-beta.2_mac_arm64.dmg` | DMG Installer |
| **macOS** | Apple Silicon App Archive | `open-download-manager_0.3.0-beta.2_mac_arm64.app.tar.gz` | App Archive |
| **macOS** | Intel 64-bit | `open-download-manager_0.3.0-beta.2_mac_x64.dmg` | DMG Installer |
| **macOS** | Intel App Archive | `open-download-manager_0.3.0-beta.2_mac_x64.app.tar.gz` | App Archive |
| **Windows** | Windows 10/11 64-bit (x64) | `open-download-manager_0.3.0-beta.2_win_x64-setup.exe` | NSIS Installer |
| **Windows** | Windows 11 ARM64 | `open-download-manager_0.3.0-beta.2_win_arm64-setup.exe` | NSIS Installer |
| **Linux** | Ubuntu / Debian 64-bit | `open-download-manager_0.3.0-beta.2_lin_x64.deb` | Debian Package |
| **Linux** | Universal Linux 64-bit | `open-download-manager_0.3.0-beta.2_lin_x64.AppImage` | AppImage |
| **Browser** | Chrome / Edge / Brave / Opera | `open-download-manager_0.3.0-beta.2_ext_chrome.zip` | ZIP Extension |
| **Browser** | Firefox Add-on | `open-download-manager_0.3.0-beta.2_ext_firefox.xpi` | Installable XPI |
| **Browser** | Firefox Source | `open-download-manager_0.3.0-beta.2_ext_firefox.zip` | Source ZIP |
| **Checksums** | SHA-256 Hashes | `open-download-manager_0.3.0-beta.2_checksums.txt` | Text |

---

## Browser Extension Setup (Beta)

1. Open **Browser Integration** in the toolbar and pick your browser.
2. Click **Download extension** — the extension is saved to `Downloads/Open Download Manager Extension/` and the folder opens in your file manager.
3. In the browser: open the extensions page, enable **Developer mode**, choose **Load unpacked** and select that folder.
4. Return to the app and click **Refresh**. The browser shows as **Connected** and the toolbar badge disappears.

---

> **Beta channel** — the BitTorrent tab is marked **Beta** in the app. Please report problems on the [issues page](https://github.com/devenvoy/open-download-manager/issues).

