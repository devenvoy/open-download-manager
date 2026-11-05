---
number: 15
title: "[Firefox 2/2] persistent extension distribution and guided setup"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:50:17Z
updated_at: 2026-11-05T19:50:17Z
url: https://github.com/devenvoy/open-download-manager/issues/15
labels: ["help wanted", "status: blocked"]
---

# Issue #15: [Firefox 2/2] persistent extension distribution and guided setup

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:50:17Z`
- **Labels**: `help wanted`, `status: blocked`
- **Issue Link**: [#15 on GitHub](https://github.com/devenvoy/open-download-manager/issues/15)

---

## Description

> *Mirrored from upstream issue [#29](https://github.com/Ahmadkhattak1/awesome-downloader/issues/29) originally opened by @Ahmadkhattak1 on 2026-10-25T21:08:06Z*

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

