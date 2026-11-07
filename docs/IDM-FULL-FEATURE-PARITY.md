# IDM Full Feature-Parity Report

## Executive summary

This report compares the supplied standalone IDM executable with the current
Open Download Manager Rust/Tauri application. It is a fresh, source-based
inventory; the earlier comparison documents were not used as evidence.

The Rust app already has a solid HTTP/HTTPS download manager foundation:
segmented transfers, pause/resume, browser handoff for supported browsers,
basic queues and one-time scheduling, categories, and selected media capture.
It is **not yet feature-equivalent to IDM**. The largest product gaps are FTP
and MMS, IDM's configurable browser/file-type rules and clipboard monitoring,
site/path credential management, named and recurring queues, and the complete
Site Grabber project workflow.

`Yes` means an equivalent user-facing capability is present in the Rust app.
`No` means it is missing or materially incomplete. A `Yes` only confirms that
the capability is represented in source/UI; it does not claim that runtime
behavior was tested or is bug-free. Partial coverage is deliberately marked
`No`.

## Evidence and limits

- **IDM target:** `IDMan.exe`, identified in the binary as IDM 6.43 Build 18.
  SHA-256: `9dbad793d951fb073c60c3f2e5d2cea5e7e4c8bc1f4204aa4bfab4d03d3c8d67`.
- **Installer:** `idman643build18.exe`, SHA-256:
  `7346128fab42957b4254eabdb012bfdbd751e9069361e98a6916f26bb03d62ee`.
- **Analysis:** Fresh Ghidra 12.2 headless analysis of both PE32 x86 files,
  including strings, references, function names, and targeted decompilation of
  transfer, browser-menu, and Grabber-related routines. The installer analysis
  covered its setup stub; its payload was not extracted.
- **Rust target:** Direct inspection of the current Rust/Tauri source,
  renderer, and browser extension. Useful entry points include
  [download_transfer_service.rs](../src-tauri/src/application/download_transfer_service.rs),
  [settings.rs](../src-tauri/src/domain/settings.rs),
  [index.html](../renderer/index.html), and
  [background.js](../extension/background.js).
- IDM strings such as menu labels and settings keys establish that a feature
  exists in the binary, but by themselves do not establish every runtime
  condition or default. Decompilation was used where available to corroborate
  transfer behavior. Neither executable was run; there was no dynamic UI
  inspection, traffic capture, installer-payload extraction, or performance
  comparison. REA was not available.
- This is a broad evidence-based feature inventory, not a guarantee that every
  IDM dialog or undocumented behavior has been enumerated. Features below are
  limited to those found in the supplied executable or its visible strings.

Evidence shorthand: **IDM strings** means UI text/configuration keys found by
Ghidra; **IDM decompilation** means a relevant decompiled routine corroborated
the behavior; **Rust UI/source** means the capability or gap was checked in the
current app source and UI.

## Feature matrix

### Transfer protocols, connections, and file handling

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 1 | Download files over HTTP | Yes | HTTP downloads are implemented. |
| 2 | Download files over HTTPS | Yes | HTTPS downloads use the HTTP executor. |
| 3 | FTP download protocol | No | The transfer executor accepts HTTP(S), not FTP. |
| 4 | FTP passive-mode option for firewalls | No | No FTP executor or passive-mode setting exists. |
| 5 | MMS streaming/download protocol | No | MMS is not implemented. |
| 6 | Split an HTTP file into multiple byte-range connections | Yes | Configurable per-file connections and segmented HTTP are present. |
| 7 | Reassign a file chunk when a connection is lost | Yes | Ghidra decompilation shows IDM's chunk reassignment path; Rust has segmented workers and rebalancing. |
| 8 | Pause, resume, cancel, and retry a download | Yes | These actions exist in the download controls. |
| 9 | Resume an interrupted HTTP download when the server supports ranges | Yes | Resume/range handling exists, though validator persistence needs correction (see plan). |
| 10 | Preserve correct ETag versus Last-Modified semantics across restart | No | Rust persists a validator as a single `etag` string, losing its validator kind. |
| 11 | Automatically reconnect/retry without a user action | No | Rust exposes retry/resume; an equivalent configurable automatic retry policy was not found. |
| 12 | Enter saved site/path username and password for protected downloads | No | No IDM-style site/path credential manager exists. |
| 13 | Configure FTP credentials and FTP proxy separately | No | FTP and its account/proxy settings are absent. |
| 14 | Choose what to do when a destination file already exists (overwrite, rename, or cancel) | No | Rust currently returns a destination-exists error rather than offering IDM-style choices. |
| 15 | Select a destination folder and filename for a new download | Yes | The Add Download flow accepts a URL and destination details. |
| 16 | Add multiple direct URLs as a batch | Yes | The app accepts multiple URLs and creates separate records. |
| 17 | Show transfer progress and per-connection/range information | Yes | The details UI shows progress and segmented connection information. |
| 18 | Queue work when the configured concurrent-download limit is reached | Yes | A queue pump enforces the app-wide concurrent-download limit. |

### Browser and operating-system integration

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 19 | Integrate with browsers to hand downloads to IDM | Yes | Native-messaging browser integration is available for Chrome, Edge, Brave, Opera, and Firefox. |
| 20 | Install or refresh browser integration from the desktop app | Yes | The app has browser discovery/setup flows for its supported browsers. |
| 21 | Intercept ordinary browser HTTP(S) downloads | Yes | The extension hands supported HTTP(S) browser downloads to the desktop app. |
| 22 | Browser context-menu action to download a selected link | Yes | The extension exposes context actions for links/media. |
| 23 | Advanced browser-integration controls and broader browser coverage | No | The Rust extension supports five browsers, but lacks IDM's broader/advanced integration surface. |
| 24 | Configure automatic capture by file extension/type | No | No user-editable IDM-style file-type capture list was found. |
| 25 | Exclude specific websites from automatic capture | No | No per-site exclusion list is exposed. |
| 26 | Exclude specific addresses/URLs from automatic capture | No | No address exclusion list is exposed. |
| 27 | Monitor the system clipboard for download links | No | The Add dialog can paste from the clipboard on demand; background clipboard monitoring is absent. |
| 28 | Configure whether a browser download is confirmed before handoff | No | A backend setting field exists, but no equivalent user-facing IDM-style rule/control was found in the current settings UI. |
| 29 | Start the app with Windows and keep it in the system tray | Yes | The General settings UI exposes Windows startup/tray behavior. |
| 30 | Automatically capture downloads from every browser and download type | No | Coverage is limited to the supported extension/native-messaging path and HTTP(S). |

### Media and streaming

The IDM binary contains media/streaming-related code and strings, including
MMS, but the static evidence did not establish exact HLS, YouTube, DASH,
playlist, or format-picker parity. Rust's HLS/YouTube functionality is noted
below as current behavior, not assumed to be equivalent just because both
products handle media.

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 31 | Offer browser-side video/media download controls | No | Rust offers an in-page chooser for supported media, but not IDM-equivalent broad site coverage. |
| 32 | HLS playlist and variant-capture parity | No | Rust discovers HLS variants; equivalent IDM behavior was not established by the static analysis. |
| 33 | YouTube video/audio capture parity | No | Rust supports selected YouTube paths; exact IDM parity was not established. |
| 34 | Media quality/format selection parity | No | Rust offers format selection for supported paths; the equivalent IDM workflow was not established. |
| 35 | Playlist selection parity | No | Rust supports YouTube playlist selection; equivalent IDM behavior was not established. |
| 36 | Download arbitrary embedded media across supported websites | No | Rust coverage is limited; DASH is explicitly unsupported and other site-specific schemes are not generally handled. |
| 37 | Download MMS media streams | No | MMS is not supported. |
| 38 | Install/use media tools to assemble separate audio/video streams | No | Rust uses FFmpeg/yt-dlp for selected media paths, but this is not equivalent to IDM's broad browser media-capture coverage. |

### Webpage downloading and Site Grabber

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 39 | “Download All Links with IDM” from a webpage | No | The extension can collect a bounded set of direct links, but does not provide IDM's full all-links selection/filter workflow. |
| 40 | Create a website Grabber project from a starting page | No | The visible Grabber button explicitly reports that website Grabber projects are unavailable. |
| 41 | Crawl pages recursively from a starting URL | No | No recursive site crawler exists. |
| 42 | Choose crawl depth / scope | No | No depth or scope controls exist. |
| 43 | Filter Grabber targets by link/file type | No | No Grabber filter editor exists. |
| 44 | Define include/exclude rules for site paths and addresses | No | No project rule editor exists. |
| 45 | Save an offline website including images, styles, and scripts | No | No site mirror/offline-page pipeline exists. |
| 46 | Keep downloaded pages' links/assets usable for offline browsing | No | No URL rewriting or offline link preservation exists. |
| 47 | Review/select discovered website files before downloading | No | No Grabber project result-selection interface exists. |
| 48 | Save and reopen named Grabber projects | No | No Grabber project model/storage exists. |
| 49 | Update an existing Grabber project/site mirror | No | No project update flow exists. |
| 50 | Schedule a Grabber project for later or recurring execution | No | No scheduled Grabber jobs exist. |
| 51 | Collect direct links from the current browser page | No | A bounded page-link collection message exists, but not a user-facing equivalent to IDM's page Grabber/all-links workflow. |

### Queues, scheduling, categories, and history

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 52 | Add downloads to a main queue and start/stop it | Yes | The app exposes Start Queue and Stop Queue actions. |
| 53 | Create and manage multiple named download queues | No | Rust has a main queue and views/filters, not IDM-style named queues. |
| 54 | Start or stop a selected named queue | No | Queue control is global. |
| 55 | Set different concurrency limits per queue | No | The concurrent-download cap is app-wide. |
| 56 | Apply separate speed limits to each queue | No | Rust has one global speed limit. |
| 57 | Schedule a single download for a future local time | Yes | One-time scheduled downloads are supported while the app is running. |
| 58 | Start/stop a queue on a recurring time/day schedule | No | No recurring queue schedule or time window exists. |
| 59 | Schedule periodic/recurring downloads | No | Only one-time scheduled starts are implemented. |
| 60 | Start all scheduled work immediately through queue controls | Yes | Start Queue starts queued, paused, and scheduled items immediately. |
| 61 | Keep scheduled downloads running after the app exits | No | The UI states that scheduled downloads start only while the app is running. |
| 62 | Automatically assign downloads to file categories | Yes | Rust derives categories from filenames/extensions. |
| 63 | Configure category-specific download folders from settings | No | The backend has category-directory storage, but the current settings UI does not expose an equivalent editor. |
| 64 | Maintain searchable download history/statuses | Yes | Persistent records include transfer state and metadata. |
| 65 | Import/export download records | Yes | Import and export commands are available. |
| 66 | Prioritize/reorder downloads within named queues | No | No IDM-style queue ordering/prioritization controls were found. |

### IDM settings and preferences

| # | IDM setting/option evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 67 | Set a default download directory | Yes | The Options dialog exposes a default folder. |
| 68 | Set a separate temporary directory for file parts | No | No temporary-download-directory setting was found. |
| 69 | Configure per-category save directories | No | Category folders exist in the backend model but are not exposed in current settings UI. |
| 70 | Limit simultaneous downloads | Yes | The app has a configurable maximum concurrent-download count. |
| 71 | Set maximum connections per file | Yes | The app exposes a per-file connection limit. |
| 72 | Select an IDM connection-speed/profile type | No | No connection profile selector was found. |
| 73 | Set a global speed cap | Yes | Rust has a global KB/s speed limit. |
| 74 | Set speed limits by queue or schedule | No | No per-queue or time-dependent speed policy exists. |
| 75 | Configure HTTP proxy host, port, and credentials | Yes | HTTP proxy configuration and optional basic authentication are present. |
| 76 | Configure HTTPS proxy host, port, and credentials | Yes | HTTPS proxy configuration is present. |
| 77 | Configure SOCKS5 proxy and DNS behavior | No | Rust proxy settings support HTTP/HTTPS only. |
| 78 | Configure a separate FTP proxy | No | FTP and FTP proxy settings are absent. |
| 79 | Detect/import proxy settings from Windows/Internet Explorer | No | No system-proxy discovery option was found. |
| 80 | Store site/path passwords for repeated downloads | No | No site/path credential list or editor exists. |
| 81 | Configure FTP username/password defaults | No | FTP credentials are not supported. |
| 82 | Configure file extensions IDM should capture | No | No editable capture-extension list exists. |
| 83 | Configure sites where IDM should not start automatically | No | No site exclusion settings exist. |
| 84 | Configure addresses where IDM should not start automatically | No | No address exclusion settings exist. |
| 85 | Enable/disable system clipboard monitoring | No | The app supports explicit paste, not clipboard monitoring. |
| 86 | Configure browser integration and advanced integration options | No | Browser setup exists, but the equivalent advanced settings surface does not. |
| 87 | Configure notification on completion | Yes | A completion notification setting is available in the download details flow. |
| 88 | Open/reveal the destination folder on completion | Yes | A completion reveal option is available. |
| 89 | Configure whether to show the completion dialog | No | Completion behavior exists in the model, but no corresponding user-facing setting was found in Options. |
| 90 | Configure sounds for IDM events | No | No sound settings or event sounds were found. |
| 91 | Run antivirus software after a download completes | No | No post-download antivirus command/setting exists. |
| 92 | Configure post-download shutdown, sleep/hibernate, or hang-up actions | No | No equivalent completion actions exist. |
| 93 | Choose overwrite/rename/cancel behavior for duplicate destination files | No | Rust reports a file-exists error rather than presenting these choices. |

### Completion behavior, updates, and application management

| # | IDM feature evidenced in binary | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 94 | Show download completion notifications | Yes | Windows notifications are supported. |
| 95 | Open the completed file or its containing folder | Yes | Open-file and open-folder actions are available; folder reveal can be enabled on completion. |
| 96 | Run arbitrary post-download commands | No | No general post-download command hook exists. |
| 97 | Check for application updates | Yes | Rust checks GitHub Releases in the background and supports manual checks. |
| 98 | Install/update IDM in place through its installer workflow | No | Rust's update flow is not equivalent to IDM's Windows installer/update behavior. |
| 99 | Provide the full IDM options/preferences surface | No | Rust exposes a smaller set of General, Save to, Connection, Proxy, and Media Tools settings. |

## Corrections and implementation plan

Priorities are ordered to improve correctness first, then close the largest
feature gaps. The plan is intentionally staged: a literal 100% clone cannot be
claimed from static inspection alone, and some IDM features depend on Windows
APIs, browser permissions, and protocol/site-specific behavior.

### P0 — Correctness and safe transfer behavior

1. **Persist resume validators with their type.** Store whether a validator is
   an ETag or Last-Modified value, and emit the matching conditional range
   header after restart. Add server-fixture tests covering both validators,
   changed validators, unsupported ranges, and restart/resume.
2. **Implement destination conflict choices.** Replace the current immediate
   “file already exists” failure with explicit Overwrite, Rename, Skip, and
   Cancel behavior. Make overwrite atomic/safe and test each branch.
3. **Surface proxy-application failures.** `update_settings` currently ignores
   errors while applying proxy settings. Return a visible error and preserve
   the last known-good runtime proxy rather than reporting a successful save
   when the active transfer client was not updated.
4. **Redact browser capture diagnostics.** Remove full captured SABR URLs from
   extension logs; signed query strings/session tokens must not be written to
   ordinary logs.
5. **Add end-to-end transfer tests.** Cover interrupted downloads, retries,
   single/segmented fallback, file collisions, and proxy reconfiguration before
   declaring those features equivalent.

### P1 — IDM's largest parity gaps

1. **Build Site Grabber as a first-class project workflow.** Add project
   creation from a start URL, scope/depth, include/exclude rules, file-type
   filters, discovered-link review, resource downloading (including
   images/styles/scripts), offline link rewriting, project persistence/update,
   and scheduled project runs. Replace the current placeholder button only
   when the full workflow is usable.
2. **Add capture rules and browser controls.** Provide configurable extension
   include/exclude lists, site/address exclusions, explicit confirmation
   policy, and supported-browser status/setup. Add clipboard monitoring as an
   opt-in setting with clear permission and privacy behavior.
3. **Add FTP.** Implement FTP transfer, passive mode, authentication, resume
   where supported, and FTP proxy configuration. Test against a local FTP
   server before adding the UI.
4. **Add site/path credentials and SOCKS5.** Provide a secure credential store
   with matching rules and explicit per-download override; add SOCKS5 proxy
   and DNS behavior only with dedicated connection tests.
5. **Expand media coverage deliberately.** Build a documented source/format
   support matrix and test HLS, DASH, and relevant browser capture flows.
   Avoid promising IDM-wide coverage based only on YouTube/HLS support.

### P2 — Queues, scheduling, and settings depth

1. Add named queues with per-queue concurrency, speed limits, order/priority,
   and independent start/stop controls.
2. Add recurring schedules and queue start/stop windows (daily/weekly), plus
   behavior for app exit, sleep/resume, missed schedules, and time-zone/DST
   changes.
3. Expose category-directory editing and a separate temporary-parts directory.
4. Add configurable connection profiles, retry/backoff policies, and
   per-site/address capture policies.
5. Add completion actions: custom sounds, antivirus command integration,
   shutdown/sleep/hibernate, and optional post-download commands, each with
   explicit confirmation and failure reporting.

### P3 — Broader polish and parity verification

1. Complete application-management parity: update/install workflow, migration
   of prior settings/history, and platform-specific startup/tray behavior.
2. Build a capability-level acceptance suite from this matrix. For every `Yes`,
   verify actual behavior on supported operating systems and browsers; for
   every `No`, mark it closed only after the user-facing workflow and tests
   exist.
3. Run real-browser and protocol integration tests, then compare download
   correctness and throughput under controlled network/server conditions.
   Static Ghidra analysis cannot establish performance parity.

## Source-backed issues to track

- Resume metadata currently has one `etag` string slot; the validator kind is
  not persisted alongside it.
- Destination collisions currently fail instead of offering IDM-style choices.
- Proxy reconfiguration errors are discarded by the settings command.
- The extension logs a captured SABR URL, which may contain a signed token.
- The Grabber toolbar button is present but explicitly reports that website
  Grabber projects are unavailable.

These are source observations, not claims that the supplied IDM binary was
runtime-tested or that every feature surfaced by a binary string is enabled in
every IDM configuration.
