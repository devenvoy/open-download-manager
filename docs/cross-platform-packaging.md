# Cross-Platform Packaging, Distribution & Signing Guide

**Scope:** macOS (Apple Silicon & Intel), Linux (Debian/Ubuntu & AppImage), and Windows (x64 & ARM64).

> **Status:** Build and packaging support is implemented. Windows ARM64 hardware
> validation and a published ARM64 release are still pending. Do not describe
> Windows ARM64 as a released, validated capability until the checklist in
> [`WINDOWS-ARM64.md`](WINDOWS-ARM64.md) is complete.

---

## 1. Distribution Artifact Matrix

Tauri names NSIS artifacts as `<product>_<version>_<arch>-setup.exe`.

| Target Platform | Architecture | Bundle Format | Output Artifact |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Silicon (`aarch64`) | DMG & `.app` | `Open Download Manager_<version>_aarch64.dmg` |
| **macOS** | Intel (`x86_64`) | DMG & `.app` | `Open Download Manager_<version>_x64.dmg` |
| **macOS** | Universal (`universal`) | DMG & `.app` | `Open Download Manager_<version>_universal.dmg` |
| **Linux** | x86_64 | `.deb` | `open-download-manager_<version>_amd64.deb` |
| **Linux** | x86_64 | `.AppImage` | `Open Download Manager_<version>_amd64.AppImage` |
| **Windows** | x64 | NSIS (`.exe`) | `Open Download Manager_<version>_x64-setup.exe` |
| **Windows** | ARM64 | NSIS (`.exe`) | `Open Download Manager_<version>_arm64-setup.exe` |

### Build commands

```bash
bun run build:mac        # dmg + app
bun run build:linux      # deb + appimage
bun run build:win        # NSIS, x86_64 MSVC target
bun run build:win:arm64  # NSIS, aarch64 MSVC target
```

Building Windows ARM64 natively requires the MSVC ARM64 build tools and:

```powershell
rustup target add aarch64-pc-windows-msvc
```

Artifacts are written under `target/<target-triple>/release/bundle/nsis/`.
x64 and ARM64 installers have distinct filenames and cannot overwrite one
another.

The NSIS installer stub is a 32-bit x86 executable that Windows runs under
emulation, as documented by Tauri. The application, native messaging host and
media tools inside the installer must all be native ARM64. Verify the installed
binaries with `scripts/verify-pe-arch.ps1`; do not use installer-stub
architecture as proof of application architecture.

### Architecture verification

```powershell
.\scripts\verify-pe-arch.ps1 `
  -Path "$env:LOCALAPPDATA\Programs\Open Download Manager\open-download-manager.exe" `
  -Expected arm64
```

PE machine `0xAA64` = ARM64, `0x8664` = x64.

---

## 2. Media Tools Resolution (FFmpeg & yt-dlp)

High-resolution format muxing and YouTube playlist discovery rely on `ffmpeg`
and `yt-dlp`. Both are resolved dynamically with the following order of
precedence:

1. **Environment Variables**:
   - `AD_FFMPEG_PATH`: Explicit path to a custom `ffmpeg` binary.
   - `AD_YTDLP_PATH`: Explicit path to a custom `yt-dlp` binary.
2. **Bundled / Adjacent Tools** (only when a distributor stages them):
   - **macOS**: `Contents/Resources/tools/<tool>` and `Contents/MacOS/<tool>`
   - **Linux**: `/usr/lib/open-download-manager/tools/` or
     `/usr/libexec/open-download-manager/tools/`
   - **Windows**: `<install_dir>/tools/<tool>.exe` and
     `<install_dir>/<tool>.exe`
   - The project does **not** currently ship ffmpeg or yt-dlp in the installer.
3. **In-App Managed Directory**: `~/.open-download-manager/tools/`
   (`%USERPROFILE%\.open-download-manager\tools` on Windows).
4. **System PATH**:
   - macOS additionally consults `/usr/libexec/path_helper` so tools installed
     through Homebrew are found when the app is launched by Finder/launchd.
   - `which::which("ffmpeg")` and `which::which("yt-dlp")`.

### Automated tool downloads

Architecture-specific binaries are selected before download:

| Platform | ffmpeg | yt-dlp |
| :--- | :--- | :--- |
| Windows x64 | GyanD essentials build (`.zip`) | `yt-dlp.exe` |
| Windows ARM64 | BtbN `winarm64` GPL build (`.zip`) | `yt-dlp_arm64.exe` |
| macOS x64/ARM64 | evermeet.cx release (`.zip`) | `yt-dlp_macos` |
| Linux x64 | johnvansickle static build (`.tar.xz`) | `yt-dlp_linux` |
| Linux ARM64 | Not auto-downloaded; install via package manager or `AD_FFMPEG_PATH` | `yt-dlp_linux_aarch64` |

Windows x64 FFmpeg is pinned to GyanD build 7.1. Windows ARM64 follows
BtbN's rolling `latest` build because there is no stable versioned ARM64
asset; record the resolved build and SHA-256 when preparing a release.

Archives are extracted in-process (ZIP, tar.xz, tar.gz). No external `unzip` or
`tar` command is required, so installation works on stock Windows systems.
Unsafe archive paths are rejected.

`AD_FFMPEG_MIRROR` and `AD_YTDLP_MIRROR` override the download URL. Mirrors are
responsible for serving a binary matching the host architecture.

### Adapter layout

Platform behavior lives under `src-tauri/src/infrastructure/media/tools/`:

- `adapter.rs` defines the `MediaToolAdapter` interface and `ToolPlatform`.
- `adapters/windows.rs`, `adapters/macos.rs`, and `adapters/linux.rs` own
  download URLs, executable names, bundled sidecar locations, and any
  platform-specific PATH discovery. Each OS adapter handles both x64 and ARM64.
- `downloader.rs`, `archive.rs`, `resolver.rs`, and `manager.rs` remain
  platform-neutral.

All adapters compile on every host. The running host is selected at runtime,
and tests can instantiate any adapter directly, so the Windows ARM64 matrix is
covered from macOS and Linux test machines.

### Dependency licensing

- **FFmpeg**: Windows ARM64 uses BtbN's GPL build. Distributors who bundle or
  redistribute that binary must satisfy GPL source-offer obligations and
  document the build configuration. Runtime downloads from a third-party mirror
  are governed by that project's license.
- **yt-dlp**: Unlicense/public domain.
- **WebView2**: Distributed by Microsoft under its own terms.

Add any redistributed binary to `THIRD-PARTY.md` and the release license notice.

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

Windows code signing is not configured. Windows SmartScreen may warn that the
installer is unsigned.

---

## 4. Native Messaging Host Locations

When users click **"Refresh browsers"** or install the browser extension, the native messaging manifest `com.open.downloadmanager.json` is registered at:

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
  - Windows Registry under `HKCU\Software\<Browser>\NativeMessagingHosts\com.open.downloadmanager`

The manifest points at `open-download-manager-native-host.exe` next to the main
application. That binary must be built for the same architecture as the app.

---

## 5. Validation Checklist

- [x] **Build configuration**: macOS, Linux, Windows x64 and Windows ARM64
      targets are wired into the build scripts.
- [x] **Media tool architecture selection**: ARM64 URLs and in-process archive
      extraction are covered by unit/integration tests.
- [ ] **Windows ARM64 clean installation**: verify on physical Windows 11 ARM64
      hardware; confirm installed app and native host are native ARM64.
- [ ] **Browser handoff on Windows ARM64**: register Chrome/Edge (ARM64) and
      verify an authenticated download request reaches the app.
- [ ] **Downloads and media assembly on Windows ARM64**: direct download,
      segmented download, cancel/resume, YouTube merge with ARM64 ffmpeg.
- [ ] **Upgrade and uninstall on Windows ARM64**: x64↔ARM64 upgrade behavior,
      registry cleanup, user data retention policy.
- [ ] **macOS / Linux regression**: confirm packaging still works after media
      tool changes.
- [x] **Release hygiene**: artifacts have distinct architecture names; hashes
      must be recorded before publishing.

Physical Windows ARM64 evidence is required before closing the Windows ARM64
goal. Emulation and build-only CI are not proof of native compatibility.
