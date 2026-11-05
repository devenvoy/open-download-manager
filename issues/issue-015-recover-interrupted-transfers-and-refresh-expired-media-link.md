---
number: 15
title: "Recover interrupted transfers and refresh expired media links"
state: OPEN
author: Ahmadkhattak1
created_at: 2026-10-25T21:07:24Z
updated_at: 2026-10-28T04:49:22Z
closed_at: null
url: https://github.com/Ahmadkhattak1/awesome-downloader/issues/15
labels: ["help wanted"]
assignees: []
comments_count: 1
---

# Issue #15: Recover interrupted transfers and refresh expired media links

- **Status**: `OPEN`
- **Author**: [@Ahmadkhattak1](https://github.com/Ahmadkhattak1)
- **Created**: `2026-10-25T21:07:24Z`
- **Updated**: `2026-10-28T04:49:22Z`
- **Labels**: `help wanted`
- **Original Issue**: [#15 on GitHub](https://github.com/Ahmadkhattak1/awesome-downloader/issues/15)

---

## Description

## Scope and acceptance
Define recovery behavior for disconnects and expired browser-captured URLs. Preserve verified partial bytes only when resource identity and range support permit; otherwise explain restart. Obtain fresh session data through the existing browser integration, bound retries, honor server retry instructions and allow immediate cancellation. Keep credentials out of persisted history/logs. Add fixtures for expiry, changed resource, partial transfer and cancellation. Coordinate existing retry PR #3 rather than duplicating it.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.



---

---

## Comments (0)

*No comments on this issue yet.*
