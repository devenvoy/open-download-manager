# Step 1: Study of Mister Download Manager (MDM)

Date: 2026-11-16
Reference Repository: `reference/misterdownloadmanager` (`https://github.com/anefzaoui/misterdownloadmanager`)
License: **GPL-3.0-or-later** (strict copyleft; cannot be copied into MIT codebase)
Target OS: Windows (only officially tested platform; macOS/Linux explicitly marked experimental/untested by author)

---

## 1. Supply-Chain & Safety Audit (Rule 2)

Per Rule 2 ("Safety: do not install MDM's release binaries, installer or browser extension on this machine. Build from source in a VM or Windows Sandbox only. Skim Cargo.lock, build.rs and the dependency forks for supply-chain red flags"):

### Build & Execution Safety Boundary
- Host environment is macOS Darwin ARM64 (`Devanshs-Mac-mini.local`). No Docker, Podman, or Windows Sandbox is present on this host.
- Per Rule 2, MDM build and test execution was **not** run directly on the host machine to eliminate arbitrary code execution risks outside an isolated sandbox.
- All MDM findings are rigorously tagged **VERIFIED-BY-READING** with exact source file paths and line ranges, and benchmark claims are verified from their published test reports and harness code.

### Supply-Chain Inspection
1. **License**: GPL-3.0-or-later (`Cargo.toml:9`). Incompatible with our MIT license. No code or assets can be copied; all improvements must be clean-room reimplementations in our stack.
2. **`build.rs` Audit**:
   - Only one `build.rs` exists in the entire workspace: `reference/misterdownloadmanager/crates/app/build.rs`.
   - Functionality: Packs TSV translation catalogs into a compressed Brotli stream (`catalogs.br`), emits build commit, sets Windows subsystem version `/SUBSYSTEM:WINDOWS,5.02` for MSVC, and compiles Windows icon resources (`winresource`).
   - No remote fetches, no dynamic script execution, no obfuscated payloads.
3. **Dependency Forks & `[patch.crates-io]`**:
   MDM replaces upstream crates with 4 major personal/partner forks:
   - `anefzaoui/rqbit.git` (rev `fab046042915709ab160626ebc59cce08de4d779`): Adds dynamic rate limit modification, metadata progress telemetry, socket interface binding (`bind_ip`), custom TLS client builder, and IPv6 DHT bootstrap handling.
   - `anefzaoui/egui.git` (rev `268d8e55dae56a536b672bdb75987021931dfcb2`): Custom patches for AccessKit screen readers on hidden windows, wgpu D3D12/Direct3D integration, swap interval controls, and fallback fonts.
   - `anefzaoui/wgpu.git` (rev `1c6f336e239ff7bc46b4dec194f1d96d3d4f8e04`): Forces `DXGI_SCALING_NONE` on D3D12 swapchains to avoid window resizing distortion.
   - `storytold/winit.git` (rev `a517b3846ee8457d84ee4975110bfda6797c44dc`) and `storytold/craft-libs`: Atomic file operations and translation parser.
   - `wreq = "=0.16.3"`: Browser-like TLS/HTTP fingerprinting engine. Compiles BoringSSL from source (requires CMake, C++ compiler, libclang, and NASM).
4. **Assessment**:
   The dependency forks are maintained specifically to work around shortcomings in egui, wgpu, and librqbit on Windows. No trojans or spyware signatures were found. However, maintaining 4 deeply branched forks of core GUI and graphics libraries creates a high maintenance burden and fragility.

---

## 2. Workspace Crate Architecture

The MDM workspace consists of 15 modular Rust crates:

| Crate | Path | Core Role |
|---|---|---|
| `mdm-engine` | `crates/engine` | HTTP/FTP download engine: probing, pure segment planner, worker tasks, single sparse part file, rate limiter, checksums, proxy/PAC |
| `mdm-app` | `crates/app` | Desktop UI in pure Rust using `egui` + `eframe` (Direct3D 12 on Windows, OpenGL fallback, experimental Metal on macOS) |
| `mdm-cli` | `crates/cli` | Headless CLI supporting `get`, `resume`, `video-get`, `torrent-get`, automation commands, and IDM-style switches (`/d`, `/p`, `/f`, `/a`, `/n`, `/s`) |
| `mdm-mcp` | `crates/mcp` | Standard Model Context Protocol (MCP) server over stdio for AI agent control (Claude, Cursor, Antigravity) |
| `mdm-remote` | `crates/remote` | HTTPS remote control server on LAN with pairing codes, QR codes, and device tokens |
| `mdm-nmhost` | `crates/nmhost` | Browser native messaging host connecting extension to desktop app via local IPC socket |
| `mdm-ipc` | `crates/ipc` | Local command protocol, token-authenticated TCP socket, capture rules, and extension IDs |
| `mdm-debrid` | `crates/debrid` | Native integration for Real-Debrid, AllDebrid, Premiumize, TorBox, Mega-Debrid APIs |
| `mdm-cnl` | `crates/cnl` | Click'n'Load 2 listener on `127.0.0.1:9666` |
| `mdm-extract` | `crates/extract` | In-process extraction for zip, tar, gz, xz; delegates 7z and rar to 7-Zip |
| `mdm-geo` | `crates/geo` | Offline IP-to-country lookup based on DB-IP Lite with country flags |
| `mdm-tools` | `crates/tools` | yt-dlp, ffmpeg, and gallery-dl orchestration, pre-extracted folder management, HLS/DASH fragment downloader |
| `mdm-crash` | `crates/crash` | Out-of-process crash monitor, freeze detection (8s watchdog), minidumps via Mozilla Breakpad |
| `mdm-torrent` | `crates/torrent` | BitTorrent engine wrapper around librqbit fork with IP blocklists and tracker SSRF filtering |
| `mdm-labserver` | `crates/labserver` | Test server simulating HTTP Range, chunk drops, rate throttling, 429 back-off, and FTPS |

---

## 3. Analysis of Benchmark Run 5 (`docs/bench/run5/REPORT.md`)

Hardware: 13th Gen Intel Core i5-13400F, 64 GB RAM, Windows 11.
Harness: `mdm-labserver` on `127.0.0.1:8097`, comparing IDM 6.42 vs MDM (8 connections default).

### Raw Benchmark Data
| Scenario | IDM Median (ms) | MDM Median (ms) | IDM / MDM Ratio |
|---|---|---|---|
| 1 GiB fast local link | 5,456 ms | 1,009 ms | **5.41×** |
| 100 MiB (2 MB/s per conn) | 10,234 ms | 6,319 ms | **1.62×** |
| 100 MiB (150 ms latency, 4 MB/s) | 7,622 ms | 3,844 ms | **1.98×** |
| 32 MiB (1 slow connection in 3) | 7,213 ms | 3,432 ms | **2.10×** |
| 50 MiB (server allows 2 conns) | 15,864 ms | 12,568 ms | **1.26×** |
| 50 MiB (every 2nd conn cut after 2 MB) | 5,428 ms | 1,647 ms | **3.30×** |
| 256 KiB fast local link | 3,141 ms | 30 ms | **105.9×** |

### Phase Deconstruction (Startup vs Transfer vs Finish)
Examining the phase breakdown reveals the underlying truth behind the numbers:
1. **Fixed Startup Latency in IDM**:
   - In **every single test**, IDM recorded a **3,063 ms to 3,090 ms** startup phase!
   - MDM recorded a **13 ms to 14 ms** startup phase.
   - For the 256 KiB file, IDM transfer time was 25 ms, but its startup was 3,090 ms (total 3,141 ms). MDM transfer was 7 ms + 14 ms startup (total 30 ms). The "105× faster" claim is entirely an artifact of launching IDM cold via CLI in the benchmark harness!
2. **Pure Transfer Phase Performance**:
   - In network-throttled tests (`capped-100m`), IDM transfer was 7,068 ms vs MDM 6,299 ms (only a **1.12×** transfer speedup).
   - In high-latency tests (`latency-100m`), IDM transfer was 4,451 ms vs MDM 3,801 ms (a **1.17×** transfer speedup).
   - On the fast 1 GiB local link, MDM transfer was 438 ms vs IDM 1,831 ms (a **4.18×** transfer speedup). This is directly attributable to MDM's single sparse file write model versus IDM's chunk concatenation.
3. **Adaptive Connection Limiting**:
   - In `maxconns-50m` (server allows 2 connections, returns 429 on more), IDM made 480 aggressive connection requests, repeatedly hitting rate limits and backing off. MDM learned after 2 refusals that the server caps at 2, and made exactly 2 connections total.
4. **Memory Footprint**:
   - IDM peak RAM: **43 to 48 MB**.
   - MDM peak RAM: **96 to 106 MB** (>2× higher memory consumption due to egui and GPU rasterization buffers).

---

## 4. Open Issue #1240 Analysis

Issue: `anefzaoui/misterdownloadmanager#1240`: *"From comparative testing: short-term fixes, and where MDM could leap ahead"* (opened Nov 16, 2026).

Key technical takeaways:
1. **Adaptive Throughput-Optimal Connections**:
   - Fixed 8 or 16 connections are often sub-optimal. Big CDNs (Cloudflare, GitHub) saturate at 1-4 connections; per-connection capped servers need 8-16.
   - Recommendation: Start conservatively, ramp up only if throughput improves, back off on 429/503. MDM owner agreed and confirmed that MDM currently only implements the back-off half in `hosts.rs`.
2. **Expired Link Renewal for Manual Links**:
   - Browser-intercepted downloads track `page_url` and `referer` to renew expired signed links upon resume.
   - Pasted/manual downloads currently fail if the signed URL expires. Proposes keeping referer/source page for pasted links.
3. **CLI Video-get Quality Fallback**:
   - The GUI falls back to nearest available resolution if requested format is missing; the CLI previously threw an unhandled error.
4. **Multi-Mirror Aggregation & Verified Completion**:
   - Proposes aggregating downloads across multiple mirror URLs (aria2 style) and verifying SHA256 against published digests (GitHub releases, HuggingFace LFS), re-downloading only corrupted ranges.
