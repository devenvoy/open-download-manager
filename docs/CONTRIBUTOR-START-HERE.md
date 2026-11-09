# Contributor Guide

Welcome to **Open Download Manager**! We welcome bug reports, feature suggestions, tests, documentation improvements, and code contributions.

---

## 1. Project Architecture

Open Download Manager is built modern, modular, and fast:
- **Desktop Runtime & Backend**: Rust with [Tauri v2](https://tauri.app) (`src-tauri/`).
- **UI / Frontend**: Clean, responsive HTML5, CSS3, and JavaScript (`renderer/`).
- **Browser Extensions**: WebExtensions (Manifest V3 for Chromium, Manifest V2 for Firefox) in `extension/`.
- **Native Messaging Host**: Rust binary (`src-tauri/src/bin/native_host.rs`) providing high-speed IPC between browsers and the desktop manager.

---

## 2. Development Setup

### Prerequisites
1. **Rust**: Latest stable toolchain (`rustup update stable`).
2. **Bun**: JavaScript runtime and test runner (`curl -fsSL https://bun.sh/install | bash`).
3. Platform-specific Tauri v2 prerequisites:
   - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
   - **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `build-essential`.
   - **Windows**: Microsoft C++ Build Tools & WebView2.

### Local Workflow
```bash
# Clone the repository
git clone https://github.com/devenvoy/open-download-manager.git
cd open-download-manager

# Install dependencies
bun install

# Run the desktop app in development mode
bun run dev

# Run all test suites (Rust unit tests + Browser extension tests)
bun run test
```

---

## 3. Contribution Workflow

1. **Pick or Propose an Issue**:
   Check our [GitHub Issues](https://github.com/devenvoy/open-download-manager/issues) for open tasks or bugs. Propose larger architectural changes in an issue before implementing them.
2. **Branch from `development`**:
   Create a descriptive feature branch (e.g. `feat/adaptive-speed-limit` or `fix/proxy-auth`).
3. **Write Tests**:
   Ensure all existing tests pass and add unit/regression coverage for any new features or bug fixes.
4. **Submit a Pull Request**:
   Target the `development` branch with a clear description of your changes and testing performed.
