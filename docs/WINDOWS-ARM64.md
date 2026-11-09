# Windows ARM64 support

## Status

**Build support:** implemented in source (Windows packaging run pending).
**Hardware validation:** pending.
**Published ARM64 release:** not yet available.

Windows ARM64 is not a released capability until the validation checklist below
is completed on physical Windows 11 ARM64 hardware and a maintainer publishes an
ARM64 installer.

## Supported matrix

| Capability | Windows x64 | Windows 11 ARM64 |
| :--- | :---: | :---: |
| Native application binary | Yes | Buildable; validation pending |
| Native messaging host | Yes | Buildable; validation pending |
| NSIS per-user installer | Yes | Buildable; validation pending |
| Browser handoff (Chrome/Edge) | Yes | Pending hardware QA |
| Browser handoff (Firefox) | Yes | Pending hardware QA |
| ffmpeg auto-download | x64 build | ARM64 build |
| yt-dlp auto-download | x64 build | ARM64 build |
| Media merge / MP3 conversion | Yes | Pending hardware QA |

Windows 10 ARM64 is not a target. Use Windows 11 ARM64 (Snapdragon X / SQ
series or equivalent).

## Build

On a Windows ARM64 machine, install the MSVC ARM64 build tools and Rust target:

```powershell
rustup target add aarch64-pc-windows-msvc
bun install --frozen-lockfile
bun run build:win:arm64
```

Artifact:

```text
target\aarch64-pc-windows-msvc\release\bundle\nsis\Open Download Manager_<version>_arm64-setup.exe
```

For an x64 build:

```powershell
bun run build:win
```

Artifacts use distinct filenames (`_arm64-setup.exe` and `_x64-setup.exe`), so
they cannot overwrite one another.

### Cross-compiling from an x64 Windows host

Install the "MSVC v143 - VS 2022 C++ ARM64 build tools" component, add the Rust
target, then run the same `build:win:arm64` command. Native ARM64 hardware is
preferred for production artifacts.

### Installer architecture

Tauri's NSIS installer stub is x86 and runs under Windows emulation. The app,
native messaging host, and media tools must be native ARM64. Check the installed
binaries:

```powershell
.\scripts\verify-pe-arch.ps1 `
  -Path "$env:LOCALAPPDATA\Programs\Open Download Manager\open-download-manager.exe" `
  -Expected arm64

.\scripts\verify-pe-arch.ps1 `
  -Path "$env:LOCALAPPDATA\Programs\Open Download Manager\open-download-manager-native-host.exe" `
  -Expected arm64
```

Expected PE machine values: ARM64 `0xAA64`, x64 `0x8664`.

## Media tools

On Windows ARM64 the app downloads:

- ffmpeg: BtbN `ffmpeg-master-latest-winarm64-gpl.zip`
- yt-dlp: `yt-dlp_arm64.exe`

Windows x64 FFmpeg is pinned to GyanD build 7.1. Windows ARM64 follows
BtbN's rolling `latest` build because there is no stable versioned ARM64
asset; record the resolved build and SHA-256 when preparing a release.

Archives are extracted in-process; no `unzip` command is required.

The project does not bundle ffmpeg or yt-dlp. Users can also install tools
manually with `winget`, `scoop`, or `choco`, or set `AD_FFMPEG_PATH` and
`AD_YTDLP_PATH`. See
[`cross-platform-packaging.md`](cross-platform-packaging.md) for the full
resolution order and licensing notes.

## Hardware validation checklist

Run on a physical Windows 11 ARM64 device. Record results and redacted evidence
in the Windows ARM64 goal issue.

### Installation

- [ ] NSIS installer runs without requiring administrator rights.
- [ ] App installs under `%LOCALAPPDATA%\Programs\Open Download Manager`.
- [ ] Start-menu and desktop shortcuts launch the app.
- [ ] Main executable PE machine is `0xAA64`.
- [ ] Native host executable PE machine is `0xAA64`.

### Browser integration

- [ ] Chrome ARM64 registration writes the native messaging manifest/registry
      entry and the extension reaches the app.
- [ ] Edge ARM64 registration and handoff work.
- [ ] Firefox ARM64 registration and handoff work, if Firefox is claimed.
- [ ] Invalid/expired handoff tokens are rejected.

### Downloads

- [ ] Direct HTTP download.
- [ ] Segmented download with resume.
- [ ] Cancel removes partial output.
- [ ] Pause/resume works.
- [ ] Progress and completion windows behave correctly.

### Media

- [ ] yt-dlp ARM64 fetches formats.
- [ ] ffmpeg ARM64 merges video + audio.
- [ ] MP3 conversion works.
- [ ] Child processes are ARM64, not emulated x64.

### Upgrade / uninstall

- [ ] x64 → ARM64 upgrade installs the native binary.
- [ ] ARM64 → x64 upgrade does not leave a mixed-architecture install.
- [ ] Uninstall removes app files, shortcuts, and native messaging registrations.
- [ ] Downloads and user settings follow the documented retention policy.

## Known limitations

- Installer stub is x86 (Tauri/NSIS design); the app itself is native ARM64.
- Installers are unsigned; SmartScreen may warn.
- Windows 10 ARM64 is not supported.
- Linux ARM64 ffmpeg is not auto-downloaded.
- ARM64 artifacts are not published until hardware validation completes.

## Related

- [Cross-platform packaging](cross-platform-packaging.md)
- [Install guide](INSTALL.md)
- [Release checklist](RELEASING.md)
