# Torrent Support — Plan

**Status:** Draft / not started
**Created:** 2026-11-11
**Scope:** Add BitTorrent (magnet, `.torrent`, download, seed, leech) as an
isolated subsystem with its own window and home-screen tab.
**Constraint:** The existing HTTP/FTP/MMS/media download flow must not change.

---

## 1. Summary

Open Download Manager currently handles finite, URL-addressed transfers through
`DownloadTransferService` and a `DownloadExecutor` implementation. BitTorrent is
a different product: swarm discovery, multi-file payloads, long-lived seeding,
peer management, and persistent piece state.

This plan adds torrents as a **separate feature island**:

- A dedicated **Torrents window** with the full client UI.
- A **Torrents tab** on the home screen that shows status and opens the window.
- A separate backend service, separate settings store, separate persistence,
  separate Tauri commands, and separate frontend assets.
- Zero changes to `DownloadRecord`, download history, the transfer scheduler,
  `CompositeDownloadExecutor`, or the existing renderer download logic.

The engine is `librtbit` from rustTorrent, published on crates.io as
`swarmforge` (Apache-2.0). It is embedded in-process behind a Cargo feature flag
so it can be disabled or removed without touching the download manager.

---

## 2. Goals

| # | Goal |
|---|------|
| G1 | Add magnet links and `.torrent` files from paste, file picker, OS open, and browser. |
| G2 | Fetch metadata, select files, download, pause, resume, recheck, remove. |
| G3 | Seed and leech with visible upload/download speed, peers, seeds, ratio. |
| G4 | Survive app restart with engine state intact (fastresume). |
| G5 | Run independently of the download queue and concurrency limiter. |
| G6 | Keep the torrent subsystem removable via one Cargo feature flag. |

## 3. Non-goals (this plan)

- No torrent rows in the existing downloads table.
- No torrents in download history JSON, categories, or named queues.
- No torrent entries in the existing completion/sleep/shutdown action ledger.
- No qBittorrent-compatible server, web UI, RSS automation, or streaming.
- No changes to `src-tauri/src/domain/download/**`.
- No changes to `src-tauri/src/application/download_transfer_service/**`.
- No changes to `src-tauri/src/infrastructure/media/composite_executor.rs`.

Those can be layered later if the feature proves out.

---

## 4. Product shape

### 4.1 Home screen tab

Add a top-level view switch in the main window. Downloads remains the default
view and keeps its current behavior.

```
┌──────────────────────────────────────────────────────────────┐
│ [Add URL] [Resume] [Stop] ...        [Downloads] [Torrents]  │
├───────────────┬──────────────────────────────────────────────┤
│ Categories    │  Torrents                                    │
│  All          │  ┌────────────────────────────────────────┐  │
│  Unfinished   │  │ Name        Size   Status  ↓     ↑  ⋮  │  │
│  Completed    │  │ debian.iso  4.1GB  Seeding 0B/s 1MB/s  │  │
│  Queued       │  │ ubuntu.iso  2.3GB  Leeching 8MB/s 0B/s │  │
│               │  └────────────────────────────────────────┘  │
│               │  [Open Torrent Client]  [Add Magnet]         │
└───────────────┴──────────────────────────────────────────────┘
```

Tab content is a read-only overview plus two actions. It reuses the torrent
view module in compact mode. It does not reuse the downloads table.

### 4.2 Torrent window

Separate native window, label `torrents`, route `torrents.html`.

```
┌────────────────────────────────────────────────────────────────────┐
│ [Add Magnet] [Add File] [Pause] [Resume] [Recheck] [Remove]        │
├──────────────────┬─────────────────────────────────────────────────┤
│ Name             │  General │ Files │ Peers │ Trackers │ Speed    │
│ debian.iso       │─────────────────────────────────────────────────│
│ ubuntu.iso       │  Status        Seeding                          │
│                  │  Size          4.1 GB                           │
│                  │  Downloaded    4.1 GB (100%)                    │
│                  │  Uploaded      812 MB                           │
│                  │  Ratio         0.19                             │
│                  │  ↓ 0 B/s       ↑ 1.2 MB/s                       │
│                  │  Peers 34 (seeds 12)   ETA —                    │
│                  │                                                 │
│                  │  Files                                          │
│                  │  ☑ debian-13.0-amd64-netinst.iso   620 MB       │
│                  │  ☑ debian-13.0-amd64-DVD-1.iso     3.5 GB       │
└──────────────────┴─────────────────────────────────────────────────┘
```

Window is independent: can be minimized, kept open, or closed without affecting
the main window. Closing it does not stop torrents (they live in the engine).
A tray/menu item reopens it.

### 4.3 Add flow

```
Magnet paste / .torrent file
        │
        ▼
┌──────────────────────────────┐
│ Add Torrent                  │
│ Source: magnet:?xt=...       │
│ Save to: [~/Downloads/Torrents] │
│ ☐ Start paused               │
│ ☑ Fetch metadata first       │
│         [Cancel] [Add]       │
└──────────────────────────────┘
        │
        ▼
Metadata fetch → file list dialog → confirm selection → start
```

For magnets, metadata is fetched first (`AddTorrentOptions.list_only` or add
paused), then the file tree is shown. For `.torrent` files the file list is
available immediately.

---

## 5. Isolation contract

Hard rules for the implementation. A PR that violates one of these needs an
explicit design review.

### Must not be modified

| Path | Reason |
|---|---|
| `src-tauri/src/domain/download/**` | Download entity, statuses, validators, URL parsing. |
| `src-tauri/src/application/download_transfer_service/**` | Download orchestration, queue pump, completion actions. |
| `src-tauri/src/application/download_executor.rs` | Executor trait and control signals. |
| `src-tauri/src/infrastructure/media/composite_executor.rs` | Protocol dispatch. |
| `src-tauri/src/infrastructure/download_history_store.rs` | History schema. |
| `src-tauri/src/application/queue_service.rs` | Named queues and schedules. |
| `renderer/app.js` download logic | Table, filters, progress, actions. |
| `extension/background.js` download interception | Existing HTTP capture. |

### Allowed minimal edits

| Path | Edit |
|---|---|
| `src-tauri/Cargo.toml` | Optional `librtbit` dependency + `torrent` feature. |
| `src-tauri/src/lib.rs` | Register torrent module, state, commands, setup/exit hooks. |
| `src-tauri/src/presentation/app_state.rs` | Add one optional `torrents` field. |
| `src-tauri/src/presentation/window_manager.rs` | Add `open_torrent_window` + label constant. |
| `renderer/index.html` | Add view switch buttons, torrent container, script tags. |
| `src-tauri/tauri.conf.json` | Deep link scheme, `.torrent` file association. |
| `extension/manifest*.json` | Magnet handling permission/entry only. |

Everything else is new files.

---

## 6. Architecture

```
┌───────────────────────────── Main window ─────────────────────────────┐
│  Downloads view (existing, untouched)                                 │
│  Torrents view (new, compact)  ──invoke──► torrent_* commands         │
└───────────────────────────────────────────────────────────────────────┘
┌──────────────────────────── Torrent window ───────────────────────────┐
│  torrents.html + torrents-view.js (full mode)                         │
└───────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────── Backend ──────────────────────────────────┐
│ presentation/commands/torrents.rs                                     │
│        │                                                              │
│        ▼                                                              │
│ application/torrent_service.rs        (new, independent)              │
│        │                                                              │
│        ├── librtbit::Api / Session                                    │
│        ├── TorrentSettingsStore       (new, separate file)            │
│        └── progress poller → Tauri event `torrent://progress`         │
└───────────────────────────────────────────────────────────────────────┘

Existing: DownloadTransferService → CompositeDownloadExecutor → HTTP/FTP/...
          (no knowledge of torrents)
```

### Why a separate service, not `DownloadExecutor`

| `DownloadExecutor` assumption | Torrent reality |
|---|---|
| One URL | Magnet URI or bencoded file, plus swarm |
| One output file | Directory tree, N files, per-file priority |
| Finite transfer | Seeds indefinitely after completion |
| `ETag`/`Last-Modified` resume | Piece bitfield + fastresume |
| Segment progress | Piece/peer progress |
| Occupies one concurrency slot | Swarm-wide peer limits |
| `discard_partial` deletes one file | Must delete a tree safely |

Forcing torrents through the trait would require changing the trait, the
scheduler, and the history schema. That is exactly the coupling this plan avoids.

### Engine lifecycle

- Created once during Tauri `setup`, after the main window is shown.
- Stored as `Arc<TorrentService>` in `AppState` (optional field).
- Session persistence folder: `<app data>/torrents/session`.
- On `RunEvent::ExitRequested`: `session.stop().await` to flush state.
- If engine init fails: log, disable the Torrents tab with an inline error, keep
  the download manager fully functional.

---

## 7. Data and state ownership

| Data | Owner | Location |
|---|---|---|
| Piece state, bitfield, fastresume | librtbit | `<app data>/torrents/session/` |
| DHT routing table | librtbit | `<app data>/torrents/dht/` |
| Torrent list, files, peers, trackers | librtbit (queried live) | memory |
| Torrent UI settings | IDM | `<app data>/torrent-settings.json` |
| Torrent window geometry | Tauri | OS window state (not persisted in v1) |
| Download history | IDM (unchanged) | existing history JSON |

**No second index in v1.** The UI reads the engine's list directly. This avoids
two sources of truth and keeps torrents out of download history by design.

If later needed (categories, per-torrent completion actions), add a thin
`torrent-index.json` keyed by info hash — never merge into download history.

---

## 8. IPC surface

All commands are prefixed `torrent_` and live in
`src-tauri/src/presentation/commands/torrents.rs`.

### Commands

| Command | Purpose |
|---|---|
| `torrent_status` | Engine availability, version, listen port, DHT state. |
| `torrent_list` | List with progress, speeds, peers, ratio. |
| `torrent_add_magnet` | Add magnet, optionally paused / metadata-only. |
| `torrent_add_file` | Add `.torrent` from path or bytes. |
| `torrent_metadata` | Metadata fetch progress for a pending magnet. |
| `torrent_files` | File tree with size, progress, priority, included. |
| `torrent_set_files` | Apply included/priority selection. |
| `torrent_action` | `pause`, `resume`, `recheck`, `forget`, `delete`. |
| `torrent_details` | General info, trackers, peers, piece stats. |
| `torrent_settings_get` | Read torrent settings. |
| `torrent_settings_set` | Update torrent settings. |
| `torrent_open_window` | Open/focus the Torrents window. |

### Events

| Event | Payload |
|---|---|
| `torrent://progress` | Batched list snapshot, 1 Hz, only while a torrent window or tab is visible. |
| `torrent://engine` | Engine state changes: starting, ready, failed, stopped. |

Polling rule: one `api_torrent_list` call per tick, never one call per torrent.
Stop the poller when no torrent UI is visible to avoid idle wakeups.

---

## 9. Frontend plan

### New files

| File | Purpose |
|---|---|
| `renderer/torrents.html` | Torrent window shell. |
| `renderer/torrents.js` | Window bootstrap, mode = `full`. |
| `renderer/torrents-view.js` | Shared view: list, detail tabs, add dialog, settings. |
| `renderer/torrents.css` | Torrent-only styles, reuses existing CSS variables. |
| `renderer/torrents.test.cjs` | Formatters, state mapping, action guards. |

### Minimal edits

`renderer/index.html`:

1. Add two view buttons near the toolbar: `Downloads` / `Torrents`.
2. Add `<section id="torrents-view" hidden>` inside `.main-pane`.
3. Load `torrents-view.js` and `torrents.css`.

`renderer/app.js`: **no changes required** if the view switch is implemented in
`torrents-view.js`:

- `torrents-view.js` binds `#view-downloads` / `#view-torrents` clicks.
- Toggles `hidden` on `#table-scroll`, `.list-search`, and `#torrents-view`.
- Does not read or write any download state.
- Reuses `window.tauriApi` wrappers and `window.formatters`.

This keeps the download renderer untouched and testable in isolation.

### Reuse

- `renderer/tauri-api.js` — invoke/event wrappers.
- `renderer/formatters.js` — bytes, speed, ETA.
- `renderer/theme.js` — light/dark/system.
- Existing CSS variables from `styles.css`.

---

## 10. Settings

Separate store, separate file, separate UI panel inside the Torrents window.

`<app data>/torrent-settings.json`:

```json
{
  "version": 1,
  "enabled": true,
  "downloadDirectory": "~/Downloads/Torrents",
  "listenPort": 0,
  "enableDht": true,
  "enableUpnp": false,
  "enableUtp": false,
  "downloadLimitKbps": 0,
  "uploadLimitKbps": 0,
  "maxPeersPerTorrent": 50,
  "seedRatioLimit": 0,
  "seedTimeLimitMinutes": 0,
  "startPaused": false,
  "notifyOnComplete": true
}
```

Rules:

- Do **not** extend `domain/settings.rs` or the existing Options schema.
- Do **not** share `speed_limit_kbps` with the download manager.
  Torrent limits are independent by design.
- `enabled` is the runtime kill switch. If false: no session, no polling, tab
  shows a disabled state.
- `listenPort = 0` means random. Default UPnP and uTP off to avoid firewall and
  router prompts.

---

## 11. OS integration

| Platform | Work |
|---|---|
| macOS | `CFBundleURLTypes` for `magnet` via `tauri-plugin-deep-link`; `.torrent` document type; existing hardened-runtime entitlement `com.apple.security.network.server` already covers the listener. |
| Windows | NSIS registers `magnet:` handler and `.torrent` association; firewall prompt appears on first listen. |
| Linux | `.desktop` `MimeType=x-scheme-handler/magnet;application/x-bittorrent;` in the deb/AppImage bundle. |

Argument handling:

- The app already uses `tauri-plugin-single-instance` (`src-tauri/src/lib.rs`).
- Extend its callback: forward `magnet:` and `.torrent` argv entries to
  `torrent_add_magnet` / `torrent_add_file`, then focus the Torrents window.
- Do not route these into `DownloadTransferService`.

Startup order matters: the second-instance callback can fire before the engine
is ready. Queue pending targets in `TorrentService` and drain when ready.

---

## 12. Browser extension

Additive only. Existing HTTP capture stays as is.

1. Content script: intercept clicks on `a[href^="magnet:"]`, `preventDefault()`,
   send `{ type: "add_torrent", magnet }` to the background worker.
2. Background: forward over the existing native messaging channel.
3. Native host (`src-tauri/src/bin/native_host.rs`): accept `add_torrent` and
   forward to the running app over the loopback bridge.
4. Bridge: new `POST /torrents` endpoint in
   `src-tauri/src/infrastructure/bridge/server.rs` (additive route).
5. Context menu: "Send magnet to Open Download Manager".

No changes to `downloads.onCreated`, `webRequest` handlers, or capture policy.

---

## 13. Dependency and build plan

### Crate

```toml
[features]
default = []
torrent = ["dep:librtbit"]

[dependencies]
librtbit = { package = "swarmforge", version = "0.1", default-features = false, features = ["rust-tls"], optional = true }
```

Critical details:

- **`default-features = false`.** Default features enable `default-tls`, which
  pulls native-tls/OpenSSL. The app is pure rustls today. `rust-tls` keeps it
  pure Rust (`ring`).
- **No `http-api`, no `webui`, no `rss`.** Only `Session` and `Api` are needed.
  Avoids axum, rusqlite, and an HTTP server attack surface.
- **Pin exactly.** crates.io has only `0.1.0`. The repo is `0.2.0-beta.5`.
  If 0.2 features are required (`completed_folder`, share limits), pin an exact
  git revision, never a branch.
- **Feature-gate everything.** With `--no-default-features` (no `torrent`), the
  binary must build exactly as today, with no librtbit in the dependency graph.

### Verified compatibility

| Item | Status |
|---|---|
| Tauri version | Same major (2). |
| Tokio | Same major. |
| `reqwest` | Tauri already pulls `0.13.5`; librtbit uses 0.13. No third copy. |
| `base64` | Tauri already pulls `0.23.1`; matches current librtbit. |
| Rust edition 2024 | Needs 1.85+; CI uses stable. |
| macOS entitlements | `network.server` already present. |
| License | Apache-2.0 into MIT project is compatible. Keep `NOTICE`, add attribution to `THIRD-PARTY.md`. |

### Cost estimate

- ~25–30 new crates.
- +8–15 MB per installer (estimate, measure in Phase 0).
- +1–3 minutes per CI target.

---

## 14. Phases

Estimates assume one experienced Rust/Tauri developer. Do not start a phase
before its predecessor's acceptance criteria pass.

### Phase 0 — Feasibility spike (2 days)

**Tasks**

1. Branch `spike/torrent-engine`.
2. Add `librtbit` as a path dependency on `reference/rustTorrent/crates/librtbit`.
3. Build macOS arm64, Windows x64, **Windows arm64**, Linux x64.
4. Start a session in a scratch binary, add a legal test torrent.
5. Measure binary delta, clean build time, idle memory, listener behavior.

**Acceptance**

- All required targets compile, or Windows arm64 is explicitly dropped.
- Magnet metadata fetch, download, pause/resume, restart persistence verified.
- Size and build-time numbers recorded in this document.

**Kill criteria:** Windows arm64 cannot build and is required; or installer
growth exceeds budget; or engine cannot start under Tauri's runtime.

### Phase 1 — Skeleton behind feature flag (4 days)

**Tasks**

1. Add optional dependency and `torrent` Cargo feature.
2. New `domain/torrent/` DTOs: settings, list item, file, peer, tracker.
3. New `application/torrent_service.rs`: session init, shutdown, engine state.
4. New `infrastructure/torrent/`: session factory, persistence paths, poller.
5. New `presentation/commands/torrents.rs` with `torrent_status` and
   `torrent_list`.
6. `AppState.torrents: Option<Arc<TorrentService>>`.
7. `open_torrent_window` in `window_manager.rs`.
8. `renderer/torrents.html` + empty shell.

**Acceptance**

- Build without `torrent` feature is byte-for-byte behaviorally identical.
- Build with feature opens an empty Torrents window.
- Engine failure shows an error state, app still works.
- No changes to download tests.

### Phase 2 — Add, fetch, download (6 days)

**Tasks**

1. `torrent_add_magnet`, `torrent_add_file`, `torrent_metadata`.
2. Add dialog with save path, start-paused, metadata-first flow.
3. Progress poller + `torrent://progress` event, 1 Hz.
4. List rendering: name, size, progress, status, down/up speed, peers, ratio.
5. Actions: pause, resume, remove, delete-with-files confirmation.
6. Completion notification through the existing notification plugin (torrent
   subsystem calls it directly; no download action ledger involvement).

**Acceptance**

- Magnet and `.torrent` both reach 100% on a local test swarm.
- Pause/resume works across app restart.
- Deleting a torrent never touches download-manager files.
- No torrent rows appear in the downloads table or history.

### Phase 3 — Detail panes (6 days)

**Tasks**

1. Files tab: tree, per-file size/progress, include/exclude, priority.
2. Peers tab: address, client, progress, down/up rate, flags.
3. Trackers tab: URL, status, peers, next announce, error.
4. General tab: info hash, added date, save path, ratio, share limits.
5. Speed tab: session totals and per-torrent limits.

**Acceptance**

- File deselection reduces total size and skips pieces correctly.
- Recheck repairs a deliberately corrupted file.
- Peer/tracker views populate without blocking the UI.

### Phase 4 — Seed and leech controls (4 days)

**Tasks**

1. Upload/download limits wired to engine `LimitsConfig`.
2. Seed ratio and seed time limits; auto-stop behavior.
3. Port, DHT, uTP, UPnP settings with restart-required markers.
4. Seeding state in list and tray tooltip.
5. Leech-only mode decision (see open decisions).

**Acceptance**

- Limits apply live without restart where the engine supports it.
- Ratio/time limits stop seeding without marking the torrent failed.
- Settings persist and reload correctly.

### Phase 5 — OS and browser integration (5 days)

**Tasks**

1. `tauri-plugin-deep-link` for `magnet:`.
2. `.torrent` file association in bundle config.
3. Single-instance argv forwarding with pre-ready queue.
4. Menu and tray entries: "Torrents", "Add Magnet…".
5. Extension magnet click + context menu + native host message + bridge route.

**Acceptance**

- Clicking a magnet in Chrome/Firefox/Safari opens the app and adds the torrent.
- Double-clicking a `.torrent` file does the same.
- Second instance exits cleanly and focuses the existing window.
- Existing browser HTTP capture still works unchanged.

### Phase 6 — Hardening and release (8 days)

**Tasks**

1. Integration tests against a local mock swarm.
2. Crash recovery, disk full, missing payload, read-only folder.
3. Proxy interaction policy (see risks).
4. Logging and a diagnostics panel (engine version, listen port, DHT nodes).
5. Docs: user guide, `THIRD-PARTY.md`, `NOTICE`, legal disclaimer.
6. CI: build with and without the feature; one smoke test per OS.

**Acceptance**

- Full test suite green on macOS, Windows x64, Linux.
- Windows arm64 verified or explicitly excluded from release.
- No regressions in existing download tests.
- Attribution and license files shipped in installers.

---

## 15. Testing

| Layer | Coverage |
|---|---|
| Unit | Settings validation, state mapping, ratio math, path sanitization. |
| Engine integration | Local tracker + seeder from rustTorrent `benchv2`, magnet and `.torrent`, pause/resume, restart, file selection. |
| IPC | Command validation, engine-unavailable paths, malformed magnet rejection. |
| UI | `renderer/torrents.test.cjs` with Bun: formatters, list state, action guards. |
| Regression | Existing `cargo test` and extension tests must pass unchanged. |
| Manual | Firewall prompts, sleep/wake, VPN, proxy on/off, second instance. |

Security tests:

- Reject non-magnet, non-`.torrent` inputs.
- Cap `.torrent` file size.
- Sanitize torrent names before path creation.
- Verify no path traversal in file names.
- Verify engine listens only on the configured port and never exposes HTTP.

---

## 16. Risks and blockers

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| R1 | Upstream is beta; only `0.1.0` published; repo is `0.2.0-beta.5`. | High | Pin exact version/rev. Vendor the 12 crates if needed. Budget upgrade work. |
| R2 | Windows arm64 unverified upstream (they ship x64 only). | High | Phase 0 gate. Drop arm64 torrent support if it fails. |
| R3 | Engine crash could take down the app. | Medium | Wrap init and commands; never `unwrap`; engine failure disables the tab only. |
| R4 | Two persistence systems could drift. | Medium | Engine is sole owner of torrent state. No IDM index in v1. |
| R5 | Seeding breaks concurrency assumptions. | Medium | Separate service, separate limits, no shared queue. |
| R6 | Proxy does not cover peer traffic. | High | Detect proxy and block torrent start with an explicit warning, or require the user to acknowledge direct connection. Do not silently bypass. |
| R7 | Firewall/UPnP prompts alarm users. | Medium | UPnP and uTP off by default; random port; explain in first-run dialog. |
| R8 | AV false positives / SmartScreen reputation. | Medium | Code signing, stable publisher identity, no bundled trackers, docs. |
| R9 | Legal exposure from user behavior. | Medium | Neutral-use disclaimer, no bundled search/index, no default trackers. |
| R10 | Binary/build growth. | Low | Feature flag, size budget in Phase 0, optional `disable-upload` build. |
| R11 | `disable-upload` is compile-time, not runtime. | Low | Decide leech-only policy before Phase 4. |
| R12 | Extension store review for magnet handling. | Low | Keep magnet logic additive and clearly scoped. |

Non-blockers confirmed:

- Apache-2.0 into MIT is compatible with attribution.
- Tauri, Tokio, `reqwest`, and `base64` versions align.
- macOS network entitlements already present.
- Rust edition 2024 works on current stable.

---

## 17. Rollout

1. **Feature flag off by default** for the first merged PRs. No user-visible change.
2. Internal builds with `--features torrent` for dogfooding.
3. Beta channel: flag on, Torrents tab visible, "Beta" badge.
4. Stable: flag on after Phase 6 acceptance.
5. Kill switch: `torrent-settings.json` → `"enabled": false` disables session
   and UI without a rebuild.

Rollback: remove the feature flag from the build. No download-manager code
depends on the torrent module.

---

## 18. Open decisions

| # | Decision | Options | Recommended |
|---|---|---|---|
| D1 | Engine strategy | Embed / sidecar / external client | Embed |
| D2 | Seeding policy | Seed forever / ratio+time limits / leech-only | Ratio + time limits, defaults off |
| D3 | Proxy behavior | Block torrents / warn and go direct / route SOCKS where possible | Warn and require explicit confirmation |
| D4 | Windows arm64 | Support / drop | Gate on Phase 0 |
| D5 | Upstream pin | crates.io `0.1.0` / git rev of `0.2.0-beta.5` | crates.io unless 0.2 features are required |
| D6 | Torrent save path | Fixed subfolder / per-torrent choice / category mapping | Per-torrent choice, default `Downloads/Torrents` |
| D7 | Home tab depth | Summary only / full list | Full list, detail opens window |
| D8 | Completion actions | Reuse download actions / torrent-only notification | Notification only in v1 |

---

## 19. Appendix — engine API mapping

| Need | librtbit API |
|---|---|
| Create engine | `Session::new_with_opts(PathBuf, SessionOptions)` |
| Serializable facade | `Api::new(session, None, None)` |
| Add magnet | `api_add_torrent(AddTorrent::from_url(...), opts)` |
| Add `.torrent` | `api_add_torrent(AddTorrent::from_bytes(...), opts)` |
| List | `api_torrent_list()` |
| Stats | `api_stats_v1(id)` → `TorrentStats` |
| Details | `api_torrent_details(id)` |
| Pause / resume | `api_torrent_action_pause` / `api_torrent_action_start` |
| Recheck | `api_torrent_action_recheck` |
| Forget / delete | `api_torrent_action_forget` / `api_torrent_action_delete` |
| File selection | `api_torrent_file_set_included(id, file_id, bool)` |
| Rate limits | `api_set_torrent_rate_limits` / `SessionOptions.ratelimits` |
| Shutdown | `Session::stop().await` |

Relevant option types:

- `SessionOptions`: `disable_dht`, `persistence`, `listen`, `ratelimits`,
  `fastresume`, `peer_limit`, `disable_local_service_discovery`.
- `ListenerOptions`: `mode` (`TcpOnly` default), `listen_addr`,
  `enable_upnp_port_forwarding` (default false).
- `AddTorrentOptions`: `paused`, `only_files`, `output_folder`, `overwrite`,
  `peer_limit`, `ratelimits`, `magnet_resolution_timeout_secs`.

Reference implementation in the cloned repo:
`reference/rustTorrent/desktop/src-tauri/src/main.rs` and `launch.rs`.
