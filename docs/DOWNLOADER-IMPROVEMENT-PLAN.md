# Downloader Improvement Plan

## Goal

Make downloads feel immediate and use available bandwidth without sacrificing
correctness. First remove avoidable waiting and fix metadata/progress semantics;
then tune the existing transfer paths based on measurements. Do not start with
a wholesale rewrite: this repository already has HTTP range segmentation,
validators, retries, resumable segment files, media-specific download paths,
and yt-dlp fragment concurrency.

## Current implementation relevant to the reported problems

- Standard HTTP downloads already probe `Range: bytes=0-0`, require a
  consistent partial-content response and a known total, and divide the file
  into concurrent ranges when multiple connections are requested. Segment
  requests send an `If-Range` validator; responses are checked against the
  requested range and rejected when a returned entity validator disagrees.
  See [`segmented.rs`](../src-tauri/src/infrastructure/http_download_executor/segmented.rs),
  [`validator.rs`](../src-tauri/src/infrastructure/http_download_executor/validator.rs),
  and [`mod.rs`](../src-tauri/src/infrastructure/http_download_executor/mod.rs).
- The HTTP path falls back to a normal stream when safe segmentation cannot be
  established. Therefore, a connection-count preference alone does not
  guarantee that a given server or media path will use several ranges.
- YouTube/site format discovery invokes yt-dlp with `--skip-download` and
  `--dump-single-json`, with a 45-second timeout. The yt-dlp transfer path asks
  for eight concurrent fragments. See [`ytdlp.rs`](../src-tauri/src/infrastructure/media/ytdlp.rs).
- Paired audio/video downloads probe both resources for size and then stream
  the tracks concurrently. This is two media streams, not necessarily
  segmented HTTP. See [`paired_executor.rs`](../src-tauri/src/infrastructure/media/paired_executor.rs).
- SABR can report an unknown total when selected formats do not provide
  content lengths. See [`sabr_executor.rs`](../src-tauri/src/infrastructure/media/sabr_executor.rs).
- The renderer can show “Not supplied by server” for missing totals, an
  overall progress bar, and a segment map. Video naming also has a generic
  `"video"` fallback. See [`app.js`](../renderer/app.js) and
  [`index.html`](../renderer/index.html).

## Prioritized phases

### Phase 0 — Measure before changing transfer policy

**Purpose:** Determine where the perceived delay and lost throughput actually
occur.

- Add structured timings for format discovery, metadata/size probes, download
  record creation, first request, first response headers, first byte, and
  completion. Keep URLs and credentials out of ordinary logs.
- Tag progress events by transfer kind: single HTTP stream, HTTP segments,
  paired audio/video, SABR, or yt-dlp fragments.
- Record requested versus effective concurrency, server range support,
  known/unknown total, retries, fallback reason, and aggregate bytes per
  second.
- Establish baseline measurements for time-to-chooser, time-to-first-byte,
  and throughput on representative direct files and media URLs.

**Exit condition:** A repeatable baseline separates UI/discovery delay from
connection setup and transfer throughput.

### Phase 1 — Make selection and first-byte startup responsive

**Purpose:** Avoid blocking the user's selection on work that can happen in
parallel or be reused.

- Trace the format chooser from extension metadata capture/prefetch through
  native format discovery. Reuse fresh captured choices; when a refresh is
  useful, run it without blocking already-available choices.
- For a selected format, create the transfer promptly. Run optional size and
  metadata probes concurrently with the first useful media request where safe;
  do not serialize HEAD requests ahead of all media activity.
- Keep the UI responsive and report discovery/connection state separately
  from active transfer progress. Do not claim a download has started until
  work or bytes have actually started.
- Preserve cancellation, pause, authentication, and stale-format handling
  while reducing wait time.

**Exit condition:** Selection-to-first-request and selection-to-first-byte
improve without reducing format correctness or hiding errors.

### Phase 2 — Correct size and URL metadata

**Purpose:** Remove misleading totals and “fake URL” displays.

- Keep exact server-provided total, extractor estimate, and unknown as distinct
  values. Never convert absent or zero-as-missing extractor size into an exact
  total.
- Prefer trustworthy response metadata (`Content-Length`, valid
  `Content-Range`, and applicable media metadata); mark estimates as estimates
  and keep unknown totals unknown.
- Display the actual media/download endpoint as the download source when
  useful, and show the originating page/referrer separately. Do not label a
  watch page as if it were the media file URL.
- Add tests for absent length, chunked/unknown-length responses, malformed
  ranges, estimates, and redirects.

**Exit condition:** The size and URL shown to the user match their provenance
and cannot silently present an estimate/referrer as an exact media endpoint.

### Phase 3 — Tune concurrency and fallback on existing transfer engines

**Purpose:** Use available bandwidth without assuming more connections always
means more speed.

- Preserve the existing segmented HTTP implementation and first verify its
  request scheduling, configured connection limit, shared rate limiter, retry
  behavior, and resume correctness under parallel load.
- Make effective concurrency observable and test the cases that disable
  segmentation: no `206`, invalid/missing total, encoded body, or no usable
  entity validator.
- Benchmark conservative connection counts against a single stream. Choose
  defaults from repeatable results; keep user settings bounded and honor
  server limits/errors.
- Ensure all segment requests use the same relevant request context and
  entity identity. Reject inconsistent ranges; never concatenate data from
  different entity versions. Retain safe retry and single-stream fallback
  behavior.
- For media, distinguish yt-dlp fragment concurrency from HTTP byte-range
  concurrency. Avoid layering competing concurrency controls blindly.

**Exit condition:** Tests prove correct output and safe fallback across range
support, validator, retry, resume, and changed-entity cases; benchmarks show
whether throughput actually improves.

### Phase 4 — Clarify progress and connection semantics

**Purpose:** Eliminate the “double progress bar” confusion without losing
useful detail.

- Keep one clear overall progress indicator for the complete output.
- Show per-range bars only for segmented HTTP downloads. For paired media,
  label the two tracks “Video” and “Audio”; for extractor downloads, show
  fragment/overall progress only when the source provides it.
- Rename generic “connections” UI to match the transfer kind. A media track is
  not a TCP connection, and an overall bar plus a per-worker map should not
  look like two competing totals.
- For unknown totals, show bytes downloaded, rate, and an indeterminate state
  rather than a misleading percentage.

**Exit condition:** UI tests cover single-stream, segmented, paired, SABR, and
unknown-total states with one unambiguous overall status.

### Phase 5 — Preserve meaningful filenames end to end

**Purpose:** Avoid generic names such as `video`, `watch`, or a raw page title.

- Define filename precedence once: explicit user name, trusted
  `Content-Disposition` filename, extractor media title plus selected
  container/extension, then a sanitized URL-derived fallback.
- Carry the chosen title/name through extension metadata, format choices,
  download records, paired/SABR paths, and final output.
- Sanitize path separators and invalid characters, preserve the correct
  extension, and handle collisions without silently overwriting a file.
- Avoid using generic page titles or `"video"` as the final fallback when a
  stable media ID or URL basename is available.

**Exit condition:** Tests assert final names for direct downloads, separate
audio/video tracks, merged outputs, missing titles, and invalid/colliding names.

## Validation matrix

Before calling the work complete, cover at least:

| Scenario | Expected behavior |
|---|---|
| Range-capable server with stable validator | Concurrent valid ranges; exact assembled bytes |
| Server ignores ranges or omits a usable total | Single-stream fallback; no fabricated percentage |
| Entity changes while resuming | Discard stale parts or restart safely |
| A segment stalls or fails | Bounded retry/resume; visible error if exhausted |
| Media exposes no exact size | Honest unknown/indeterminate total |
| Paired audio and video | Concurrent track work; clearly labeled track progress |
| Format metadata is already fresh | Immediate chooser; no redundant blocking discovery |
| Extractor metadata is stale or unavailable | Refresh/fallback with surfaced error, not a false success |
| Title/filename missing or unsafe | Stable sanitized filename with correct extension |

## Recommended implementation order

1. Instrument and baseline (Phase 0).
2. Remove avoidable chooser and probe serialization (Phase 1).
3. Correct size and source-URL semantics (Phase 2).
4. Test and tune existing concurrency/fallback logic (Phase 3).
5. Align progress labels with transfer type (Phase 4).
6. Trace and fix filename propagation (Phase 5).

This order targets the reported “takes too long to connect / start” problem
first, while ensuring later speed tuning is based on actual transfer behavior
rather than simply increasing a connection count.
