---
number: 7
title: "Recover interrupted transfers and refresh expired media links"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:53Z
updated_at: 2026-11-05T19:58:29Z
url: https://github.com/devenvoy/open-download-manager/issues/7
labels: ["help wanted"]
---

# Issue #7: Recover interrupted transfers and refresh expired media links

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:53Z`
- **Labels**: `help wanted`
- **Issue Link**: [#7 on GitHub](https://github.com/devenvoy/open-download-manager/issues/7)

---

## Description

> *Mirrored from upstream issue [#15](https://github.com/Ahmadkhattak1/awesome-downloader/issues/15) originally opened by @Ahmadkhattak1 on 2026-10-25T21:07:24Z*

## Scope and acceptance
Define recovery behavior for disconnects and expired browser-captured URLs. Preserve verified partial bytes only when resource identity and range support permit; otherwise explain restart. Obtain fresh session data through the existing browser integration, bound retries, honor server retry instructions and allow immediate cancellation. Keep credentials out of persisted history/logs. Add fixtures for expiry, changed resource, partial transfer and cancellation. Coordinate existing retry PR #3 rather than duplicating it.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

