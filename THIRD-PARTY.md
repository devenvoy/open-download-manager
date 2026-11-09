# Third-Party Notices & Licensing

Open Download Manager is open-source software licensed under the [MIT License](LICENSE).

This document lists the third-party components, frameworks, and optional external tools utilized by Open Download Manager.

---

## 1. Core Framework & Runtime

- **Tauri v2** (`https://tauri.app`):
  Licensed under Apache License 2.0 or MIT License.
  Used as the desktop application container and native OS bridge.

- **Rust Ecosystem Dependencies**:
  Core network, serialization, and concurrency capabilities utilize standard open-source Rust crates (including `tokio`, `reqwest`, `serde`, `flate2`, and `image`) distributed under permissive licenses (MIT, Apache-2.0, or BSD). Full dependency specifications are defined in [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml).

---

## 2. Browser Extensions

- **Chromium & Firefox Integration**:
  The browser extensions located in [`extension/`](extension/) are original code written for Open Download Manager and distributed under the project's [MIT License](LICENSE).
  Uses standard W3C WebExtensions APIs and the native messaging protocol.

- **YouTube SABR Transport**:
  The SABR stream processor references YouTube's UMP transport specifications adapted under the MIT License (`https://github.com/LuanRT/GoogleVideo`).

---

## 3. Optional On-Demand Media Tools

Open Download Manager does **not** bundle binary media tools directly in the application installer. To process complex adaptive media streams, the application can resolve these tools from your system `PATH` or download official releases on demand into your local user tools directory (`~/.open-download-manager/tools`):

- **yt-dlp** (`https://github.com/yt-dlp/yt-dlp`):
  - **License**: The Unlicense (Public Domain dedication).
  - Used optionally for adaptive stream format extraction and fragment retrieval.

- **FFmpeg** (`https://ffmpeg.org`):
  - **License**: GNU Lesser General Public License (LGPL) version 2.1 or later, or GNU General Public License (GPL) version 2 or later, depending on the build configuration.
  - Used strictly as an external CLI process for local multiplexing and format conversion.
