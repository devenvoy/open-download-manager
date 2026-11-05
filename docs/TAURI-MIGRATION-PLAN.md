# Electron to Rust + Tauri Migration Plan

## Architecture and implementation rules

These rules apply to every migration phase and to new code added during the
transition.

### Clean architecture and module boundaries

- Organize code into clear layers with one-way dependencies:
  - **Domain:** core models, value types, invariants, and business rules. Keep
    this layer independent of Tauri, the UI, the filesystem, and network
    libraries.
  - **Application:** use cases and orchestration. Depend on domain types and
    define the interfaces needed by external services.
  - **Infrastructure:** implementations for HTTP, storage, media tools,
    operating-system integration, and browser messaging.
  - **Presentation:** Tauri commands/events and UI code. Translate at the
    boundary; do not put business rules in commands or renderer handlers.
- Keep dependencies pointed inward. Domain code must not import Tauri or
  infrastructure modules; use traits at boundaries where inversion is needed.
- Give each module one clear responsibility. Split modules that become difficult
  to navigate, and group closely related code under a named feature or domain
  rather than creating catch-all modules.
- Keep Tauri commands thin: validate boundary input, call an application
  operation, and map its typed result to a response.

### Files, types, and naming

- Give each substantial struct its own clearly named Rust module/file, with its
  associated implementation in that module. Split an implementation into a
  separate sibling module only when it has a distinct responsibility or is
  large enough to hinder navigation.
- Avoid files that collect unrelated structs, implementations, or helper
  functions. Keep small, tightly coupled private helpers with their owning
  module.
- Use descriptive, human-readable names that express the domain and behavior.
  Avoid cryptic abbreviations, generic names such as `Manager` without a clear
  scope, and names that expose implementation details unnecessarily.
- Use Rust visibility deliberately: keep items private by default and expose
  only the types and operations required by other modules.

### Constants and configuration

- Do not scatter hard-coded limits, timeouts, retry counts, protocol names, or
  other policy values throughout implementation code.
- Put fixed values in named constants near the module that owns the policy.
  Use focused constants modules only when values are shared across multiple
  modules; avoid a single global catch-all constants file.
- Put user-configurable or environment-dependent values in typed configuration
  structures, with validation at the point where configuration enters the
  application.
- Keep secrets out of source, default configuration, persisted settings, and
  logs.

### Type safety, validation, and errors

- Prefer enums and dedicated value types over stringly typed states, actions,
  categories, and modes. Make invalid states difficult to represent.
- Use exhaustive `match` expressions for state and enum handling; avoid
  catch-all branches when they could hide a newly added case.
- Validate untrusted input at application boundaries and convert it into
  validated domain types before using it.
- Return explicit, typed errors through each layer. Do not silently discard
  failures or replace them with success-shaped defaults.
- Use checked conversions and explicit handling for optional or fallible values;
  avoid unchecked assumptions and unnecessary casts.

### Performance, reuse, and maintainability

- Choose efficient algorithms and bounded resource use appropriate to the
  measured workload. Avoid unbounded queues, task creation, buffering, or
  parallelism.
- Measure before optimizing, and benchmark changes against the Phase 1 baseline.
  Do not trade download integrity, cancellation, privacy, or reliability for an
  unverified performance gain.
- Reuse a shared implementation when multiple features have the same stable
  responsibility. Prefer simple composition and focused traits over duplicate
  logic or broad abstractions created only for hypothetical future needs.
- Keep interfaces small and explicit. Avoid unnecessary configurability,
  indirection, global mutable state, and tight coupling between UI, domain, and
  operating-system code.
- Add unit tests for domain rules and focused integration tests for module
  boundaries. Test error paths, limits, and state transitions as well as success
  cases.

## Goal

Replace the current Electron desktop shell and JavaScript download backend with
a Rust + Tauri application. The intended benefits are a smaller installer,
faster startup, and lower idle memory use, while preserving the current download
and browser-integration behavior.

This is a staged migration, not a one-step rewrite. Rust and Tauri may reduce
framework overhead, but they do not automatically improve download throughput.
Measure the current application and the Tauri implementation to verify gains.

## Pre-migration architecture (Electron reference)

- **Desktop shell:** Electron, previously packaged for Windows x64 with `electron-builder`.
- **UI:** Plain HTML, CSS, and JavaScript in `renderer/`.
- **Download engine:** JavaScript modules in `core/`, including HTTP transfers,
  segmented downloads, HLS, queueing, scheduling, and persistence.
- **Browser handoff:** Browser extension, local authenticated bridge, and a
  Windows native-messaging host.
- **Media tools:** FFmpeg and yt-dlp are separately bundled executables.
- **User data:** Download history and settings are stored under
  `%USERPROFILE%\.awesome-downloader`.

The renderer and browser extension remain in the active source tree. The legacy
shell and backend are archived locally under the ignored
`reference/electron-legacy/` directory; their tracked pre-migration source is
available at commit `302fc1c`. The backend, browser handoff, packaging, and
update process need deliberate migration and compatibility work.

## Principles

1. Keep previously published Windows releases available until the replacement
   passes release gates.
2. Preserve download behavior, user data, and browser-extension behavior unless
   a change is separately scoped and tested.
3. Keep credentials and signed media URLs transient; do not persist or log them.
4. Separate the Tauri shell migration from the download-engine rewrite where
   practical.
5. Set measurable performance targets from a Windows baseline instead of
   assuming a framework swap meets them.

## Phases

### 1. Establish a Windows baseline

- Build the current Windows x64 installer and record its SHA-256 hash.
- Measure installer size, installed size, cold startup time, and idle memory.
- Benchmark representative HTTP and segmented downloads, pause/resume, and
  video/audio assembly.
- Record the Windows version, hardware, runtime version, and test procedure so
  later results are comparable.
- Define target reductions and acceptable behavior/performance regressions
  after collecting those measurements.
- Follow the [Windows baseline runbook](./TAURI-MIGRATION-BASELINE.md) and use
  its capture helper for package, startup, and idle-memory measurements.

**Exit criteria:** Baseline results and target thresholds are documented and
repeatable.

### 2. Add a Tauri shell and reuse the renderer

- Create the Tauri application and configure its Windows build.
- Load the existing renderer before making broad UI changes.
- Replace Electron preload/IPC calls with a small, explicit Tauri
  command-and-event API.
- Reimplement only the shell capabilities the UI needs, including window
  management, tray behavior, notifications, file dialogs, and startup behavior.
- Keep the API typed and narrow; do not expose unrestricted filesystem or
  process access to the webview.
- Keep each Rust layer and test in focused modules/files; do not place domain,
  application, storage, and command logic in the Tauri entry point.

**Exit criteria:** The Tauri app launches, renders the current UI, and passes
smoke tests for its shell commands and events.

### 3. Define compatibility contracts and protect user data

- Specify the payloads and error formats for UI commands, progress events, and
  browser handoff.
- Preserve `%USERPROFILE%\.awesome-downloader` as the data location where
  possible.
- Add versioned migration logic for history and settings before changing their
  format.
- Ensure interrupted or failed migrations leave the original data recoverable.
- Add contract tests for state transitions, serialization, and error handling.

**Exit criteria:** Existing history/settings can be read by the new app, and
tests cover the migration and compatibility boundaries.

### 4. Port the download backend incrementally

Port functionality in independently testable slices:

1. Queueing, scheduling, cancellation, progress reporting, persistence, and
   regular HTTP downloads.
2. Segmented transfers, throttling, retry behavior, and resume/recovery.
3. HLS handling and media assembly integration.
4. YouTube/SABR transport and format selection, after resolving its dependency
   strategy.

The existing YouTube SABR path depends on the JavaScript `googlevideo` package.
Choose and validate one of these approaches before removing the Node runtime:

- Keep a narrowly scoped sidecar for this functionality temporarily; or
- Replace it with a Rust implementation and prove functional compatibility.

Keep FFmpeg and yt-dlp as separate tools initially. Reassess bundling only after
the core migration works. Review their licenses, notices, provenance, and
distribution obligations as part of packaging work.

**Exit criteria:** The Rust backend passes behavior and regression tests for
each migrated slice, including cancellation, integrity, recovery, and media
assembly.

### 5. Migrate browser integration

- Keep the browser extension and its user-facing behavior unchanged initially.
- Replace the Electron-dependent Windows native host with a Rust native-messaging
  host.
- Preserve the authenticated, loopback-only bridge and request validation.
- Preserve browser registration, extension setup, and uninstall cleanup.
- Verify that cookies, authorization headers, and signed URLs remain transient
  and never enter download history, logs, or diagnostics.

**Exit criteria:** Supported browser handoff works against the Tauri app in
fixture tests and on a real Windows browser installation.

### 6. Replace packaging and updates

- Replace `electron-builder` packaging with Tauri's Windows packaging flow.
- Recreate installer, portable-build (if retained), install-directory,
  shortcut, upgrade, and uninstall behavior.
- Preserve browser registration cleanup and user data across upgrades.
- Design updater signing, key storage, release automation, and user consent
  before enabling Tauri updates.
- Audit final artifacts for bundled files, size, notices, and third-party
  licensing requirements.

**Exit criteria:** The app builds reproducibly and installs/upgrades/uninstalls
cleanly on a Windows machine without development tools.

### 7. Validate and roll out

- Run the existing regression suite and new Rust, UI/backend contract, and
  integration tests.
- Test on a clean Windows x64 machine without Node.js installed.
- Verify install, first launch, settings/history migration, browser handoff,
  direct and segmented downloads, pause/resume, media assembly, updates, and
  uninstall.
- Compare installer size, installed size, startup time, idle memory, and
  representative transfer behavior against the baseline.
- Keep previously published Electron releases available as a user fallback
  until the Tauri candidate passes all gates; do not overwrite or replace them
  before a deliberate release decision.

**Exit criteria:** No critical regressions remain, all required release checks
pass, and measured results meet the agreed performance targets. Phase 1
measurements are deferred and do not block Tauri implementation.

## Current implementation status

- **Phase 1:** Deferred by the maintainer; baseline measurements are not a
  blocker for implementing the Tauri app. Capture them later only if a measured
  performance comparison is needed.
- **Phase 2:** A Tauri v2 shell now loads the existing renderer, serves an
  initial snapshot, persists validated settings, provides native folder
  selection and clipboard reads, and opens/resizes/closes a native download
  details window. Other window workflows, tray behavior, notifications, and
  full command/event parity remain to be migrated.
- **Phase 3:** On first run, validated settings are imported from a version 1
  legacy `downloads.json` into versioned `settings.json`. The legacy history
  file remains unchanged, and invalid or unsupported legacy data fails
  explicitly. The Tauri snapshot now reads version 1 download records without
  rewriting the legacy file, resets interrupted runtime statuses to paused,
  and excludes transient headers, validators, segmented URLs, and SABR session
  data and URL credentials, query strings, and fragments from renderer
  snapshots. New downloads use a separate, versioned `downloads-tauri.json`
  history file written atomically, leaving the legacy history source intact.
  Categories and transfer protocols are represented by domain enums while
  retaining the renderer's existing serialized category names. Command/event
  compatibility is only partially implemented.
- **Phase 4:** The native download engine is complete across all planned transfer
  types: regular single-stream HTTP transfers, byte-range segmented transfers
  with connection-level progress reporting and resume, scheduling, shared
  bandwidth rate limiting, bounded exponential backoff retries on connect/probe
  transient errors and 408/429/5xx status codes, and automatic mid-stream
  recovery with range-based resumption. Media handling is implemented for HLS VOD
  playlists (resolving master and media playlists, downloading segments
  concurrently, estimating stream size, and assembling/muxing via FFmpeg),
  audio/video track conversion (MP4 and MP3 via FFmpeg), and YouTube format
  resolution via yt-dlp.
- **Phase 5:** Browser integration has been migrated from Electron to native Rust:
  an authenticated, loopback-only bridge (`127.0.0.1`) runs with cryptographically
  secure Bearer tokens, CSRF protection (mandatory rejection of `Origin` headers
  and validation of `Host`), and health/formats/downloads endpoints. A dedicated
  `awesome-downloader-native-host` binary implements the standard WebExtension
  stdio native messaging protocol, and browser registration manifests are
  generated for Chrome, Edge, Brave, Opera, and Firefox.
- **Phase 6:** Packaging is configured for Tauri v2 Windows NSIS builds:
  `tauri.conf.json` bundles the main application, native host binary, browser
  extension assets, application and installer icons, and sets currentUser
  per-user installation mode with desktop/start menu shortcuts.
- **Phase 7:** Release validation: all 118 Rust domain, application, infrastructure,
  presentation, media/sabr, and integration tests pass, and all 88 browser extension and renderer tests
  pass without regression (206 total passing automated tests). The Auto-Updater
  with 6-hour background polling and active transfer guard (Phase F) and YouTube SABR
  Streaming Protocol & zero-copy Protobuf parser (Phase G) are complete,
  achieving 100% behavioral parity with the legacy Electron application. Clean Windows x64 deployment
  and final artifact audits can be validated using the defined release gates.

## Release gates

- **Size:** Installer and installed-size reductions are measured against the
  baseline and meet the agreed thresholds.
- **Performance:** Startup and idle-memory targets are met without unacceptable
  regressions in download throughput or stability.
- **Compatibility:** Existing history/settings are preserved or safely migrated.
- **Integrity:** Download verification, pause/resume, cancellation, and recovery
  remain correct.
- **Browser integration:** Native messaging and browser setup work on a clean
  Windows installation.
- **Privacy and security:** No credentials or signed media URLs are persisted or
  exposed through logs, and Tauri commands enforce narrow permissions.
- **Distribution:** Installer, update, and third-party licensing checks pass.

## Risks and decisions to resolve

| Risk or decision | Mitigation |
| --- | --- |
| A Tauri shell reduces framework overhead but not the size of FFmpeg, yt-dlp, or other bundled tools. | Measure current and final payloads; investigate large assets separately. |
| A Rust rewrite can introduce transfer, resume, or media regressions. | Port incrementally and preserve the existing regression tests as behavioral requirements. |
| The current SABR dependency is JavaScript-based. | Decide on a temporary sidecar or validated Rust replacement before removing Node-related runtime support. |
| Native messaging and browser registration are Windows-specific and operationally sensitive. | Keep the extension stable and test the Rust host against real installed browsers. |
| User data may be lost or become unreadable during schema changes. | Preserve the current data directory and use versioned, recoverable migrations. |
| Update signing and key handling are new release responsibilities. | Define signing, secret handling, and release procedures before enabling updates. |
| Tauri does not automatically add Linux or macOS support. | Treat those as separate platform goals with their own integration and device validation. |

## Out of scope for the initial migration

- Claiming Linux or macOS support solely because Tauri can target those systems.
- Replacing FFmpeg or yt-dlp as part of the shell migration.
- Rewriting the browser extension without a separately validated need.
- Adding unrelated features or changing download semantics during the port.

## Repository references

- Tauri configuration and Rust application: [`src-tauri/`](../src-tauri/)
- Reused renderer: [`renderer/`](../renderer/)
- Browser extension retained during migration: [`extension/`](../extension/)
- Legacy Electron source: local ignored `reference/electron-legacy/` archive or
  the tracked source at commit `302fc1c`.
- Third-party component notes: [`THIRD-PARTY.md`](../THIRD-PARTY.md)
- Release checklist: [`RELEASING.md`](./RELEASING.md)
- Contributor roadmap: [`CONTRIBUTOR-START-HERE.md`](./CONTRIBUTOR-START-HERE.md)
