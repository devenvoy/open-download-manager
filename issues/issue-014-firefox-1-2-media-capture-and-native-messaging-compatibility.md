---
number: 14
title: "[Firefox 1/2] media capture and native messaging compatibility"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:50:14Z
updated_at: 2026-11-05T19:50:14Z
url: https://github.com/devenvoy/open-download-manager/issues/14
labels: ["help wanted", "status: ready"]
---

# Issue #14: [Firefox 1/2] media capture and native messaging compatibility

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:50:14Z`
- **Labels**: `help wanted`, `status: ready`
- **Issue Link**: [#14 on GitHub](https://github.com/devenvoy/open-download-manager/issues/14)

---

## Description

> *Mirrored from upstream issue [#28](https://github.com/Ahmadkhattak1/awesome-downloader/issues/28) originally opened by @Ahmadkhattak1 on 2026-10-25T21:08:03Z*

## Delivery position
Parent goal: #10 (Firefox). **Step 1 of 2.**

Ready for a scope/ownership proposal; no implementation prerequisite.
Next: #29.

PRs: link this implementation issue and use `Refs #10`; close only this step when fully completed. Update the parent checklist and readiness label after validation. Status labels are maintained manually, not automatically enforced merge gates.

## Scope and acceptance
Audit existing Firefox manifest and API differences; implement browser capture, format selection, audio pairing and handoff parity for supported media. Keep native messaging trust boundaries and scoped credentials. Test Firefox on currently supported Windows first and coordinate platform registration with #6/#7. Include reload, navigation, multiple profiles and unavailable-host tests.

Parent goal: #10. This issue is one deliverable toward that goal; merging it does not close the parent.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

## Dependencies
Contributes to #10; installation follows in #29. Windows implementation can proceed before Linux/macOS ports.

