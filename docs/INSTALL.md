# Installation and Setup Guide

Open Download Manager is available across macOS, Windows, and Linux, with native browser integration extensions for Chromium-based browsers and Firefox.

---

## 1. Desktop Application Installation

### macOS (Apple Silicon & Intel)
1. Download the latest `.dmg` installer from the [Releases page](https://github.com/devenvoy/open-download-manager/releases/latest):
   - **Apple Silicon (M1/M2/M3/M4)**: `open-download-manager_0.2.0_mac_arm64.dmg`
   - **Intel x86_64**: `open-download-manager_0.2.0_mac_x64.dmg`
2. Double-click the downloaded `.dmg` file.
3. Drag **Open Download Manager** into your **Applications** folder.
4. Launch Open Download Manager from Applications or Spotlight.

### Windows (10 / 11)
1. Download the setup executable from the [Releases page](https://github.com/devenvoy/open-download-manager/releases/latest):
   - **Windows 64-bit (x64)**: `open-download-manager_0.2.0_win_x64-setup.exe`
   - **Windows ARM64**: `open-download-manager_0.2.0_win_arm64-setup.exe`
2. Run the installer wizard and complete the setup.
3. Launch Open Download Manager from the Start Menu or desktop shortcut.

### Linux (Ubuntu, Debian, Fedora, Arch)
1. Download the package for your distribution from the [Releases page](https://github.com/devenvoy/open-download-manager/releases/latest):
   - **Debian / Ubuntu / Mint**: `open-download-manager_0.2.0_lin_x64.deb`
     ```bash
     sudo dpkg -i open-download-manager_0.2.0_lin_x64.deb
     sudo apt-get install -f # resolve any missing dependencies if needed
     ```
   - **Universal Linux AppImage**: `open-download-manager_0.2.0_lin_x64.AppImage`
     ```bash
     chmod +x open-download-manager_0.2.0_lin_x64.AppImage
     ./open-download-manager_0.2.0_lin_x64.AppImage
     ```

---

## 2. Browser Extension Setup

Open Download Manager communicates seamlessly with your browser using the Native Messaging protocol.

### Chromium Browsers (Google Chrome, Microsoft Edge, Brave, Opera, Vivaldi)
1. Download `open-download-manager_0.2.0_ext_chrome.zip` from the [Releases page](https://github.com/devenvoy/open-download-manager/releases/latest).
2. Unzip the archive into a permanent folder (e.g., `Documents/OpenDownloadManagerExtension`).
3. Open your browser's extensions manager:
   - Chrome: `chrome://extensions`
   - Edge: `edge://extensions`
   - Brave: `brave://extensions`
   - Opera: `opera://extensions`
4. Toggle on **Developer mode** in the top-right corner.
5. Click **Load unpacked** and select the unzipped extension directory (the folder containing `manifest.json`).
6. Open Open Download Manager desktop app, navigate to **Options → Browser Integration**, and confirm your browser shows as **Connected**.

### Mozilla Firefox
1. Download `open-download-manager_0.2.0_ext_firefox.xpi` from the [Releases page](https://github.com/devenvoy/open-download-manager/releases/latest).
2. In Firefox, open `about:addons` (or press `Ctrl+Shift+A` / `Cmd+Shift+A`).
3. Click the gear icon (⚙️) in the top-right corner and select **Install Add-on From File...**.
4. Select the downloaded `.xpi` file and click **Add** when prompted.

---

## 3. Media Tools Integration (`yt-dlp` & `ffmpeg`)

For downloading and merging adaptive YouTube streams (SABR / 1080p+ videoplayback):
1. In Open Download Manager, navigate to **Options → Media Tools**.
2. If `yt-dlp` or `ffmpeg` is already in your system PATH, the app will auto-detect it.
3. If not installed, click **Install Media Tools** to automatically download official, verified binaries to your local directory (`~/.open-download-manager/tools/`).
