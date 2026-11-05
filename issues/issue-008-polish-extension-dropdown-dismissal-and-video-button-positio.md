---
number: 8
title: "Polish extension dropdown dismissal and video-button positioning"
state: OPEN
author: devenvoy
created_at: 2026-11-05T19:49:56Z
updated_at: 2026-11-05T19:49:56Z
url: https://github.com/devenvoy/open-download-manager/issues/8
labels: ["help wanted"]
---

# Issue #8: Polish extension dropdown dismissal and video-button positioning

- **Status**: `OPEN`
- **Author**: [@devenvoy](https://github.com/devenvoy)
- **Created**: `2026-11-05T19:49:56Z`
- **Labels**: `help wanted`
- **Issue Link**: [#8 on GitHub](https://github.com/devenvoy/open-download-manager/issues/8)

---

## Description

> *Mirrored from upstream issue [#17](https://github.com/Ahmadkhattak1/awesome-downloader/issues/17) originally opened by @Ahmadkhattak1 on 2026-10-25T21:07:30Z*

## Scope and acceptance
Dismiss the quality menu on outside click and scrolling; hide/reposition the button when its video leaves view. Support dismissing the button, keyboard navigation and SPA video changes. Avoid raw signed-URL tooltips. Verify multi-video pages, nested scrolling and extension reload/context invalidation. Preserve user-selected quality and working handoff.


## Working agreement
Approved objective. Claim this issue with an approach and test plan before starting to avoid duplicate work. One focused PR per independently testable change; use draft PRs for dependencies. Each PR must link its issue, include regression evidence and receive maintainer review. Keep main usable on Windows. No unrelated dependency upgrades, credentials, proprietary code or automatic release publishing. New scope beyond this issue goes through the maintainer. Passing unit tests alone does not establish platform/browser support.

