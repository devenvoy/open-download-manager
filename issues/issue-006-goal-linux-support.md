---
number: 6
title: "[Goal] Linux support"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:00:20Z
updated_at: 2026-10-25T21:17:35Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/6
labels: ["enhancement", "help wanted", "type: goal"]
assignees: []
comments_count: 0
---

# Issue #6: [Goal] Linux support

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:00:20Z`
- **Updated**: `2026-10-25T21:17:35Z`
- **Labels**: `enhancement`, `help wanted`, `type: goal`
- **Original Issue**: [#6 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/6)

---

## Description

## Work order — Linux
This is the tracking goal. Pick a step below rather than opening a PR for the entire goal.

1. #14 — Extract tested OS and architecture adapters without changing Windows behavior. Start here after agreeing scope.
2. #22 — Linux: desktop runtime and native messaging integration. Depends on #14 being merged and validated.
3. #23 — Linux: package media dependencies and validate install/upgrade/uninstall. Depends on #22 being merged and validated.

Each PR should use `Closes #<implementation-issue>` only when it completes that issue, and `Refs #6` for this goal. Do not auto-close this parent from an individual step. Mark steps complete after merge and validation; close the goal only after its acceptance criteria and maintainer release review are satisfied. When a prerequisite completes, the maintainer changes the next issue from `status: blocked` to `status: ready`. Draft work may be discussed early but must not be merged ahead of its dependency.

## Objective
The maintainer has approved Linux support. Current packaging targets Windows x64 and the native host setup uses reg.exe, native-host.exe and Windows paths.

## Acceptance criteria
- Propose the initial supported distributions, architectures and package formats in this issue before implementation.
- Make desktop startup, data paths, native-host launch/registration and bundled media tools work on the agreed Linux targets.
- Provide installation, extension setup, upgrade and uninstall instructions.
- Verify download, pause/resume where supported, audio/video merging, progress/completion windows and browser handoff on a clean Linux installation.
- Include reproducible build instructions and platform-specific tests; preserve Windows behavior.

## Contribution workflow
This objective is approved by the maintainer. Comment with your proposed approach and test plan before starting, so scope and ownership can be agreed. Keep each PR focused and link this issue. Additional features and broader IDM-parity work require maintainer approval first. Do not include cookies, signed media URLs or browser profiles in public evidence.

## Delivery plan
Tracking goal: deliver through focused PRs, not one platform rewrite.
- [ ] Shared platform adapters: #14
- [ ] Linux runtime/native messaging: #22 (after #14)
- [ ] Linux packaging and clean-machine validation: #23 (after #22)
Keep this goal open until the agreed Linux support matrix passes validation and the maintainer approves release.



---

---

## Comments (0)

*No comments on this issue yet.*
