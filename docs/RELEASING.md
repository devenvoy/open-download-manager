# Release checklist

Windows packaging and release automation are paused while the Tauri migration is
in progress. The current Tauri configuration does not produce an installer.
Do not run the old Electron release workflow or publish an artifact from this
branch as a replacement release.

Before any Tauri release:

1. Complete all phases and release gates in the
   [migration plan](TAURI-MIGRATION-PLAN.md).
2. Run the Rust and extension regression suites on Windows x64.
3. Build the Windows installer and verify it on a clean machine without Node.js:
   install, launch, history/settings migration, browser handoff, direct and
   segmented downloads, pause/resume, media assembly, upgrade, and uninstall.
4. Record installer and installed-size measurements, startup and idle-memory
   results, and download benchmark results against the Phase 1 baseline.
5. Review the final artifact for private data, bundled files, license notices,
   provenance, and source-distribution obligations. Record hashes.
6. Publish a prerelease until clean-machine checks and known limitations are
   documented. Do not attach research, browser profiles, signed media links, or
   user data.

Code signing, updater signing, browser-store publication, and Tauri release
automation are not configured.
