---
number: 29
title: "[Firefox 2/2] persistent extension distribution and guided setup"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:08:06Z
updated_at: 2026-10-25T21:18:10Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/29
labels: ["help wanted", "status: blocked"]
assignees: []
comments_count: 0
---

# Issue #29: [Firefox 2/2] persistent extension distribution and guided setup

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:08:06Z`
- **Updated**: `2026-10-25T21:18:10Z`
- **Labels**: `help wanted`, `status: blocked`
- **Original Issue**: [#29 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/29)

---

## Description

## Delivery position
Parent goal: #10 (Firefox). **Step 2 of 2.**

Blocked by #28. Begin implementation after that prerequisite is merged and validated; early planning is welcome.
Final step: supply the parent goal with end-to-end validation evidence.

PRs: link this implementation issue and use `Refs #10`; close only this step when fully completed. Update the parent checklist and readiness label after validation. Status labels are maintained manually, not automatically enforced merge gates.

## Scope and acceptance
Depends on verified Firefox capture/handoff. Define signed persistent installation and store/distribution requirements, accurate setup UI and browser integration status. Clearly label temporary developer loading. Verify installation survives restart and upgrades; publishing/signing requires maintainer credentials and release approval, not contributor access.

Parent goal: #10. This issue is one deliverable toward that goal; merging it does not close the parent.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

## Dependencies
Depends on #28; contributes to #10.



---

## Comments (0)

*No comments on this issue yet.*
