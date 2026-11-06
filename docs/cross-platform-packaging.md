# Cross-Platform Packaging, Distribution & Signing Guide

**Scope:** macOS (Apple Silicon & Intel), Linux (Debian/Ubuntu & AppImage), and Windows (x64 & ARM64).

---

## 1. Distribution Artifact Matrix

| Target Platform | Architecture | Bundle Format | Output Artifact |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Silicon (`aarch64`) | DMG & `.app` | `Open Download Manager_aarch64.dmg` |
| **macOS** | Intel (`x86_64`) | DMG & `.app` | `Open Download Manager_x64.dmg` |
| **macOS** | Universal (`universal`) | DMG & `.app` | `Open Download Manager_universal.dmg` |
| **Linux** | x86_64 | `.deb` | `open-download-manager_amd64.deb` |
| **Linux** | x86_64 | `.AppImage` | `Open Download Manager_amd64.AppImage` |
| **Windows** | x64 | NSIS (`.exe`) | `Open Download Manager_x64-setup.exe` |
| **Windows** | ARM64 | NSIS (`.exe`) | `Open Download Manager_arm64-setup.exe` |

---

## 2. Media Tools Resolution (FFmpeg & yt-dlp)

High-resolution format muxing and YouTube playlist discovery rely on `ffmpeg` and `yt-dlp`. Both tools are resolved dynamically across all platforms with the following order of precedence:

1. **Environment Variables**:
   - `AD_FFMPEG_PATH`: Explicit path to custom `ffmpeg` binary.
   - `AD_YTDLP_PATH`: Explicit path to custom `yt-dlp` binary.
2. **Bundled Companion Tools**:
   - **macOS**: `Contents/Resources/tools/ffmpeg` and `Contents/MacOS/tools/ffmpeg`
   - **Linux**: `/usr/lib/open-download-manager/tools/` or adjacent `tools/`
   - **Windows**: `<install_dir>/tools/ffmpeg.exe` and `tools/yt-dlp.exe`
3. **Standard System Package Managers**:
   - **macOS**: Homebrew (`/opt/homebrew/bin/`), Intel Homebrew (`/usr/local/bin/`), MacPorts (`/opt/local/bin/`)
   - **Linux**: `/usr/bin/`, `/usr/local/bin/`, `/snap/bin/`
   - **System PATH**: `which::which("ffmpeg")` and `which::which("yt-dlp")`

---

## 3. macOS Code Signing & Notarization Plan

To avoid Gatekeeper warnings on macOS:

### Prerequisites:
- Apple Developer Program membership.
- `Developer ID Application` certificate exported as `.p12`.
- App-specific password generated from appleid.apple.com.

### Environment Variables / GitHub Secrets:
```bash
APPLE_CERTIFICATE="<base64-encoded .p12>"
APPLE_CERTIFICATE_PASSWORD="<p12 password>"
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
APPLE_ID="developer@example.com"
APPLE_PASSWORD="<app-specific-password>"
APPLE_TEAM_ID="<10-char Team ID>"
```

### Local / Ad-Hoc Signing (for development/testing):
```bash
# Ad-hoc sign for local execution without Apple Developer account
codesign --force --deep --sign - "target/release/bundle/macos/Open Download Manager.app"
```

---

## 4. Native Messaging Host Locations

When users click **"Refresh browsers"** or install the browser extension, the native messaging manifest `com.opendownloadmanager.bridge.json` is registered at:

- **macOS**:
  - Chrome: `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/`
  - Edge: `~/Library/Application Support/Microsoft Edge/NativeMessagingHosts/`
  - Brave: `~/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts/`
  - Opera: `~/Library/Application Support/com.operasoftware.Opera/NativeMessagingHosts/`
  - Firefox: `~/Library/Application Support/Mozilla/NativeMessagingHosts/`
- **Linux**:
  - Chrome/Chromium: `~/.config/google-chrome/NativeMessagingHosts/`
  - Brave: `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/`
  - Opera: `~/.config/opera/NativeMessagingHosts/`
  - Firefox: `~/.mozilla/native-messaging-hosts/`
- **Windows**:
  - Windows Registry under `HKCU\Software\<Browser>\NativeMessagingHosts\com.opendownloadmanager.bridge`

---

## 5. Clean-Install, Upgrade & Uninstall Validation Checklist

- [x] **Clean Installation**:
  - macOS: Drag `.app` into `/Applications`, verify icon and dock display.
  - Linux: `sudo dpkg -i open-download-manager_amd64.deb` creates menu launcher.
  - Windows: NSIS installs to currentUser local app data without requiring admin.
- [x] **Browser Handoff**:
  - Opening extension and clicking "Refresh browsers" writes valid JSON manifest.
  - Download requests from browser bridge are authenticated via 256-bit token.
- [x] **Media Handoff & Merge**:
  - Direct downloads stream via multi-segmented HTTP chunks.
  - YouTube adaptive streams mux video and audio without quality degradation.
- [x] **Clean Uninstall**:
  - Unregistering browsers prunes NativeMessaging manifests.
  - App state in OS data directory is preserved or cleanly removable.
