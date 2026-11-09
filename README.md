<p align="center">
  <img src="assets/app-icon.png" alt="Open Download Manager Logo" width="128" height="128" style="border-radius: 24px;" />
</p>

<h1 align="center">Open Download Manager</h1>

<p align="center">
  <b>A modern, high-performance, open-source download manager built with Rust & Tauri v2.</b><br>
  Original open-source implementation with dynamic multi-threaded acceleration, YouTube SABR streaming, and universal browser integration.
</p>

<p align="center">
  <a href="https://github.com/devenvoy/open-download-manager/releases/latest"><img src="https://img.shields.io/github/v/release/devenvoy/open-download-manager?color=2563EB&label=Release&style=flat-square" alt="Latest Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg?style=flat-square" alt="License: MIT" /></a>
  <a href="https://github.com/devenvoy/open-download-manager/actions/workflows/release.yml"><img src="https://img.shields.io/github/actions/workflow/status/devenvoy/open-download-manager/release.yml?branch=main&label=CI%20Build&style=flat-square" alt="CI Build Status" /></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/macOS-Apple%20Silicon%20%7C%20Intel-000000?style=flat-square&logo=apple&logoColor=white" alt="macOS" />
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20(x64%20%26%20ARM64)-0078D6?style=flat-square&logo=windows&logoColor=white" alt="Windows" />
  <img src="https://img.shields.io/badge/Linux-Ubuntu%20%7C%20Debian%20%7C%20AppImage-FCC624?style=flat-square&logo=linux&logoColor=black" alt="Linux" />
  <img src="https://img.shields.io/badge/Extensions-Chrome%20%7C%20Firefox%20%7C%20Edge%20%7C%20Brave-FF7139?style=flat-square&logo=googlechrome&logoColor=white" alt="Extensions" />
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2021-DEA584?style=flat-square&logo=rust&logoColor=black" alt="Rust" />
  <img src="https://img.shields.io/badge/Tauri-v2-FFC131?style=flat-square&logo=tauri&logoColor=black" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/Bun-v1.3-FBF0DF?style=flat-square&logo=bun&logoColor=black" alt="Bun" />
</p>

---

## ⚡ Key Features

- **🚀 Multi-Threaded Acceleration**: Dynamic file segment splitting with up to 32 concurrent chunks, active throughput re-balancing, and zero-byte thread recovery.
- **🎬 YouTube SABR & Adaptive Streaming**: Native parser for YouTube SABR / `videoplayback` streams with seamless automatic fallback to `yt-dlp` and paired audio/video muxing.
- **🧩 Native Browser Extensions**: Full integration for **Google Chrome, Microsoft Edge, Brave, Opera, Vivaldi** (Manifest V3), and **Mozilla Firefox** (Manifest V2).
- **🎥 In-Page Video Grabber**: Floating on-page video download bar with format selection (1080p, 4K, 60fps, audio-only) and keyboard accessibility.
- **🔄 Robust Pause & Resume**: Reliable transfer resumption across network dropouts, validating `ETag` and `Last-Modified` timestamps.
- **🎨 Dark & Light Theming**: Ergonomic IDM-inspired layout with system theme detection and high-contrast dark mode.
- **🌐 Comprehensive Proxy Support**: HTTP, HTTPS, and SOCKS5 proxy support with reachability testing and 1-click system proxy auto-import.
- **⏱️ Queues & Scheduling**: Named download queues, cron-style recurring schedules, and post-download power actions (sleep, shutdown).
- **🔒 Privacy First & Local**: Zero telemetry, no cloud accounts, and all credentials kept safely in local memory.

---

## 📦 Downloads & Installation

Get the latest installer for your operating system from the **[Releases Page](https://github.com/devenvoy/open-download-manager/releases/latest)**.

<details>
<summary><b>🍏 macOS (Apple Silicon & Intel)</b></summary>

<br>

1. Download the `.dmg` package matching your Mac:
   - **Apple Silicon (M1 / M2 / M3 / M4)**: [`open-download-manager_0.2.0_mac_arm64.dmg`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_mac_arm64.dmg)
   - **Intel 64-bit**: [`open-download-manager_0.2.0_mac_x64.dmg`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_mac_x64.dmg)
2. Open the `.dmg` file and drag **Open Download Manager** into your **Applications** folder.
3. Launch the app from Applications or Spotlight search.

</details>

<details>
<summary><b>🪟 Windows (10 / 11 — x64 & ARM64)</b></summary>

<br>

1. Download the setup executable matching your CPU:
   - **Windows 64-bit (x64)**: [`open-download-manager_0.2.0_win_x64-setup.exe`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_win_x64-setup.exe)
   - **Windows 11 ARM64**: [`open-download-manager_0.2.0_win_arm64-setup.exe`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_win_arm64-setup.exe)
2. Run the installer wizard and choose your options.
3. Launch **Open Download Manager** from the Start Menu or desktop shortcut.

</details>

<details>
<summary><b>🐧 Linux (Debian, Ubuntu, Fedora, Arch & AppImage)</b></summary>

<br>

- **Debian / Ubuntu / Linux Mint (`.deb`)**:
  ```bash
  sudo dpkg -i open-download-manager_0.2.0_lin_x64.deb
  sudo apt-get install -f
  ```
- **Universal Linux (`.AppImage`)**:
  ```bash
  chmod +x open-download-manager_0.2.0_lin_x64.AppImage
  ./open-download-manager_0.2.0_lin_x64.AppImage
  ```

</details>

<details>
<summary><b>🌐 Browser Extensions Setup (Chrome, Edge, Brave, Opera, Firefox)</b></summary>

<br>

### Chromium Browsers (Google Chrome, Microsoft Edge, Brave, Opera, Vivaldi)
1. Download [`open-download-manager_0.2.0_ext_chrome.zip`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_ext_chrome.zip).
2. Extract the ZIP file into a permanent directory.
3. In your browser, navigate to the Extensions management page:
   - Chrome: `chrome://extensions`
   - Edge: `edge://extensions`
   - Brave: `brave://extensions`
4. Enable **Developer mode** (toggle in the top-right corner).
5. Click **Load unpacked** and select the unzipped directory containing `manifest.json`.

### Mozilla Firefox
1. Download [`open-download-manager_0.2.0_ext_firefox.xpi`](https://github.com/devenvoy/open-download-manager/releases/download/v0.2.0/open-download-manager_0.2.0_ext_firefox.xpi).
2. In Firefox, open `about:addons` and click the gear icon (⚙️) in the upper-right corner.
3. Select **Install Add-on From File...** and choose the downloaded `.xpi` file.

</details>

---

## 🛠️ Optional Media Tools (`yt-dlp` & `ffmpeg`)

For downloading and merging high-definition adaptive YouTube streams:
- Open Download Manager automatically checks your system `PATH` for existing `yt-dlp` and `ffmpeg` installations.
- If not found, open **Options → Media Tools** and click **Install Media Tools**. The app will safely download and configure verified binaries to your local user directory (`~/.open-download-manager/tools/`).

---

## 💻 Building from Source

### Prerequisites
- [Rust](https://rustup.rs/) (edition 2021, latest stable)
- [Bun](https://bun.sh/) (v1.3+)
- Standard platform build tools (C compiler, Xcode tools on macOS, or WebView2 on Windows)

### Build & Run
```bash
# Clone the repository
git clone https://github.com/devenvoy/open-download-manager.git
cd open-download-manager

# Install project dependencies
bun install

# Start the desktop application in development mode
bun run dev

# Run full test suite (Rust unit tests & browser extension tests)
bun run test
```

### Packaging Release Bundles
```bash
bun run build:mac        # Build macOS DMG & .app
bun run build:win        # Build Windows NSIS installer (x64)
bun run build:linux      # Build Linux .deb & .AppImage
bun run pack:extension   # Build Chrome & Firefox extension packages
```

---

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!
- Check out the [Contributor Guide](docs/CONTRIBUTOR-START-HERE.md) for architecture details and branch guidelines.
- Review [SECURITY.md](SECURITY.md) for vulnerability reporting procedures.

---

## 📄 License

This project is licensed under the **[MIT License](LICENSE)**.  
Third-party licenses and component notices are detailed in **[THIRD-PARTY.md](THIRD-PARTY.md)**.
