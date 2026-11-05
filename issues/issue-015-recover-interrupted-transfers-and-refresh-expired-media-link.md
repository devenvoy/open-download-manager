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

## Comments (1)

### Comment #1 by [@Trust-Coder](https://github.com/Trust-Coder) • `2026-10-28T04:49:22Z`
[Original Comment Link](https://github.com/Ahmadkhattak1/awesome-downloader/issues/15#issuecomment-6460936286)

I've opened #45 for a narrow slice of this issue: **bounded retries and honoring server retry instructions** for direct HTTP transfers.

- Permanent failures (403/404/410/416, server ignoring ranges or a changed resource, inconsistent `Content-Range`, non-identity encoding) now fail on the first attempt instead of retrying 4 times.
- `429`/`503` responses honor `Retry-After` (seconds or HTTP date, capped at 60 s). Network errors, `408` and `5xx` still retry as before. Cancel/pause still interrupt any wait immediately.
- Fixture tests cover permanent failure, `Retry-After` timing, cancellation during the wait, and header parsing.

Not included, and still open under this issue: refreshing expired browser-captured links, recovering changed resources, and the HLS segment retry loop. It also doesn't overlap closed PR #3 (no new settings or UI). Happy to adjust the scope if you'd prefer it split differently.

---
