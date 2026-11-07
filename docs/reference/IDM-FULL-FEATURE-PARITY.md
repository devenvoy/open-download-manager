# IDM Full Feature-Parity Report

## Executive summary

This report compares IDM 6.43 Build 18 capabilities with the current Open
Download Manager Rust/Tauri application. It is a fresh, source-based inventory;
the earlier comparison documents were not used as evidence.

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

- **IDM target:** IDM 6.43 Build 18.
- **Evidence:** User-facing labels, configuration keys, function names, and
  selected transfer, browser-menu, and Grabber code paths were inspected.
- **Rust target:** Direct inspection of the current Rust/Tauri source,
  renderer, and browser extension. Useful entry points include
    [download_transfer_service.rs](../../src-tauri/src/application/download_transfer_service.rs),
    [settings.rs](../../src-tauri/src/domain/settings.rs),
    [index.html](../../renderer/index.html), and
    [background.js](../../extension/background.js).
- IDM strings such as menu labels and settings keys establish that a feature
  exists, but by themselves do not establish every runtime
  condition or default. Relevant code paths were inspected to corroborate
  transfer behavior. Runtime UI behavior, traffic capture, and performance
  were not validated.
- This is a broad evidence-based feature inventory, not a guarantee that every
  IDM dialog or undocumented behavior has been enumerated. Features below are
  limited to capabilities corroborated by the inspected evidence.

Evidence shorthand: **Observed behavior** means a user-facing label,
configuration key, diagnostic, or relevant code path supports the feature;
**Rust UI/source** means the capability or gap was checked in the current app
source and UI.

## Feature matrix

### Transfer protocols, connections, and file handling

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 1 | Download files over HTTP | Yes | HTTP downloads are implemented. |
| 2 | Download files over HTTPS | Yes | HTTPS downloads use the HTTP executor. |
| 3 | FTP download protocol | No | The transfer executor accepts HTTP(S), not FTP. |
| 4 | FTP passive-mode option for firewalls | No | No FTP executor or passive-mode setting exists. |
| 5 | MMS streaming/download protocol | No | MMS is not implemented. |
| 6 | Split an HTTP file into multiple byte-range connections | Yes | Configurable per-file connections and segmented HTTP are present. |
| 7 | Reassign a file chunk when a connection is lost | Yes | IDM's chunk reassignment path and Rust's segmented-worker rebalancing provide the equivalent behavior. |
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

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
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

IDM has media/streaming-related capabilities including a video panel and MMS.
The inspected media routing recognizes YouTube/Google video delivery URLs and
HLS/MPD paths. Rust's HLS/YouTube functionality is noted below as current
behavior, not assumed equivalent just because both products handle media.

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 31 | Offer browser-side video/media download controls | No | Rust offers an in-page chooser for supported media, but not IDM-equivalent broad site coverage. |
| 32 | Recognize the broad video MIME/content-type set used by the video panel | No | Rust has selected media capture paths; its extension does not implement an equivalent configurable and broadly integrated video-panel capture surface. |
| 33 | Special handling for YouTube and Google video delivery URLs | No | Rust has YouTube-specific capture, but its tested/implemented route set is narrower than IDM's observed YouTube/Google-video URL and player-response handling. |
| 34 | Handle adaptive HLS and DASH/MPD media paths | No | Rust handles a subset of HLS VOD; encrypted, live, byte-range, and some nested HLS cases are rejected, and DASH/MPD is unsupported. |
| 35 | Present a supported video-download panel across IDM-integrated player sites | No | Rust has an in-page chooser for supported capture paths, not the same breadth of integrated player-site coverage. |
| 36 | Download MMS media streams | No | MMS is not supported. |

### Webpage downloading and Site Grabber

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
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

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
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

| # | IDM setting/option | Rust app equivalent? | Rust app behavior / gap |
|---:|---|:---:|---|
| 67 | Set a default download directory | Yes | The Options dialog exposes a default folder. |
| 68 | Set a separate temporary directory for file parts | No | No temporary-download-directory setting was found. |
| 69 | Configure per-category save directories | No | Category folders exist in the backend model but are not exposed in current settings UI. |
| 70 | Limit simultaneous downloads | Yes | The app has a configurable maximum concurrent-download count. |
| 71 | Set maximum connections per file | Yes | The app exposes a per-file connection limit. |
| 72 | Configure per-site connection-count exceptions | No | IDM has per-site connection exceptions; Rust only has a global per-file connection limit. |
| 73 | Set a global speed cap | Yes | Rust has a global KB/s speed limit. |
| 74 | Limit total downloads by a configured time window and data amount | No | No rolling time-window/data-volume download cap is implemented. |
| 75 | Configure HTTP proxy host, port, and credentials | Yes | HTTP proxy configuration and optional basic authentication are present. |
| 76 | Configure HTTPS proxy host, port, and credentials | Yes | HTTPS proxy configuration is present. |
| 77 | Configure SOCKS5 proxy and DNS behavior | No | Rust proxy settings support HTTP/HTTPS only. |
| 78 | Configure a separate FTP proxy | No | FTP and FTP proxy settings are absent. |
| 79 | Detect/import desktop/browser proxy settings | No | No system-proxy discovery option was found. |
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

| # | IDM capability | Rust app equivalent? | Rust app behavior / gap |
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
4. Add per-site connection exceptions and rolling download limits; keep
   retry/backoff policies and capture policies as separately scoped work.
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
   Static source inspection cannot establish performance parity.

## Source-backed issues to track

- Resume metadata currently has one `etag` string slot; the validator kind is
  not persisted alongside it.
- Destination collisions currently fail instead of offering IDM-style choices.
- Proxy reconfiguration errors are discarded by the settings command.
- The extension logs a captured SABR URL, which may contain a signed token.
- The Grabber toolbar button is present but explicitly reports that website
  Grabber projects are unavailable.

These are source observations, not claims that every feature is enabled by
default in every IDM configuration.

## Issue map for remaining gaps

The issue numbers below point to scoped implementation work. The P0 items
address correctness first; the larger parity features follow in dependency
order.

| Report rows | Issue | Scope |
|---|---|---|
| 9–10 | [#25](https://github.com/devenvoy/open-download-manager/issues/25) | Preserve HTTP validator type across resume |
| 14, 93 | [#26](https://github.com/devenvoy/open-download-manager/issues/26) | Offer destination conflict actions |
| Proxy settings | [#27](https://github.com/devenvoy/open-download-manager/issues/27) | Surface proxy reconfiguration failures |
| 3–4, 78, 81 | [#28](https://github.com/devenvoy/open-download-manager/issues/28) | FTP transfers, credentials, passive mode, and proxy |
| 24–26, 28, 30, 82–84 | [#29](https://github.com/devenvoy/open-download-manager/issues/29) | Browser capture rules, file types, exclusions, confirmation |
| 12, 80 | [#30](https://github.com/devenvoy/open-download-manager/issues/30) | Secure site/path credentials |
| 87, 89–92, 96 | [#31](https://github.com/devenvoy/open-download-manager/issues/31) | Completion actions, sounds, scanning, and commands |
| 68–69 | [#32](https://github.com/devenvoy/open-download-manager/issues/32) | Category folders and temporary-parts directory |
| 27, 85 | [#33](https://github.com/devenvoy/open-download-manager/issues/33) | Opt-in clipboard link monitoring |
| 77 | [#34](https://github.com/devenvoy/open-download-manager/issues/34) | SOCKS5 proxy and DNS behavior |
| 5, 36 | [#35](https://github.com/devenvoy/open-download-manager/issues/35) | MMS support or explicit unsupported guidance |
| 72, 74 | [#36](https://github.com/devenvoy/open-download-manager/issues/36) | Per-site connection overrides and rolling data limits |
| 31–35 | [#37](https://github.com/devenvoy/open-download-manager/issues/37) | Browser media panel and adaptive-stream coverage |
| 23, 86 | [#38](https://github.com/devenvoy/open-download-manager/issues/38) | Opt-in advanced browser integration |
| 79 | [#39](https://github.com/devenvoy/open-download-manager/issues/39) | Import system/browser proxy configuration |
| 39–51 | [#14](https://github.com/devenvoy/open-download-manager/issues/14) | Bounded Site Grabber and current-page link selection |
| 53–61, 66 | [#13](https://github.com/devenvoy/open-download-manager/issues/13) | Named queues, queue controls, and recurring schedules |
