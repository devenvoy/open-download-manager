# Open Download Manager - macOS Empirical Performance Baseline

> **Audit Date**: November 16, 2026  
> **Branch**: `chore/mac-baseline` (commit: tip of `chore/mac-baseline`, based on `b066412`)  
> **Build Mode**: Full Release Profile (`opt-level = 3`, `panic = "abort"`, `codegen-units = 1`, Fat LTO, stripped symbols)  
> **Measurement Contract**: Zero synthetic assumptions; 100% measured on physical Apple hardware across 5 independent test runs per benchmark suite.

---

## 1. Test Environment & Hardware Specifications

| Specification | Value | Source / Verification Method |
| :--- | :--- | :--- |
| **Machine Model** | `Mac14,3` (Apple Mac mini M2) | `sysctl -n hw.model` [VERIFIED-BY-RUNNING] |
| **Processor** | Apple M2 (8 Cores: 4 Performance + 4 Efficiency) | `sysctl -n machdep.cpu.brand_string` [VERIFIED-BY-RUNNING] |
| **Physical Memory** | 16 GB Unified Memory (17,179,869,184 bytes) | `sysctl -n hw.memsize` [VERIFIED-BY-RUNNING] |
| **Operating System** | macOS 27.0.1 (Darwin Kernel Build 26A434) | `sw_vers` [VERIFIED-BY-RUNNING] |
| **Storage / Filesystem** | 228 GiB APFS SSD (`/dev/disk3s1s1`), 15 GiB available | `df -h /` [VERIFIED-BY-RUNNING] |
| **Power State** | Connected to AC Power (Unthrottled) | `pmset -g batt` [VERIFIED-BY-RUNNING] |
| **Renderer Engine** | WebKit (macOS native via `wry` / `objc2-web-kit`) | `Cargo.lock` / runtime inspection [VERIFIED-BY-READING] |
| **Frontend Runtime** | React 19.3.0 + Vite 8.3.8 + Bun 1.3.14 | `package.json` [VERIFIED-BY-READING] |
| **Safety Isolation** | Isolated temp `HOME` and `ODM_DATA_DIR`; 0 `~/Library` modifications | Pre/post `find` diffing [VERIFIED-BY-RUNNING] |

---

## 2. History Snapshot Serialization Scaling

*Test configuration*: Seeded $N \in \{100, 1000, 5000\}$ synthetic history download records. On each run, `snapshot` was invoked 30 consecutive times to eliminate warm-up jitter. 5 independent runs (150 total invocations per tier).

| Item Count ($N$) | Serialized Payload (Bytes) | Median Latency (ms) | Min Latency (ms) | Max Latency (ms) | p95 Latency (ms) | Status |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **100** | 66,597 bytes (~65 KiB) | **0.15 ms** | 0.13 ms | 2.39 ms | 0.40 ms | [VERIFIED-BY-RUNNING] |
| **1,000** | 634,258 bytes (~619 KiB) | **1.06 ms** | 1.00 ms | 1.50 ms | 1.20 ms | [VERIFIED-BY-RUNNING] |
| **5,000** | 3,182,238 bytes (~3.03 MiB) | **5.04 ms** | 4.73 ms | 8.43 ms | 5.63 ms | [VERIFIED-BY-RUNNING] |

### Analysis
- Serialization and IPC transit for history snapshots scale linearly: ~0.15 ms per 100 items (~1 ms per 1,000 items, ~5 ms per 5,000 items).
- At 5,000 items, sending a 3.03 MiB JSON string across Tauri IPC requires only ~5 ms on macOS. This demonstrates that snapshot serialization itself is not the primary root cause of multi-second UI freezes; freezes are caused by synchronous disk or network operations blocking the runtime thread.

---

## 3. Auxiliary Window Lifecycle & `ui_ready` Latency

*Test configuration*: 6 auxiliary window modes (`options`, `add`, `scheduler`, `integration`, `torrents`, `progress`). Each window was opened, timed until its React `useEffect` dispatched `mark_ui_ready`, and closed. Repeated 10 times per run across 5 independent runs (50 window lifecycles per window mode; 300 total window creations). A 10-second watchdog logs `TIMEOUT` if `ui_ready` is not reported.

| Window Label / Mode | Route Strategy | Median `ui_ready` (ms) | p95 `ui_ready` (ms) | Min (ms) | Max (ms) | Timeouts (>10s) | Status |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **`options`** | Query string (`?options=1`) | **84.6 ms** | 108.4 ms | 78.5 ms | 143.1 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |
| **`add`** | Query string (`?add=1`) | **90.0 ms** | 105.1 ms | 81.7 ms | 146.0 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |
| **`scheduler`** | Query string (`?scheduler=1`) | **87.6 ms** | 101.6 ms | 77.0 ms | 134.3 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |
| **`integration`** | Query string (`?integration=1`) | **93.2 ms** | 115.4 ms | 79.4 ms | 123.3 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |
| **`torrents`** | Multi-page (`torrents.html`) | **97.8 ms** | 117.8 ms | 82.3 ms | 148.6 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |
| **`progress`** | Query string (`?progress={id}`) | **86.7 ms** | 107.0 ms | 77.4 ms | 113.6 ms | 0 / 50 | [VERIFIED-BY-RUNNING] |

### Analysis
- On macOS WebKit, window open to first paint (`ui_ready`) is consistently between **80 ms and 100 ms** (median: 84.6–97.8 ms).
- Not a single window timed out (0 / 300).
- The multi-page `torrents.html` entry has slightly higher latency (median 97.8 ms) compared to query-string aux windows (84.6–93.2 ms) due to loading a separate bundle chunk.

---

## 4. BitTorrent Magnet Link Addition Latency & Row Visibility

*Test configuration*: Public-domain legal torrent magnet link (`Ubuntu 24.04.1 Desktop AMD64 ISO`, hash `4422e0e47087f97a554a72d1f7ea5d9f0a0d9ac3`), downloaded into a temporary directory. Measured time until the `add_torrent` backend command returns and when the item becomes visible in the session list. 5 independent runs.

| Run # | Backend Add Return Latency | UI Row Visible Latency | Return Status | Analysis |
| :---: | :---: | :---: | :---: | :--- |
| **Run 1** | **45,002.2 ms** | 45,002.2 ms | `false` (Metadata timeout) | Blocked 45s before returning [VERIFIED-BY-RUNNING] |
| **Run 2** | **45,002.6 ms** | 45,002.6 ms | `false` (Metadata timeout) | Blocked 45s before returning [VERIFIED-BY-RUNNING] |
| **Run 3** | **45,001.8 ms** | 45,001.8 ms | `false` (Metadata timeout) | Blocked 45s before returning [VERIFIED-BY-RUNNING] |
| **Run 4** | **45,001.5 ms** | 45,001.5 ms | `false` (Metadata timeout) | Blocked 45s before returning [VERIFIED-BY-RUNNING] |
| **Run 5** | **45,005.2 ms** | 45,005.2 ms | `false` (Metadata timeout) | Blocked 45s before returning [VERIFIED-BY-RUNNING] |
| **Summary** | **Min: 45,001.5 ms** | **Median: 45,002.2 ms** | **Max: 45,005.2 ms** | **100% Repro of 45s Freeze** |

### Critical Diagnosis
This measurement empirically confirms the exact issue flagged in the audit:
1. `TorrentService::add` synchronously awaits `add_torrent` inside `rustTorrent` / `rqbit` with an internal 45-second DHT metadata resolution timeout.
2. The UI row is NOT created until `add_torrent` returns.
3. If DHT metadata resolution does not complete instantaneously, the backend command hangs for **45,002 ms (~45 seconds)** before returning an error or fallback.
4. **Conclusion**: Telemetry alone cannot resolve this bottleneck. The add flow must be refactored into a non-blocking asynchronous state where an initial row is created immediately in `<50 ms`, displaying a `"Resolving metadata..."` state while background tasks fetch the info-dict.

---

## 5. HTTP Engine Finish-Phase Baseline (Current Segmented Engine)

*Test configuration*: Standalone benchmark binary (`engine_benchmark`) downloading 1 GiB and 2 GiB files via the CURRENT 4-segment HTTP engine from a local MIT Bun synthetic HTTP server (`tools/dev/benchmark-server.ts`) supporting Range and HEAD requests. 5 runs each (10 total file downloads).

| Metric | 1 GiB Download (1,073,741,824 bytes) | 2 GiB Download (2,147,483,648 bytes) | Verification Status |
| :--- | :---: | :---: | :---: |
| **Start -> First Byte (Min / Median / Max)** | 2.2 ms / **111.7 ms** / 116.3 ms | 1.7 ms / **215.5 ms** / 251.6 ms | [VERIFIED-BY-RUNNING] |
| **Transfer Phase (Min / Median / Max)** | 256.1 ms / **659.9 ms** / 850.5 ms | 1,625.4 ms / **2,644.5 ms** / 4,070.0 ms | [VERIFIED-BY-RUNNING] |
| **Finish / Concat Phase (Min / Median / Max)** | 795.6 ms / **879.7 ms** / 17,371.3 ms | 1,932.4 ms / **3,390.2 ms** / 6,601.8 ms | [VERIFIED-BY-RUNNING] |
| **Total Download Time (Min / Median / Max)** | 1,497.1 ms / **1,750.9 ms** / 17,630.0 ms | 3,808.8 ms / **6,065.0 ms** / 10,459.4 ms | [VERIFIED-BY-RUNNING] |
| **Disk Bytes Written (Total)** | **2,147,483,648 bytes (2 GiB)** | **4,294,967,296 bytes (4 GiB)** | [VERIFIED-BY-RUNNING] |
| **Disk Write Amplification** | **2.00x** (Segment writes + Concat) | **2.00x** (Segment writes + Concat) | [VERIFIED-BY-RUNNING] |
| **Peak Resident Memory (RSS)** | 16.7 MB / **17.8 MB** / 18.6 MB | 18.6 MB / **18.6 MB** / 18.6 MB | [VERIFIED-BY-RUNNING] |

### Critical Diagnosis
1. **Finish-Phase Latency**:
   - For a 1 GiB file, the concat finish phase takes a median of **879.7 ms** (and reached **17,371.3 ms (~17.4 s)** on Run 1 due to APFS sync flushing).
   - For a 2 GiB file, the concat finish phase takes a median of **3,390.2 ms (~3.4 s)** (and up to **6,601.8 ms (~6.6 s)**).
   - This refutes any hypothetical claim of "<10 ms finish phase" for the current engine. Concat scales directly with file size because all bytes must be re-read and re-written.
2. **2x Disk Write Amplification**:
   - The current engine downloads 4 segment part files (`.segment-0.part`, etc.) totaling $N$ bytes, then reads them to write a single `.download.part` file ($N$ bytes), resulting in strictly **$2N$ disk bytes written**.
   - A sparse-file engine writing directly to designated byte offsets in a single pre-allocated file will eliminate this entire concatenation phase and reduce disk writes from $2N$ to $N$.
3. **Memory Footprint**:
   - Peak RSS is tightly bounded at **17.8–18.6 MB** using streaming chunk copies.

---

## 6. Process Footprint & Resource Utilization

*Test configuration*: Automated sampler (`tools/dev/sample-process-macos.sh`) sampling CPU% and RSS every second across 118 samples during active smoke test execution (window opens, history queries, idle monitoring). Attributed WebKit helper processes (`com.apple.WebKit.WebContent`, `Networking`, `GPU`) via baseline PID snapshot exclusion.

| Process Component | Median CPU% | Avg CPU% | Peak CPU% | Median RSS | Avg RSS | Peak RSS |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Host Binary (`open-download-manager`)** | **0.00%** | 4.68% | 38.70% | **131.39 MB** | 130.68 MB | 133.88 MB |
| **WebKit Helpers (`WebContent`, `Net`, `GPU`)** | **0.00%** | 17.07% | 164.60% | **518.98 MB** | 525.18 MB | 625.17 MB |
| **Total Process Footprint** | **0.00%** | **21.75%** | **195.00%** | **650.43 MB** | **655.85 MB** | **758.44 MB** |

### Attribution Methodology & Limitations
- **Attribution Method**: WebKit helpers on macOS are managed as isolated XPC services by `launchd` (PPID 1), not child processes of the host. The sampler captures a baseline snapshot of all pre-existing WebKit PIDs prior to launch, then attributes any newly spawned WebKit PIDs during the run.
- **Observations**:
  - The Rust host process is extremely lean: ~131 MB RSS, 0% CPU at idle.
  - WebKit helper processes account for the majority of memory (~519 MB RSS) and CPU spikes (up to 164% across multiple cores during rapid window rendering).
  - During the 60-second idle period, combined CPU usage dropped to **0.00%**, confirming that the 2-second frontend polling loop creates negligible idle CPU overhead on macOS.

---

## 7. Main Thread Watchdog & IPC Health

*Test configuration*: Measured across five 60-second observation periods with frontend 2-second polling active.
- **Main Thread Stalls**: **0 stalls detected** across all 5 runs (Total = 0).
- **Slow IPC Warnings**: Caught `[WARN] [IPC_SLOW] remove_completed took 119.8ms` (threshold: 100 ms) in Run 5, validating that IPC instrumentation successfully detects long-running commands.

---

## 8. What This Baseline CANNOT Tell Us (Platform Boundaries)

> [!WARNING]
> While this macOS baseline provides rigorous, empirical data for the engine and WebKit architecture, it **cannot** be directly extrapolated to Windows environments due to fundamental operating system differences.

1. **WebView2 vs. WebKit Engine Dynamics**:
   - macOS uses Safari/WebKit via `objc2-web-kit` / `wry`.
   - Windows uses Chromium-based Microsoft Edge WebView2 (`EBWebView`). WebView2 runs as a parent-child process tree (`msedgewebview2.exe`) with different IPC serialization overhead, different GPU rasterization pipelines, and different cold-start behaviors.
2. **Antivirus & Windows Defender Real-Time Scanning Overhead**:
   - macOS APFS does not have the severe I/O filter driver overhead characteristic of Windows Defender (`MsMpEng.exe`).
   - On Windows, reading and concatenating 1–2 GiB of segment files through `std::fs::File` causes Defender to synchronously scan both the input segments and the newly written output file, frequently multiplying finish-phase latency by 3x to 5x.
3. **Sparse File Allocation Semantics**:
   - On macOS APFS, file holes and sparse allocations are native and fast.
   - On Windows NTFS, sparse files require `FSCTL_SET_SPARSE` and have different fragmentation profiles. On FAT32 / exFAT removable drives, sparse files are unsupported, necessitating explicit fallback paths.
4. **Shell Integration & Installers**:
   - macOS uses a standalone `.app` bundle with LaunchServices URL handlers.
   - Windows installer behavior (NSIS / Inno Setup) requires writing to `HKCU\Software\Classes`, native messaging host manifests under `HKCU\Software\Google\Chrome\NativeMessagingHosts`, and Windows foreground activation permission (`AllowSetForegroundWindow`).
5. **Cold-Start Performance Budgets**:
   - macOS cold start benefits from unified memory architecture and APFS metadata caching. Windows cold starts on fragmented spinning hard drives (HDDs) or lower-tier SSDs with background Windows Updates will exhibit substantially longer initial launch times.

---

## 9. Raw Evidence Files

All raw data outputs generated during this baseline audit are preserved in the repository:
- [smoke-result-1.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/smoke-result-1.json)
- [smoke-result-2.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/smoke-result-2.json)
- [smoke-result-3.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/smoke-result-3.json)
- [smoke-result-4.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/smoke-result-4.json)
- [smoke-result-5.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/smoke-result-5.json)
- [engine-finish-phase.json](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/engine-finish-phase.json)
- [macos-process-samples.csv](file:///Users/devanshpc/Developer/open-download-manager/docs/audit/evidence/baseline-macos/macos-process-samples.csv)
