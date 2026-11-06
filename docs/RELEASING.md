# Release checklist

Tauri release automation is not configured. Builds and releases are produced
manually. Do not publish an artifact until the checks below pass.

## Build

```bash
bun install --frozen-lockfile

# Windows x64
bun run build:win

# Windows ARM64
bun run build:win:arm64

# macOS
bun run build:mac

# Linux
bun run build:linux
```

Windows artifacts are written to
`target/<target-triple>/release/bundle/nsis/`. macOS and Linux artifacts are
written under `target/release/bundle/`.

## Required checks

1. Run the Rust and extension regression suites on Windows x64 and Windows
   ARM64.
2. Build the Windows installer and verify it on a clean machine without Node.js:
   install, launch, history/settings migration, browser handoff, direct and
   segmented downloads, pause/resume, media assembly, upgrade, and uninstall.
3. For Windows ARM64, complete
   [`WINDOWS-ARM64.md`](WINDOWS-ARM64.md) on physical Windows 11 ARM64 hardware.
   Confirm the app and native host are native ARM64:
   `scripts/verify-pe-arch.ps1 -Path <exe> -Expected arm64`.
4. Confirm the ARM64 and x64 installers have distinct filenames and cannot
   overwrite each other. Record both SHA-256 hashes.
5. Record installer and installed-size measurements, startup and idle-memory
   results, and download benchmark results against the Phase 1 baseline.
6. Review the final artifact for private data, bundled files, license notices,
   provenance, and source-distribution obligations.
7. Publish a prerelease until clean-machine checks and known limitations are
   documented. Do not attach research, browser profiles, signed media links, or
   user data.

Code signing, updater signing, browser-store publication, and Tauri release
automation are not configured. Windows ARM64 is not a released capability until
hardware validation and maintainer release review are complete.
