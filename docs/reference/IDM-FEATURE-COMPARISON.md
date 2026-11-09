# IDM vs. Open Download Manager: Feature Comparison

## Executive summary

This compares the current Rust/Tauri application with IDM 6.43 Build 18
capabilities. It is a source-level comparison, not a claim of runtime or speed
equivalence.

The application already implements HTTP(S) downloads, validated segmented
transfers with chunk reassignment and single-stream recovery, pause/resume,
queues, scheduling, rate limiting, browser capture, and several media paths.
It is not a feature-for-feature replacement for IDM: FTP is not accepted, the
general Site Grabber workflow is not present, and some UI, protocol, and
interoperability behavior remains unverified or partial.

The most important remaining work is:

1. **Measure rather than assume parity:** no same-file or same-network
   throughput, startup-latency, or reliability benchmark has been run.
2. **Improve resume-validator persistence:** the saved download record carries
   a validator string but not whether it came from ETag or Last-Modified, so a
   process-restart resume cannot always reuse the original validator type.
3. **Complete metrics instrumentation:** completion and fallback are logged,
   but response/first-byte/retry measurements are not wired and reported
   concurrency is not a live count for every transfer engine.
4. **Decide on product-scope gaps:** FTP, Digest proxy authentication, and a
   full Site Grabber are absent; whether to implement them depends on the
   intended product, not on download-engine correctness.

The best direction is to close the measurable reliability/observability gaps,
decide which absent protocols are in scope, then benchmark them. A wholesale
rewrite is not justified by this comparison.

## Scope and evidence

- **IDM evidence:** user-facing labels, configuration markers, diagnostic
  messages, and selected functional paths support the observations below.
  Static evidence does not establish every default or runtime behavior.
- **Rust evidence:** repository source at revision `cfed517`
  (`feat(transfer): implement chunk rebalancing, recovery, and
  eager probing`), checked against the current worktree. The revision is
  reported for traceability, not as a claim of behavioral equivalence.
- **Evidence labels:** “Observed” means labels, settings, diagnostics, or a
  relevant functional path support the behavior. “Not found” means not found
  in the reviewed repository scope, not a universal proof about every build.
- **Performance:** no throughput or time-to-first-byte comparison was run.
  Static inspection cannot establish which product is faster on a given
  server, site, or network.

For additional behavior notes and evidence limits, see
[`DOWNLOAD-BEHAVIOR-NOTES.md`](./DOWNLOAD-BEHAVIOR-NOTES.md).

## Feature-by-feature comparison

| Feature | Observed IDM behavior | Rust application status | Comparison and action |
|---|---|---|---|
| **HTTP/HTTPS downloads** | HTTP request and response handling is represented. | Implemented through the reqwest-based HTTP executor. | **Present in both.** The app's supported address protocols are explicitly HTTP/HTTPS; see [`address.rs`](../../src-tauri/src/domain/download/address.rs). |
| **Parallel byte-range downloads** | `Range`, `If-Range`, `Content-Range`, and range-reply handling are represented. | Implemented when a range probe finds a known total and strong ETag or Last-Modified validator; otherwise transfer uses one stream. | **Present, conditional.** A requested connection count is not a guarantee: server support and a usable validator are required. See [`segmented.rs`](../../src-tauri/src/infrastructure/http_download_executor/segmented.rs) and [`http_download_executor/mod.rs`](../../src-tauri/src/infrastructure/http_download_executor/mod.rs). |
| **Chunk rebalancing/reassignment** | Diagnostic messages describe reassigning a file chunk from one connection to another. | Implemented: an idle worker can split and take the unreceived upper portion of a sufficiently large active range. | **Present in both at the feature level.** Validate fairness, byte ownership, pause/cancel races, and throughput under slow or uneven servers. Do not infer algorithmic equivalence from similar behavior. See [`segmented.rs`](../../src-tauri/src/infrastructure/http_download_executor/segmented.rs). |
| **Range consistency and corruption recovery** | Diagnostic messages describe inconsistent parallel data and restarting with one connection. | The app validates initial and retried range status/bounds/total, encoding, and any returned entity validator. Segmented failures remove range parts and retry from byte zero with one stream; validator changes also clear stale parts. | **Comparable safety goals are implemented; runtime parity is untested.** Tests and real-server fault injection should still cover changed entities, ignored ranges, truncated responses, cleanup failures, and retry exhaustion. See [`segmented.rs`](../../src-tauri/src/infrastructure/http_download_executor/segmented.rs) and [`http_download_executor/mod.rs`](../../src-tauri/src/infrastructure/http_download_executor/mod.rs). |
| **Pause/resume** | Resume-related HTTP and FTP handling is represented, including messages for servers that cannot resume. | HTTP single-stream and segmented transfers support pause/resume; media engines have path-specific behavior. HTTP validators are carried in download history, while some media transfer paths are not uniformly resumable. | **Present, but not uniform.** Persist validator type as well as value: the current history field stores a string, so Last-Modified cannot be distinguished from ETag after restart. Test process-restart resume for each transfer type. See [`record.rs`](../../src-tauri/src/domain/download/record.rs), [`download_transfer_service.rs`](../../src-tauri/src/application/download_transfer_service.rs), and [`http_download_executor/mod.rs`](../../src-tauri/src/infrastructure/http_download_executor/mod.rs). |
| **Transient retry/reconnect** | HTTP/FTP connection handling and retry-related messages are represented. | Bounded request and mid-stream retries are implemented in the HTTP transfer modules. | **Present in core HTTP.** Add tests that prove retries cannot append bytes from a changed entity; make retry/fallback counts visible in diagnostics. |
| **Unknown file size** | Messages warn that a file may be incomplete when its size is unknown, and report bytes downloaded without claiming a known total. | Progress carries an optional total; the UI can show “Not supplied by server.” SABR leaves total unknown unless all selected formats provide lengths; paired media reports a sum only when both track sizes are known. | **Good parity in principle.** Preserve `None` for unknown totals and label extractor estimates as estimates. See [`sabr_executor.rs`](../../src-tauri/src/infrastructure/media/sabr_executor.rs), [`paired_executor.rs`](../../src-tauri/src/infrastructure/media/paired_executor.rs), and [`app.js`](../../renderer/app.js). |
| **Paired audio/video size and startup** | A setting/message describes starting transfer work while the Download File Info dialog is open. | Paired workers start without serial size probes; initial size is read from captured `clen` metadata when present and corrected from each response. A combined total is reported only when both track lengths are known. | **Good behavior in the examined path.** Compare first-byte latency and ensure the UI does not misrepresent estimated or unknown totals. See [`paired_executor.rs`](../../src-tauri/src/infrastructure/media/paired_executor.rs). |
| **Fast media-format selection** | Browser video-panel and format-selection behavior is represented; discovery latency and site coverage have not been compared. | The UI awaits native format discovery. yt-dlp format discovery runs `--dump-single-json` and has a 45-second timeout. | **Potential startup bottleneck.** Reuse fresh captured metadata and let already-available choices render while refresh occurs. Do not block a selected transfer on optional probing. See [`ytdlp.rs`](../../src-tauri/src/infrastructure/media/ytdlp.rs) and [`app.js`](../../renderer/app.js). |
| **YouTube and adaptive media** | YouTube/Google-video URL handling and a browser video panel are represented; exact protocol-level equivalence has not been established. | The app has YouTube browser capture, SABR, paired audio/video, yt-dlp, HLS, and playlist flows. | **A distinct app strength, not a proven parity comparison.** Keep transfer type and resume capability explicit per media path. README lists DRM/private/live-media limitations; see [`README.md`](../../README.md), [`sabr_executor.rs`](../../src-tauri/src/infrastructure/media/sabr_executor.rs), and [`hls_executor.rs`](../../src-tauri/src/infrastructure/media/hls_executor.rs). |
| **FTP** | The `FTPConnection` component has distinct connection, authentication, directory, metadata, passive/active data-channel, restart, retrieval, and reply-processing stages; diagnostic method names include `SendRetr`, `SendSize`, and `SendCWD`. | Download addresses reject every protocol except HTTP and HTTPS. | **Missing.** Add FTP only if it is within product scope; it requires a separate protocol executor, credentials policy, passive-mode behavior, size/range/resume handling, and tests. Do not bolt FTP into the HTTP executor. |
| **Proxy support** | HTTP proxy handling includes a Digest authorization format; its usage conditions are not established. | HTTP/HTTPS proxy configuration supports optional Basic authentication. | **Partial.** Digest support is not configured in the app. Confirm real user requirements before prioritizing it. See [`proxy.rs`](../../src-tauri/src/domain/settings/proxy.rs) and [`client.rs`](../../src-tauri/src/infrastructure/http_download_executor/client.rs). |
| **Download queues and concurrency** | Queue identifiers, queue initialization/error messages, and scheduler settings are represented. | The app has a queue pump, global concurrent-download limit, pause/start-queue actions, import/export, and persisted download history. | **Present at core level.** Verify queue state and scheduled work across restart with tests. See [`download_transfer_service.rs`](../../src-tauri/src/application/download_transfer_service.rs), [`download_history_service.rs`](../../src-tauri/src/application/download_history_service.rs), and [`menu.rs`](../../src-tauri/src/presentation/menu.rs). |
| **Scheduling** | Scheduler settings and related behavior are represented. | Scheduled timestamps are validated and due work is started by the scheduler service/window. | **Present.** The feature exists; this is not evidence that scheduling UX or recurrence controls match IDM. See [`download_history_service.rs`](../../src-tauri/src/application/download_history_service.rs) and [`window_manager.rs`](../../src-tauri/src/presentation/window_manager.rs). |
| **Bandwidth limits** | Speed-related controls and transfer behavior are represented; the rate-limiter algorithm has not been compared. | A shared token/time-based rate limiter is used by transfers and settings. | **Present, speed parity unmeasured.** Run multi-download tests and benchmark aggregate behavior before changing defaults. See [`download_rate_limiter.rs`](../../src-tauri/src/application/download_rate_limiter.rs). |
| **Categories and save locations** | Category-related behavior exists, but its complete rule set has not been established. | The app classifies downloads by file type and supports category-specific save directories. | **App feature present; parity unverified.** Compare automatic category rules, folder templates, and user overrides in a runtime/UI pass. See [`category.rs`](../../src-tauri/src/domain/category.rs) and [`settings.rs`](../../src-tauri/src/domain/settings.rs). |
| **Download history and portability** | Queue/history-related settings are represented; persistence format and export behavior have not been compared. | Download records persist locally; the app supports history import/export and actions on completed files. | **Basic history exists; direct parity is unverified.** Compare recovery after restart, history filtering, retention, and import/export compatibility separately. See [`download_history_service.rs`](../../src-tauri/src/application/download_history_service.rs) and [`commands.rs`](../../src-tauri/src/presentation/commands.rs). |
| **Download File Info and user actions** | A Download File Info dialog and optional transfer while it is open are represented. | The app dialog accepts URL(s), category, filename, destination, description, scheduling, and start/later choices. | **Similar workflow concepts.** Dialog layout, metadata completeness, and whether bytes start before confirmation are not established as equivalent. See [`index.html`](../../renderer/index.html) and [`download_transfer_service.rs`](../../src-tauri/src/application/download_transfer_service.rs). |
| **Desktop integration and completion actions** | Browser and desktop integration settings are represented; exact notification/tray behavior was not compared. | The app includes a system tray, optional autostart, completion notifications, reveal/open actions, and update checks. | **Different integration surfaces.** Validate each operating system and clean-install path. See [`tray.rs`](../../src-tauri/src/presentation/tray.rs), [`autostart.rs`](../../src-tauri/src/infrastructure/autostart.rs), and [`commands.rs`](../../src-tauri/src/presentation/commands.rs). |
| **Browser integration and capture** | Browser integration and video-panel behavior are represented; their full extent has not been compared. | Browser extensions support Chrome, Edge, Brave, Opera, and Firefox, with browser-to-desktop handoff and media capture. | **Present for the documented browser set.** Verify browser and download interception coverage with supported-browser tests. See [`README.md`](../../README.md), [`extension/`](../../extension/), and [`bridge/server.rs`](../../src-tauri/src/infrastructure/bridge/server.rs). |
| **Filename and server metadata** | HTTP response/header processing is present; full IDM filename precedence was not established. | Content-Disposition parsing is used by HTTP response handling; generic names are recognized by normalized stem, with URL/title fallbacks. | **Present at core level.** Test precedence across explicit user names, browser titles, extractor names, server headers, and signed URLs. See [`filename.rs`](../../src-tauri/src/domain/download/filename.rs), [`single_stream.rs`](../../src-tauri/src/infrastructure/http_download_executor/single_stream.rs), and [`download_transfer_service.rs`](../../src-tauri/src/application/download_transfer_service.rs). |
| **Visible source URL** | No detailed comparison of IDM's source/referrer display behavior was made. | UI distinguishes watch-page URLs from media endpoints and sanitizes displayed URLs to avoid exposing credentials/query tokens. | **Present with an explicit privacy advantage in the reviewed UI.** Confirm all diagnostics and error messages also avoid signed URLs. See [`app.js`](../../renderer/app.js) and [`filename.rs`](../../src-tauri/src/domain/download/filename.rs). |
| **Progress / connection visualization** | Per-connection chunk state and reassignment are represented. | Renderer shows segmented HTTP progress and paired track state; transfer records expose transfer kind and concurrency. yt-dlp reports its configured fragment concurrency limit of eight. | **Present, but metric semantics need refinement.** A configured fragment limit is not the same as active network connections; distinguish ranges, fragments, streams, paired tracks, and active top-level downloads. See [`app.js`](../../renderer/app.js), [`ytdlp.rs`](../../src-tauri/src/infrastructure/media/ytdlp.rs), and [`sabr_executor.rs`](../../src-tauri/src/infrastructure/media/sabr_executor.rs). |
| **Site Grabber / website-level batch retrieval** | Grabber project state includes `GrabberSts`, crawl depth and file counters, include/exclude filters, saved projects, and update behavior. | YouTube playlist selection/batch queue exists; no general website mirroring/crawler flow was found in the reviewed Rust application. | **Missing as a product capability.** This is separate from segmented file downloads and playlist queues; see the linked Site Grabber issue for bounded crawl requirements. |
| **Performance instrumentation** | No comparable runtime telemetry is available. | HTTP transfer completion and fallback are logged; first-request, range support, and known total are recorded. Response, first-byte, and retry metric methods are not wired, and the effective concurrency field is initialized from the requested value. | **Partial.** Wire first-response/byte and retry events, record actual active workers, and add privacy-safe structured benchmarks. No IDM-vs-app speed result is available. See [`metrics.rs`](../../src-tauri/src/infrastructure/metrics.rs) and [`http_download_executor/mod.rs`](../../src-tauri/src/infrastructure/http_download_executor/mod.rs). |

## Remaining gaps and recommended improvements

### Correctness and reliability

1. **Persist validator type.** Store validator kind and value (ETag versus
   Last-Modified) with partial transfer state. On restart, send the matching
   `If-Range`; if the original validator is unavailable, discard partial range
   data and restart safely.
2. **Exercise recovery paths.** Add deterministic HTTP-server tests for changed
   entity validators, incorrect `Content-Range`, ignored ranges, truncation,
   and range-part cleanup failures. Static checks do not prove these paths work
   under real network races.
3. **Measure resume support per media engine.** HTTP, HLS, SABR, yt-dlp, and
   paired downloads have different persistence and restart semantics; document
   them independently rather than promising generic resume.

### Performance and observability

1. **Instrument actual transfer events.** Record response headers, first byte,
   each retry, and actual active workers/fragments; currently several metric
   methods are unused.
2. **Benchmark like-for-like workloads.** Compare time-to-first-byte,
   completed throughput, retry/fallback rate, CPU and memory on controlled
   files and servers, including servers that disallow ranges. No speed claim
   should be made until this exists.
3. **Keep media discovery responsive.** Format discovery still depends on
   yt-dlp and can take up to its configured timeout; cached/captured formats
   should remain available while refresh errors are surfaced.

### Product-scope gaps

1. Add FTP as a separate protocol implementation only if compatibility
   requirements call for it.
2. Assess Digest proxy authentication against real user environments before
   extending the Basic-only proxy support.
3. Define the desired Site Grabber scope separately from playlist batching.
4. Continue improving browser interception, download dialogs,
   scheduling, queue management, and settings only where documented user
   stories require parity; this static comparison does not establish UI parity.

## Recommended implementation order

1. Add tests for the newly implemented reassignment and recovery behavior.
2. Persist validator kind alongside value and verify cross-process resume.
3. Finish transfer-event and effective-concurrency instrumentation.
4. Establish repeatable performance baselines before tuning chunk policies.
5. Decide separately whether FTP, Digest proxy auth, and Site Grabber are in
   product scope.

## What this report does not claim

- It does not claim IDM is always faster or always uses the maximum number of
  connections.
- It does not claim every observed message or setting is reachable or used by every
  download.
- It does not establish parity with every current product release.
- It does not treat the app's early-preview limitations as bugs when they are
  an intentional product-scope choice.
