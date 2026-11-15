# Release Notes — v0.3.0

Open Download Manager **v0.3.0** is a major milestone release introducing a fully migrated, high-performance desktop interface built on **React 19, TypeScript, Vite, Tailwind CSS, and shadcn/ui**, an embedded **BitTorrent engine**, an integrated **71-icon file-type catalog**, and an upgraded **media pipeline** supporting YouTube SABR and native macOS QuickTime compatibility.

---

## 🌟 Highlights

### 1. Modernized Frontend Stack (React 19 + TypeScript + Vite)
- **Modular Component Architecture**: Replaced hand-written monolithic web assets with feature-sliced React 19 components (`renderer/src/features/`) under strict 300-line modularity rules.
- **Tailwind CSS & Scoped Styling**: Deprecated legacy `styles.css` and `torrents.css`, unifying all design tokens and responsive dialog styling in `index.css`.
- **Zero Placeholder Guarantee**: Full 1:1 feature parity across all seven application windows (Main Downloads, Add Download, Options/Settings, Transfer Progress, Scheduler, Browser Integration, and Torrents).

### 2. Built-in BitTorrent Subsystem
- **Complete Torrent Client**: Native engine with support for magnet links, `.torrent` file parsing, DHT peer discovery, tracker communication, and seed/leech limits.
- **Partitioned File Browser**: Modernized Files tab with distinct **Downloaded** (featuring complete badges and checkmarks) and **Available to Download** sections with **Select All / Deselect All** controls.
- **Dynamic Window Splitter**: Resizable split pane between the torrent list and detail panels with localStorage persistence.
- **Streamlined Status & Ratio Metrics**: Cleaned up subtitle and stats displays—progress bar tracks hide once downloaded, and ratio counters only show when active seeding data exists.

### 3. Integrated File-Type Icon Catalog
- **71 Native SVG Catalog Icons**: Deployed an extensive desktop icon pack covering video, audio, compressed archives, documents, code, spreadsheets, executables, and packages.
- Integrated across both the Main Downloads table and the Torrents manager.

### 4. Resilient Media Download & QuickTime Compatibility
- **Native QuickTime Compatibility**: FFmpeg muxing encodes audio tracks to standard AAC (192k) with `mov_text` subtitle tracks, preventing macOS playback incompatibility warnings.
- **Multi-Track Subtitles & Audio**: Embedded metadata, chapter marks, and all available subtitle tracks with ISO-639 language tags and human-readable track names (e.g. English, Spanish, Japanese) instead of generic "track 0".
- **Direct yt-dlp Format Selection**: User-selected formats (resolutions, audio-only, 4K) are directly respected and downloaded through the integrated toolchain.

---

## 📦 Release Assets

| Platform | Architecture / Package | Filename | Type |
| :--- | :--- | :--- | :--- |
| **macOS** | Apple Silicon (M1/M2/M3/M4) | `open-download-manager_0.3.0_mac_arm64.dmg` | DMG Installer |
| **macOS** | Apple Silicon App Archive | `open-download-manager_0.3.0_mac_arm64.app.tar.gz` | App Archive |
| **macOS** | Intel 64-bit | `open-download-manager_0.3.0_mac_x64.dmg` | DMG Installer |
| **macOS** | Intel App Archive | `open-download-manager_0.3.0_mac_x64.app.tar.gz` | App Archive |
| **Windows** | Windows 10/11 64-bit (x64) | `open-download-manager_0.3.0_win_x64-setup.exe` | NSIS Installer |
| **Windows** | Windows 11 ARM64 | `open-download-manager_0.3.0_win_arm64-setup.exe` | NSIS Installer |
| **Linux** | Ubuntu / Debian 64-bit | `open-download-manager_0.3.0_lin_x64.deb` | Debian Package |
| **Linux** | Universal Linux 64-bit | `open-download-manager_0.3.0_lin_x64.AppImage` | AppImage |
| **Browser** | Chrome / Edge / Brave / Opera | `open-download-manager_0.3.0_ext_chrome.zip` | ZIP Extension |
| **Browser** | Firefox Add-on | `open-download-manager_0.3.0_ext_firefox.xpi` | Installable XPI |
| **Browser** | Firefox Source | `open-download-manager_0.3.0_ext_firefox.zip` | Source ZIP |
| **Checksums** | SHA-256 Hashes | `open-download-manager_0.3.0_checksums.txt` | Text |

---

## 🔒 Verification & Provenance

Release assets are signed with GitHub's Sigstore key. You can verify the integrity and provenance of any downloaded binary using the GitHub CLI:

```bash
gh attestation verify <binary-file> --repo devenvoy/open-download-manager
```
