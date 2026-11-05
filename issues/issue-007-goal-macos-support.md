---
number: 7
title: "[Goal] macOS support"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:00:23Z
updated_at: 2026-10-29T12:44:38Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/7
labels: ["enhancement", "help wanted", "type: goal"]
assignees: []
comments_count: 2
---

# Issue #7: [Goal] macOS support

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:00:23Z`
- **Updated**: `2026-10-29T12:44:38Z`
- **Labels**: `enhancement`, `help wanted`, `type: goal`
- **Original Issue**: [#7 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/7)

---

## Description

## Work order — macOS
This is the tracking goal. Pick a step below rather than opening a PR for the entire goal.

1. #14 — Extract tested OS and architecture adapters without changing Windows behavior. Start here after agreeing scope.
2. #24 — macOS: desktop runtime and native messaging integration. Depends on #14 being merged and validated.
3. #25 — macOS: package app and media tools, signing plan and clean-install validation. Depends on #24 being merged and validated.

Each PR should use `Closes #<implementation-issue>` only when it completes that issue, and `Refs #7` for this goal. Do not auto-close this parent from an individual step. Mark steps complete after merge and validation; close the goal only after its acceptance criteria and maintainer release review are satisfied. When a prerequisite completes, the maintainer changes the next issue from `status: blocked` to `status: ready`. Draft work may be discussed early but must not be merged ahead of its dependency.

## Objective
The maintainer has approved macOS support. Current installer, native-host launcher, integration registration and media-tool packaging are Windows-specific.

## Acceptance criteria
- Propose minimum macOS version, Intel/Apple Silicon coverage and installer format before implementation.
- Implement platform-specific data paths, native messaging registration, launcher and media-tool packaging.
- Document signing/notarization requirements and any release credentials needed from the maintainer; never commit credentials.
- Verify clean installation, browser handoff, downloads, audio/video merging, progress/completion windows, upgrade and uninstall on the agreed targets.
- Provide reproducible builds and tests without regressing Windows.

## Contribution workflow
This objective is approved by the maintainer. Comment with your proposed approach and test plan before starting, so scope and ownership can be agreed. Keep each PR focused and link this issue. Additional features and broader IDM-parity work require maintainer approval first. Do not include cookies, signed media URLs or browser profiles in public evidence.

## Delivery plan
- [ ] Shared platform adapters: #14
- [ ] macOS runtime/native messaging: #24 (after #14)
- [ ] macOS packaging, signing plan and validation: #25 (after #24)
Use focused PRs. Keep the parent open until the agreed macOS targets are validated and release is approved.



---

## Comments (2)

### Comment #1 by [@toseef847](https://github.com/toseef847) • `2026-10-29T09:47:13Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/7#issuecomment-6493397663)

**Looking for a tester with an Intel Mac** for the macOS packaging PR #69 (step 3, #25).

The x64 DMG builds and passes a launch smoke test on GitHub's `macos-15-intel` runner, but Intel support won't be claimed until it has run on real Intel hardware. If you have an Intel Mac on macOS 13 or later, please:

1. Build it: `npm ci && npm run dist:mac:x64` on the PR branch (the build fails if any binary isn't x86_64). Or use the `macos-x64` artifact from a release-candidate run once there is one.
2. Install `release/Awesome-Downloader-<version>-macOS-x64.dmg` to /Applications and launch it. It is ad-hoc signed, so a downloaded copy needs `xattr -dr com.apple.quarantine`.
3. Load the extension from Browser Integration into Chrome or Edge. Send a normal download and a YouTube download that merges audio and video. Check that progress and completion windows appear.
4. Install a newer build over it, then uninstall as described in docs/RELEASING.md#macos.

Please report your Mac model, macOS version, `uname -m`, and anything that failed, with logs, on #69.

---

### Comment #2 by [@toseef847](https://github.com/toseef847) • `2026-10-29T12:44:38Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/7#issuecomment-6497020290)

Proposed scope for the first macOS preview, from #68 and #69. Please confirm or adjust:

- **Minimum macOS: 13.0 (Ventura).** That's the `LSMinimumSystemVersion` Electron 44 sets in the built app, for both architectures. It isn't 12.
- **Apple Silicon: validated.** I tested on an M1 with macOS 26.6.2: clean build, DMG install, launch, Chrome native-host handoff, upgrade and uninstall.
- **Intel: pending hardware.** The x64 DMG builds and passes the architecture check and launch smoke test on GitHub's `macos-15-intel` runner. Until someone tests it on a real Intel Mac, it isn't claimed as supported. If you have one, the `unsigned-macos-x64-release-candidate` artifact from #69's release workflow is the build to try.
- **Installer:** one DMG per architecture, signed with Developer ID and notarized once the signing secrets are added.

---
