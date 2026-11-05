---
number: 27
title: "[Windows ARM64 3/3] installer, distinct artifacts and hardware validation"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:08:01Z
updated_at: 2026-10-25T21:18:01Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/27
labels: ["help wanted", "status: blocked"]
assignees: []
comments_count: 0
---

# Issue #27: [Windows ARM64 3/3] installer, distinct artifacts and hardware validation

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:08:01Z`
- **Updated**: `2026-10-25T21:18:01Z`
- **Labels**: `help wanted`, `status: blocked`
- **Original Issue**: [#27 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/27)

---

## Description

## Delivery position
Parent goal: #8 (Windows ARM64). **Step 3 of 3.**

Blocked by #26. Begin implementation after that prerequisite is merged and validated; early planning is welcome.
Final step: supply the parent goal with end-to-end validation evidence.

PRs: link this implementation issue and use `Refs #8`; close only this step when fully completed. Update the parent checklist and readiness label after validation. Status labels are maintained manually, not automatically enforced merge gates.

## Scope and acceptance
Depends on ARM64 runtime/dependency work. Build a distinct ARM64 installer and retain x64 artifacts without name collisions. Test on Windows ARM hardware: clean install, browser handoff, direct/media downloads and merging, cancel/resume, upgrade and uninstall. Provide reproducible build instructions and explicit architecture/support limitations before release.

Parent goal: #8. This issue is one deliverable toward that goal; merging it does not close the parent.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

## Dependencies
Depends on #26; contributes to parent #8.



---

---

## Comments (0)

*No comments on this issue yet.*
