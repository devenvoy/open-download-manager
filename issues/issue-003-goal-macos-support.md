---
number: 3
title: "[Goal] macOS support"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:41Z
updated_at: 2026-11-05T19:58:25Z
url: https://github.com/devenvoy/open-download-manager/issues/3
labels: ["enhancement", "help wanted", "type: goal"]
---

# Issue #3: [Goal] macOS support

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:41Z`
- **Labels**: `enhancement`, `help wanted`, `type: goal`
- **Issue Link**: [#3 on GitHub](https://github.com/devenvoy/open-download-manager/issues/3)

---

## Description

> *Mirrored from upstream issue [#7](https://github.com/Ahmadkhattak1/awesome-downloader/issues/7) originally opened by @Ahmadkhattak1 on 2026-10-25T21:00:23Z*

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

