---
number: 4
title: "[Goal] Windows ARM64 support"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:44Z
updated_at: 2026-11-05T19:49:44Z
url: https://github.com/devenvoy/open-download-manager/issues/4
labels: ["enhancement", "help wanted", "type: goal"]
---

# Issue #4: [Goal] Windows ARM64 support

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:44Z`
- **Labels**: `enhancement`, `help wanted`, `type: goal`
- **Issue Link**: [#4 on GitHub](https://github.com/devenvoy/open-download-manager/issues/4)

---

## Description

> *Mirrored from upstream issue [#8](https://github.com/Ahmadkhattak1/awesome-downloader/issues/8) originally opened by @Ahmadkhattak1 on 2026-10-25T21:00:25Z*

## Work order — Windows ARM64
This is the tracking goal. Pick a step below rather than opening a PR for the entire goal.

1. #14 — Extract tested OS and architecture adapters without changing Windows behavior. Start here after agreeing scope.
2. #26 — Windows ARM64: audit and implement runtime/native dependency compatibility. Depends on #14 being merged and validated.
3. #27 — Windows ARM64: installer, distinct artifacts and hardware validation. Depends on #26 being merged and validated.

Each PR should use `Closes #<implementation-issue>` only when it completes that issue, and `Refs #8` for this goal. Do not auto-close this parent from an individual step. Mark steps complete after merge and validation; close the goal only after its acceptance criteria and maintainer release review are satisfied. When a prerequisite completes, the maintainer changes the next issue from `status: blocked` to `status: ready`. Draft work may be discussed early but must not be merged ahead of its dependency.

## Objective
The maintainer has approved Windows ARM support. Current build scripts and NSIS configuration explicitly target x64.

## Acceptance criteria
- Produce a distinct ARM64 installer/artifact with the correct Electron runtime.
- Audit the native host and bundled media binaries for ARM64 compatibility; document any dependency gaps before choosing fallbacks.
- Verify installation, browser registration/handoff, downloads, merging, progress/completion and uninstall on Windows ARM hardware.
- Keep x64 releases available and verify architecture-specific artifacts cannot overwrite one another.
- Include build instructions, dependency licensing notices and a tested support matrix.

## Contribution workflow
This objective is approved by the maintainer. Comment with your proposed approach and test plan before starting, so scope and ownership can be agreed. Keep each PR focused and link this issue. Additional features and broader IDM-parity work require maintainer approval first. Do not include cookies, signed media URLs or browser profiles in public evidence.

## Delivery plan
- [ ] Shared architecture-aware adapters: #14
- [ ] ARM64 runtime/dependency compatibility: #26
- [ ] ARM64 installer and hardware validation: #27 (after #26)
Keep this goal open until the native ARM64 support matrix is validated. Emulation is not proof of native compatibility.

