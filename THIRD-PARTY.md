# Third-party media component

The previously published Electron preview uses ffmpeg-static 5.3.0
(GPL-3.0-or-later) to supply FFmpeg binary release b6.1.1. FFmpeg remains a
separate executable invoked for local media assembly/conversion.

Package source, license and binary provenance: https://github.com/eugeneware/ffmpeg-static

Binary release and associated source/build information: https://github.com/eugeneware/ffmpeg-static/releases/tag/b6.1.1

FFmpeg source and licensing: https://ffmpeg.org/download.html and https://ffmpeg.org/legal.html

The legacy Electron installer bundles Electron and dependency license files with
their upstream notices, plus FFmpeg's GPL text and build provenance under
`resources/licenses`. The Tauri application has not adopted these binaries or
dependencies yet.
Corresponding source/build material for this exact binary and its linked libraries
must be prepared before a public binary release; upstream links here alone are not
a completed redistribution audit. See docs/RELEASING.md.

# YouTube SABR transport

The browser-capture download path uses `googlevideo` 5.0.1, an MIT-licensed library for YouTube's UMP/SABR transport. Open Download Manager supplies its own browser-session capture, format selection, desktop handoff, file writing, muxing, and user interface.

Source and license: https://github.com/LuanRT/GoogleVideo

# YouTube extraction (legacy Electron preview)

The previously published Electron Windows build includes yt-dlp 2026.08.19 from
its official GitHub release,
SHA-256 `66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a`.
Source: https://github.com/yt-dlp/yt-dlp/tree/2026.08.19 . The core is public domain;
the Windows executable includes additional components under their own licenses.
Full upstream notices are included in `vendor/LICENSE.txt` and
`vendor/THIRD_PARTY_LICENSES.txt`, copied to the packaged `resources/tools` folder.
This is an independent extractor, not recovered IDM implementation code.

# Tauri media tool downloads

The Tauri application can install media tools on demand into the current user's
`.open-download-manager/tools` directory:

- **yt-dlp**: downloaded from the official `yt-dlp/yt-dlp` GitHub release for the
  host architecture (`yt-dlp.exe`, `yt-dlp_arm64.exe`, `yt-dlp_macos`,
  `yt-dlp_linux`, `yt-dlp_linux_aarch64`). Public domain core; bundled
  components keep their own licenses.
- **FFmpeg**:
  - Windows x64: GyanD `codexffmpeg` essentials build (GPL).
  - Windows ARM64: BtbN FFmpeg-Builds `winarm64` GPL build.
  - macOS: evermeet.cx release.
  - Linux x64: johnvansickle static build.
  - Linux ARM64: not downloaded automatically.

FFmpeg is a separate executable. Redistributors that bundle or mirror these
binaries are responsible for the corresponding source offer, license text, and
build provenance. The project does not currently ship FFmpeg or yt-dlp inside
the Tauri installer. See `docs/cross-platform-packaging.md` for current
resolution and licensing details.
