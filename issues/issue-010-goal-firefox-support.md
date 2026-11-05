---
number: 10
title: "[Goal] Firefox support"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:00:31Z
updated_at: 2026-10-25T21:18:04Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/10
labels: ["enhancement", "help wanted", "type: goal"]
assignees: []
comments_count: 0
---

# Issue #10: [Goal] Firefox support

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:00:31Z`
- **Updated**: `2026-10-25T21:18:04Z`
- **Labels**: `enhancement`, `help wanted`, `type: goal`
- **Original Issue**: [#10 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/10)

---

## Description

## Work order — Firefox
This is the tracking goal. Pick a step below rather than opening a PR for the entire goal.

1. #28 — Firefox: media capture and native messaging compatibility. Start here after agreeing scope.
2. #29 — Firefox: persistent extension distribution and guided setup. Depends on #28 being merged and validated.

Each PR should use `Closes #<implementation-issue>` only when it completes that issue, and `Refs #10` for this goal. Do not auto-close this parent from an individual step. Mark steps complete after merge and validation; close the goal only after its acceptance criteria and maintainer release review are satisfied. When a prerequisite completes, the maintainer changes the next issue from `status: blocked` to `status: ready`. Draft work may be discussed early but must not be merged ahead of its dependency.

## Objective
The maintainer has approved Firefox. A Firefox manifest and manual registration path already exist, but automatic setup and YouTube capture do not have Chrome/Edge parity.

## Acceptance criteria
- Audit Firefox API/manifest compatibility and implement missing media capture and native messaging behavior.
- Provide a persistent extension installation/distribution plan; clearly distinguish temporary developer loading from normal-user installation.
- Add accurate setup instructions and desktop integration status.
- Verify supported YouTube/direct media selection, separate audio/video merging and desktop handoff.
- Test browser restart, multiple profiles, duplicate requests and connector failure on supported platforms.
- Document actual limitations and add regression tests; do not claim parity based solely on a manifest.

## Contribution workflow
This objective is approved by the maintainer. Comment with your proposed approach and test plan before starting, so scope and ownership can be agreed. Keep each PR focused and link this issue. Additional features and broader IDM-parity work require maintainer approval first. Do not include cookies, signed media URLs or browser profiles in public evidence.

## Delivery plan
- [ ] Capture and native messaging compatibility: #28
- [ ] Persistent extension installation and guided setup: #29 (after #28)
Coordinate new OS registration with #6/#7. Keep this parent open until both user-visible support and installation work.



---

---

## Comments (0)

*No comments on this issue yet.*
