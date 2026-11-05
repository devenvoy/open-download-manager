---
number: 9
title: "Add named download queues and recurring schedules"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:59Z
updated_at: 2026-11-05T19:49:59Z
url: https://github.com/devenvoy/open-download-manager/issues/9
labels: ["help wanted"]
---

# Issue #9: Add named download queues and recurring schedules

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:59Z`
- **Labels**: `help wanted`
- **Issue Link**: [#9 on GitHub](https://github.com/devenvoy/open-download-manager/issues/9)

---

## Description

> *Mirrored from upstream issue [#19](https://github.com/Ahmadkhattak1/awesome-downloader/issues/19) originally opened by @Ahmadkhattak1 on 2026-10-25T21:07:35Z*

## Scope and acceptance
Propose a small data model and UX before implementation. Support named queues, ordering and explicit recurring start/stop rules. Specify timezone, daylight-saving, missed-run and app-closed behavior without implying background execution exists. Preserve current one-off schedules/history. Use separate PRs for queue persistence/operations and recurring scheduling/UI, with migration and deterministic clock tests.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

